use askama::Template;
use axum::{
    extract::Path,
    extract::State,
    response::{Html, Redirect},
};

use crate::data::{
    EntryKind, GitPort, RepositoryPort, RepositoryRecord, ResolvedRev, TreeEntryView,
};
use crate::error::{AppError, DomainError};
use crate::web::AppState;

use super::{
    PathView, RepoHeader, join_path, parent_path, path_route, path_row,
    repo_context_at_rev_for_repo, require_repo, rev_indicator, split_tail,
};

#[derive(Clone, Debug)]
pub struct TreePage {
    pub repo: RepositoryRecord,
    pub rev: ResolvedRev,
    pub rev_name: String,
    pub path: String,
    pub entries: Vec<TreeEntryView>,
}

struct TreeRow {
    href: String,
    name: String,
    mode: String,
    size: String,
    kind_label: &'static str,
}

#[derive(Template)]
#[template(path = "tree.html")]
struct TreeTemplate {
    parent_href: Option<String>,
    rows: Vec<TreeRow>,
}

pub async fn default_handler(
    State(state): State<AppState>,
    Path(repo_name): Path<String>,
) -> Result<Redirect, AppError> {
    let ctx = super::repo_context(&state, &repo_name)
        .await
        .map_err(|error| error.with_config(&state.config))?;
    Ok(Redirect::temporary(&path_route(
        &repo_name,
        "tree",
        &ctx.default_rev,
        "",
    )))
}

pub async fn handler(
    State(state): State<AppState>,
    Path((repo_name, tail)): Path<(String, String)>,
) -> Result<Html<String>, AppError> {
    let config = state.config.clone();
    let result = async {
        let repo = require_repo(&state, &repo_name).await?;
        let (rev, path) =
            split_tail(&tail, false).map_err(|error| error.with_repo(repo.clone()))?;
        let repo_ctx = repo_context_at_rev_for_repo(&state, repo, &rev)
            .await
            .map_err(|e| e.with_path(&rev, &path))?;
        let page = load(
            &state.repo_port,
            &state.git_port,
            &repo_name,
            &rev,
            path.clone(),
        )
        .await
        .map_err(|e| {
            AppError::from_domain(e, Some(repo_ctx.repo.clone())).with_path(&rev, &path)
        })?;
        let content = render_body(&page)?;
        let mut header = RepoHeader::new(&page.repo.name, Some(&page.rev_name), "files");
        header.rev = Some(rev_indicator(
            &page.repo.name,
            &page.rev,
            &page.path,
            "tree",
        ));
        header.path = Some(path_row(
            &page.repo.name,
            &page.rev_name,
            &page.path,
            false,
            PathView::Browse,
        ));
        let title = format!("{} - Tree {}", page.repo.name, page.path);
        super::render(&config, &title, Some(&header), None, content).map(Html)
    }
    .await;
    result.map_err(|error| error.with_config(&config))
}

pub async fn load<R: RepositoryPort, G: GitPort>(
    repo_port: &R,
    git_port: &G,
    repo_name: &str,
    rev_name: &str,
    path: String,
) -> Result<TreePage, DomainError> {
    let repo = repo_port
        .get_repository(repo_name)
        .await?
        .ok_or(DomainError::RepoNotFound)?;
    let rev = repo_port.resolve_rev(repo.id, rev_name).await?;
    let entries = git_port
        .list_tree(repo.path.clone(), rev.commit_hash.clone(), path.clone())
        .await?;
    Ok(TreePage {
        repo,
        rev,
        rev_name: rev_name.to_string(),
        path,
        entries,
    })
}

fn render_body(page: &TreePage) -> Result<String, AppError> {
    let _timer = crate::debug_timing::start("render.body");
    let repo_name = &page.repo.name;
    let rev = &page.rev_name;
    let path = &page.path;
    let parent_href =
        (!path.is_empty()).then(|| path_route(repo_name, "tree", rev, &parent_path(path)));
    let rows = page
        .entries
        .iter()
        .map(|entry| {
            let child_path = join_path(path, &entry.name);
            let op = match entry.kind {
                EntryKind::Blob => "blob",
                _ => "tree",
            };
            TreeRow {
                href: if matches!(entry.kind, EntryKind::Blob | EntryKind::Tree) {
                    path_route(repo_name, op, rev, &child_path)
                } else {
                    String::new()
                },
                name: entry.kind.display_name(&entry.name),
                mode: entry.mode.clone(),
                size: entry.kind.display_size(entry.size),
                kind_label: match entry.kind {
                    EntryKind::Commit => "Submodule",
                    EntryKind::Other => "Unsupported object",
                    EntryKind::Blob if entry.mode == "120000" => "Symbolic link",
                    EntryKind::Blob if entry.mode == "100755" => "Executable",
                    _ => "",
                },
            }
        })
        .collect();
    TreeTemplate { parent_href, rows }
        .render()
        .map_err(AppError::internal)
}

#[cfg(test)]
mod tests {
    use askama::Template;

    use super::{TreeRow, TreeTemplate};

    #[test]
    fn template_renders_rows() {
        let html = TreeTemplate {
            parent_href: Some("/repo/-/tree/main".to_string()),
            rows: vec![
                TreeRow {
                    href: "/repo/-/tree/main/page".to_string(),
                    name: "page/".to_string(),
                    mode: "040000".to_string(),
                    size: "--".to_string(),
                    kind_label: "",
                },
                TreeRow {
                    href: "/repo/-/blob/main/main.rs".to_string(),
                    name: "main.rs".to_string(),
                    mode: "100644".to_string(),
                    size: "1.0 KiB".to_string(),
                    kind_label: "",
                },
            ],
        }
        .render()
        .unwrap();

        assert!(html.contains(
            "<th scope=\"col\">Mode</th><th scope=\"col\">Name</th><th scope=\"col\">Size</th>"
        ));
        assert!(html.contains("<td><a href=\"/repo/-/tree/main\">../</a></td>"));
        assert!(html.contains("<td><a href=\"/repo/-/tree/main/page\">page/</a>"));
        assert!(html.contains("<td><a href=\"/repo/-/blob/main/main.rs\">main.rs</a>"));
    }
}
