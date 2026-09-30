use std::path::Path;

use crate::url::{decode, path_route};
use async_trait::async_trait;
use git2::{Delta, DiffFormat, ObjectType, Oid, Patch, Repository};
use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd, html};

use crate::data::{
    BlobView, ChangedFile, CommitDiffView, CommitInfo, EntryKind, GitPort, ReadmeView, ResolvedRev,
    TreeEntryView, short_hash,
};
use crate::error::DomainError;
use crate::format::utc_time_without_zone;

#[derive(Clone, Default)]
pub struct GitRepository;

impl GitRepository {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl GitPort for GitRepository {
    async fn read_readme(
        &self,
        repo_path: String,
        rev: ResolvedRev,
        repo_name: String,
    ) -> Result<Option<ReadmeView>, DomainError> {
        run("git.read_readme", repo_path, move |repo| {
            let commit = find_commit(repo, &rev.commit_hash)?;
            for path in [
                "README.md",
                "README.markdown",
                "README",
                "readme.md",
                "readme",
            ] {
                if let Ok(blob) = blob_at_path(repo, &commit, path)
                    && let Ok(text) = std::str::from_utf8(blob.content())
                {
                    return Ok(Some(ReadmeView {
                        path: path.to_string(),
                        html: render_markdown(text, &repo_name, &rev.input, path),
                    }));
                }
            }
            Ok(None)
        })
        .await
    }

    async fn path_kind(
        &self,
        repo_path: String,
        hash: String,
        path: String,
    ) -> Result<Option<EntryKind>, DomainError> {
        run("git.path_kind", repo_path, move |repo| {
            let commit = find_commit(repo, &hash)?;
            let tree = commit.tree()?;
            if path.is_empty() {
                return Ok(Some(EntryKind::Tree));
            }
            match tree.get_path(Path::new(&path)) {
                Ok(entry) => Ok(Some(match entry.kind() {
                    Some(ObjectType::Tree) => EntryKind::Tree,
                    Some(ObjectType::Blob) => EntryKind::Blob,
                    Some(ObjectType::Commit) => EntryKind::Commit,
                    _ => EntryKind::Other,
                })),
                Err(error) if error.code() == git2::ErrorCode::NotFound => Ok(None),
                Err(error) => Err(error.into()),
            }
        })
        .await
    }

    async fn list_tree(
        &self,
        repo_path: String,
        hash: String,
        path: String,
    ) -> Result<Vec<TreeEntryView>, DomainError> {
        run("git.list_tree", repo_path, move |repo| {
            let commit = find_commit(repo, &hash)?;
            let tree = tree_at_path(repo, &commit, &path)?;
            let mut entries = Vec::new();
            for entry in &tree {
                let kind = match entry.kind() {
                    Some(ObjectType::Tree) => EntryKind::Tree,
                    Some(ObjectType::Blob) => EntryKind::Blob,
                    Some(ObjectType::Commit) => EntryKind::Commit,
                    _ => EntryKind::Other,
                };
                let size = if kind == EntryKind::Blob {
                    repo.find_blob(entry.id())
                        .ok()
                        .map(|blob| blob.size() as u64)
                } else {
                    None
                };
                entries.push(TreeEntryView {
                    name: entry.name().unwrap_or("?").to_string(),
                    mode: format!("{:06o}", entry.filemode()),
                    kind,
                    size,
                });
            }
            entries.sort_by(|a, b| {
                let ak = if a.kind == EntryKind::Tree { 0 } else { 1 };
                let bk = if b.kind == EntryKind::Tree { 0 } else { 1 };
                ak.cmp(&bk).then_with(|| a.name.cmp(&b.name))
            });
            Ok(entries)
        })
        .await
    }

    async fn read_blob(
        &self,
        repo_path: String,
        hash: String,
        path: String,
    ) -> Result<BlobView, DomainError> {
        run("git.read_blob", repo_path, move |repo| {
            let commit = find_commit(repo, &hash)?;
            let blob = blob_at_path(repo, &commit, &path)?;
            let bytes = blob.content().to_vec();
            let size = bytes.len();
            let text_lines = std::str::from_utf8(&bytes)
                .ok()
                .filter(|text| !text.contains('\0'))
                .map(|text| text.lines().map(ToOwned::to_owned).collect());
            Ok(BlobView {
                bytes,
                size,
                text_lines,
            })
        })
        .await
    }

