use std::path::Path;

use async_trait::async_trait;
use git2::{Delta, DiffFormat, ObjectType, Oid, Patch, Repository};
use pulldown_cmark::{Options, Parser, html};

use crate::data::{
    BlobView, ChangedFile, CommitDiffView, CommitInfo, EntryKind, GitPort, ReadmeView,
    TreeEntryView, short_hash,
};
use crate::error::DomainError;
use crate::format::relative_time;

#[derive(Clone)]
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
        hash: String,
    ) -> Result<Option<ReadmeView>, DomainError> {
        run(repo_path, move |repo| {
            let commit = find_commit(repo, &hash)?;
            for path in [
                "README.md",
                "README.markdown",
                "README",
                "readme.md",
                "readme",
            ] {
                if let Ok(blob) = blob_at_path(repo, &commit, path) {
                    if let Ok(text) = std::str::from_utf8(blob.content()) {
                        return Ok(Some(ReadmeView {
                            path: path.to_string(),
                            html: render_markdown(text),
                        }));
                    }
                }
            }
            Ok(None)
        })
        .await
    }

    async fn list_tree(
        &self,
        repo_path: String,
        hash: String,
        path: String,
    ) -> Result<Vec<TreeEntryView>, DomainError> {
        run(repo_path, move |repo| {
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
        run(repo_path, move |repo| {
            let commit = find_commit(repo, &hash)?;
            let blob = blob_at_path(repo, &commit, &path)?;
            let bytes = blob.content().to_vec();
            let size = bytes.len();
            let text_lines = std::str::from_utf8(&bytes)
                .ok()
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
        run(repo_path, move |repo| {
            let commit = find_commit(repo, &hash)?;
            let commit_info = read_commit_info(&commit)?;
            let diff = diff_for_commit(repo, &commit)?;
            let stats = diff.stats()?;
            let mut files = Vec::new();
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
                let (additions, deletions) = match Patch::from_diff(&diff, idx)? {
                    Some(patch) => {
                        let (_, additions, deletions) = patch.line_stats()?;
                        (additions, deletions)
                    }
                    None => (0, 0),
                };
                files.push(ChangedFile {
                    path,
                    status: delta_status(delta.status()).to_string(),
                    additions,
                    deletions,
                });
            }
            let patch_text = patch_text(&diff)?;
            let patch_lines = patch_text
                .lines()
                .map(ToOwned::to_owned)
                .take(600)
                .collect();
            Ok(CommitDiffView {
                commit: commit_info,
                files_changed: stats.files_changed(),
                insertions: stats.insertions(),
                deletions: stats.deletions(),
                files,
                patch_lines,
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
        run(repo_path, move |repo| {
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

async fn run<T, F>(repo_path: String, f: F) -> Result<T, DomainError>
where
    T: Send + 'static,
    F: FnOnce(&Repository) -> Result<T, DomainError> + Send + 'static,
{
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
        time_label: relative_time(time),
        parent_hashes: commit
            .parent_ids()
            .map(|oid| short_hash(&oid.to_string()))
            .collect(),
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

fn render_markdown(markdown: &str) -> String {
    let parser = Parser::new_ext(markdown, Options::all());
    let mut rendered = String::new();
    html::push_html(&mut rendered, parser);
    ammonia::clean(&rendered)
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
