use std::time::Instant;

use askama::Template;
use axum::{extract::Path, extract::State, response::Html};

use crate::data::{
    EntryKind, GitPort, RepositoryPort, RepositoryRecord, ResolvedRev, TreeEntryView,
};
use crate::error::{AppError, DomainError};
use crate::web::AppState;

use super::{
    PathView, RepoHeader, Tab, join_path, parent_path, path_row, repo_context_at_rev,
    rev_indicator, split_tail,
};

#[derive(Clone, Debug)]
pub struct TreePage {
    pub repo: RepositoryRecord,
    pub default_rev: String,
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
}

#[derive(Template)]
#[template(path = "tree.html")]
struct TreeTemplate {
    parent_href: Option<String>,
    rows: Vec<TreeRow>,
}

pub async fn handler(
    State(state): State<AppState>,
    Path((repo_name, tail)): Path<(String, String)>,
) -> Result<Html<String>, AppError> {
    let (rev, path) = split_tail(&tail, false)?;
    let start = Instant::now();
    let repo_ctx = repo_context_at_rev(&state, &repo_name, &rev).await?;
    let page = load(&state.repo_port, &state.git_port, &repo_name, &rev, path)
        .await
        .map_err(|e| {
            AppError::from_domain(
                e,
                Some(repo_ctx.repo.clone()),
                Some(repo_ctx.default_rev.clone()),
            )
        })?;
    let content = render_body(&page)?;
    let header = RepoHeader {
        name: page.repo.name.clone(),
        default_rev: page.default_rev.clone(),
        tab: Tab::Files,
        rev: Some(rev_indicator(&page.repo.name, &page.rev)),
        object: None,
        path: Some(path_row(
            &page.repo.name,
            &page.rev_name,
            &page.path,
            false,
            PathView::Browse,
        )),
    };
    let title = format!("{} - Tree {}", page.repo.name, page.path);
    super::render(&title, Some(&header), None, content, start).map(Html)
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
    let default_rev = repo_port.default_rev(&repo).await?;
    let rev = repo_port.resolve_rev(repo.id, rev_name).await?;
    let entries = git_port
        .list_tree(repo.path.clone(), rev.commit_hash.clone(), path.clone())
        .await?;
    Ok(TreePage {
        repo,
        default_rev,
        rev,
        rev_name: rev_name.to_string(),
        path,
        entries,
    })
}

fn render_body(page: &TreePage) -> Result<String, AppError> {
    let repo_name = &page.repo.name;
    let rev = &page.rev_name;
    let path = &page.path;
    let parent_href =
        (!path.is_empty()).then(|| format!("/{repo_name}/-/tree/{rev}/{}", parent_path(path)));
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
                href: format!("/{repo_name}/-/{op}/{rev}/{child_path}"),
                name: entry.kind.display_name(&entry.name),
                mode: entry.mode.clone(),
                size: entry.kind.display_size(entry.size),
            }
        })
        .collect();
    TreeTemplate { parent_href, rows }
        .render()
        .map_err(AppError::internal)
}