    async fn commit_diff(
        &self,
        repo_path: String,
        hash: String,
    ) -> Result<CommitDiffView, DomainError> {
        run("git.commit_diff", repo_path, move |repo| {
            let commit = find_commit(repo, &hash)?;
            let commit_info = read_commit_info(&commit)?;
            let mut diff = diff_for_commit(repo, &commit)?;
            diff.find_similar(None)?;
            let stats = diff.stats()?;
            let mut files = Vec::new();
            let mut remaining = 600;
            for idx in 0..diff.deltas().len() {
                let Some(delta) = diff.get_delta(idx) else {
                    continue;
                };
                let path = delta
                    .new_file()
                    .path()
                    .or_else(|| delta.old_file().path())
                    .and_then(Path::to_str)
                    .unwrap_or("")
                    .to_string();
                let (additions, deletions, lines) = match Patch::from_diff(&diff, idx)? {
                    Some(mut patch) => {
                        let (_, additions, deletions) = patch.line_stats()?;
                        let buffer = patch.to_buf()?;
                        let lines: Vec<String> = String::from_utf8_lossy(&buffer)
                            .lines()
                            .map(ToOwned::to_owned)
                            .collect();
                        (additions, deletions, lines)
                    }
                    None => (0, 0, Vec::new()),
                };
                let truncated = lines.len() > remaining;
                let patch_lines: Vec<_> = lines.into_iter().take(remaining).collect();
                remaining -= patch_lines.len();
                let old_path = delta
                    .old_file()
                    .path()
                    .and_then(Path::to_str)
                    .filter(|old| *old != path)
                    .map(ToOwned::to_owned);
                files.push(ChangedFile {
                    path,
                    old_path,
                    status: delta_status(delta.status()).to_string(),
                    additions,
                    deletions,
                    is_submodule: delta.new_file().mode() == git2::FileMode::Commit
                        || delta.old_file().mode() == git2::FileMode::Commit,
                    patch_lines,
                    truncated,
                });
            }
            let patch_text = patch_text(&diff)?;
            let truncated = files.iter().any(|file| file.truncated);
            Ok(CommitDiffView {
                commit: commit_info,
                files_changed: stats.files_changed(),
                insertions: stats.insertions(),
                deletions: stats.deletions(),
                files,
                truncated,
                patch_text,
            })
        })
        .await
    }

    async fn read_commits(
        &self,
        repo_path: String,
        hashes: Vec<String>,
    ) -> Result<Vec<CommitInfo>, DomainError> {
        run("git.read_commits", repo_path, move |repo| {
            let mut infos = Vec::new();
            for hash in hashes {
                let commit = find_commit(repo, &hash)?;
                infos.push(read_commit_info(&commit)?);
            }
            Ok(infos)
        })
        .await
    }
}

async fn run<T, F>(operation: &'static str, repo_path: String, f: F) -> Result<T, DomainError>
where
    T: Send + 'static,
    F: FnOnce(&Repository) -> Result<T, DomainError> + Send + 'static,
{
    let _timer = crate::debug_timing::start(operation);
    tokio::task::spawn_blocking(move || {
        let repo = Repository::open(&repo_path).or_else(|_| Repository::open_bare(&repo_path))?;
        f(&repo)
    })
    .await?
}

fn find_commit<'repo>(
    repo: &'repo Repository,
    hash: &str,
) -> Result<git2::Commit<'repo>, DomainError> {
    let oid = Oid::from_str(hash).map_err(|_| DomainError::RevNotFound)?;
    repo.find_commit(oid).map_err(|_| DomainError::RevNotFound)
}

fn read_commit_info(commit: &git2::Commit<'_>) -> Result<CommitInfo, DomainError> {
    let hash = commit.id().to_string();
    let author = commit.author();
    let time = commit.time().seconds();
    Ok(CommitInfo {
        short_hash: short_hash(&hash),
        hash,
        subject: commit.summary().unwrap_or("(no subject)").to_string(),
        message: commit.message().unwrap_or("").trim_end().to_string(),
        author: author.name().unwrap_or("unknown").to_string(),
        author_email: author.email().unwrap_or("").to_string(),
        time_label: utc_time_without_zone(time),
        authored_at: absolute_time(author.when()),
        parent_hashes: commit.parent_ids().map(|oid| oid.to_string()).collect(),
    })
}

fn tree_at_path<'repo>(
    repo: &'repo Repository,
    commit: &git2::Commit<'repo>,
    path: &str,
) -> Result<git2::Tree<'repo>, DomainError> {
    let tree = commit.tree()?;
    if path.is_empty() {
        return Ok(tree);
    }
    let entry = tree
        .get_path(Path::new(path))
        .map_err(|_| DomainError::PathNotFound)?;
    let object = entry.to_object(repo)?;
    object.peel_to_tree().map_err(|_| DomainError::PathNotFound)
}

fn blob_at_path<'repo>(
    repo: &'repo Repository,
    commit: &git2::Commit<'repo>,
    path: &str,
) -> Result<git2::Blob<'repo>, DomainError> {
    if path.is_empty() {
        return Err(DomainError::PathNotFound);
    }
    let tree = commit.tree()?;
    let entry = tree
        .get_path(Path::new(path))
        .map_err(|_| DomainError::PathNotFound)?;
    let object = entry.to_object(repo)?;
    object.peel_to_blob().map_err(|_| DomainError::PathNotFound)
}

fn absolute_time(value: git2::Time) -> String {
    let offset = time::UtcOffset::from_whole_seconds(value.offset_minutes() * 60)
        .unwrap_or(time::UtcOffset::UTC);
    time::OffsetDateTime::from_unix_timestamp(value.seconds()).ok()
        .and_then(|date| date.to_offset(offset).format(&time::format_description::parse_borrowed::<2>("[year]-[month]-[day] [hour]:[minute]:[second] [offset_hour sign:mandatory]:[offset_minute]").ok()?).ok())
        .unwrap_or_else(|| "Unknown date".to_string())
}

pub fn render_markdown(markdown: &str, repo: &str, rev: &str, path: &str) -> String {
    // push_html writes a bare newline for SoftBreak and relies on HTML whitespace
    // collapsing; join soft-wrapped lines with a space here instead.
    let mut events: Vec<_> = Parser::new_ext(markdown, Options::all())
        .map(|event| match event {
            Event::SoftBreak => Event::Text(" ".into()),
            event => event,
        })
        .collect();
    let mut ids = std::collections::HashSet::new();
    for idx in 0..events.len() {
        if !matches!(events[idx], Event::Start(Tag::Heading { .. })) {
            continue;
        }
        let text: String = events[idx + 1..]
            .iter()
            .take_while(|event| !matches!(event, Event::End(TagEnd::Heading(_))))
            .filter_map(|event| match event {
                Event::Text(text) | Event::Code(text) => Some(text.as_ref()),
                Event::SoftBreak | Event::HardBreak => Some(" "),
                _ => None,
            })
            .collect();
        if let Event::Start(Tag::Heading { id, .. }) = &mut events[idx] {
            let base = id.as_ref().map(ToString::to_string).unwrap_or_else(|| {
                text.to_lowercase()
                    .chars()
                    .filter_map(|c| {
                        if c.is_whitespace() {
                            Some('-')
                        } else if c.is_alphanumeric() || matches!(c, '-' | '_') {
                            Some(c)
                        } else {
                            None
                        }
                    })
                    .collect()
            });
            let base = if base.is_empty() {
                "section".to_string()
            } else {
                base
            };
            let mut unique = base.clone();
            let mut suffix = 1;
            while !ids.insert(unique.clone()) {
                unique = format!("{base}-{suffix}");
                suffix += 1;
            }
            *id = Some(unique.into());
        }
    }
    let mut rendered = String::new();
    html::push_html(&mut rendered, events.into_iter());
    let repo = repo.to_string();
    let rev = rev.to_string();
    let mut base = url::Url::parse("https://repository.invalid/").unwrap();
    base.path_segments_mut().unwrap().extend(path.split('/'));
    let mut cleaner = ammonia::Builder::default();
    cleaner.add_generic_attributes(&["id"]);
    cleaner.id_prefix(Some("readme-"));
    cleaner.attribute_filter(move |element, attribute, value| {
        if !matches!((element, attribute), ("a", "href") | ("img", "src")) {
            return Some(value.into());
        }
        if let Some(fragment) = value.strip_prefix('#') {
            return Some(format!("#readme-{fragment}").into());
        }
        if value.starts_with("//") || url::Url::parse(value).is_ok() {
            return Some(value.into());
        }
        let resolved = base.join(value).ok()?;
        let path = decode(resolved.path())?;
        // Directory links use the same content route; the blob handler redirects them to tree.
        let mut href = path_route(
            &repo,
            if element == "img" { "raw" } else { "blob" },
            &rev,
            path.trim_matches('/'),
        );
        if resolved.path() == base.path() && element == "a" && resolved.fragment().is_some() {
            href = String::new();
            href.push_str("#readme-");
            href.push_str(resolved.fragment().unwrap());
        } else if let Some(fragment) = resolved.fragment() {
            href.push('#');
            href.push_str(fragment);
        }
        Some(href.into())
    });
    cleaner.clean(&rendered).to_string()
}

fn diff_for_commit<'repo>(
    repo: &'repo Repository,
    commit: &git2::Commit<'repo>,
) -> Result<git2::Diff<'repo>, DomainError> {
    let tree = commit.tree()?;
    if commit.parent_count() == 0 {
        let diff = repo.diff_tree_to_tree(None, Some(&tree), None)?;
        return Ok(diff);
    }
    let parent = commit.parent(0)?;
    let parent_tree = parent.tree()?;
    let diff = repo.diff_tree_to_tree(Some(&parent_tree), Some(&tree), None)?;
    Ok(diff)
}

fn patch_text(diff: &git2::Diff<'_>) -> Result<String, DomainError> {
    let mut out = String::new();
    diff.print(DiffFormat::Patch, |_delta, _hunk, line| {
        match line.origin() {
            ' ' | '+' | '-' | '=' | '>' | '<' => out.push(line.origin()),
            _ => {}
        }
        out.push_str(std::str::from_utf8(line.content()).unwrap_or(""));
        true
    })?;
    Ok(out)
}

fn delta_status(status: Delta) -> &'static str {
    match status {
        Delta::Added => "added",
        Delta::Deleted => "deleted",
        Delta::Modified => "modified",
        Delta::Renamed => "renamed",
        Delta::Copied => "copied",
        Delta::Typechange => "typechange",
        Delta::Unreadable => "unreadable",
        Delta::Conflicted => "conflicted",
        Delta::Ignored => "ignored",
        Delta::Untracked => "untracked",
        Delta::Unmodified => "unmodified",
    }
}

#[cfg(test)]
mod tests {
    use super::render_markdown;

    #[test]
    fn readme_links_anchors_and_html_stay_in_their_context() {
        let html = render_markdown(
            "# Usage\n\n[Self](README.md#usage) [Root](/LICENSE) [Directory](../src/)\n\n![Image](../image.png)\n\nText[^note].\n\n[^note]: A note.\n\n## Usage\n\n<span id=\"content\">Safe anchor</span><a href=\"javascript:alert(1)\">Unsafe link</a><script>alert(1)</script>",
            "repo space",
            "feature:login",
            "docs/README.md",
        );
        assert!(html.contains("href=\"#readme-usage\""));
        assert!(html.contains("id=\"readme-usage-1\""));
        assert!(html.contains("href=\"/repo%20space/-/blob/feature:login/LICENSE\""));
        assert!(html.contains("href=\"/repo%20space/-/blob/feature:login/src\""));
        assert!(html.contains("src=\"/repo%20space/-/raw/feature:login/image.png\""));
        assert!(html.contains("href=\"#readme-note\""));
        assert!(html.contains("id=\"readme-note\""));
        assert!(html.contains("id=\"readme-content\""));
        assert!(!html.contains("id=\"content\""));
        assert!(!html.contains("javascript:"));
        assert!(!html.contains("<script>"));
    }

    #[test]
    fn soft_breaks_join_wrapped_lines() {
        let html = render_markdown("one\ntwo\nthree\n", "repo", "main", "README.md");
        assert!(html.contains("<p>one two three</p>"), "{html}");
    }
}
