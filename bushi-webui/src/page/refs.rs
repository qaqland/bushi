use std::time::Instant;

use askama::Template;
use axum::{extract::Path, extract::State, response::Html};

use crate::data::{RefRecord, RepositoryPort, RepositoryRecord, short_hash};
use crate::error::{AppError, DomainError};
use crate::web::AppState;

use super::{PathView, RepoHeader, Tab, path_row, repo_context, rev_indicator};

#[derive(Clone, Debug)]
pub struct RefsPage {
    pub repo: RepositoryRecord,
    pub default_rev: String,
    pub branches: Vec<RefRecord>,
    pub tags: Vec<RefRecord>,
}

struct RefRow {
    name: String,
    tree_href: String,
    commit_href: String,
    short: String,
    history_href: String,
}

#[derive(Template)]
#[template(path = "refs.html")]
struct RefsTemplate {
    branches: Vec<RefRow>,
    tags: Vec<RefRow>,
}

pub async fn handler(
    State(state): State<AppState>,
    Path(repo_name): Path<String>,
) -> Result<Html<String>, AppError> {
    let start = Instant::now();
    let repo_ctx = repo_context(&state, &repo_name).await?;
    let page = load(&state.repo_port, &repo_name).await.map_err(|e| {
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
        tab: Tab::Refs,
        rev: Some(rev_indicator(&page.repo.name, &repo_ctx.rev)),
        object: None,
        path: Some(path_row(
            &page.repo.name,
            &page.default_rev,
            "",
            false,
            PathView::None,
        )),
    };
    super::render(
        &format!("{} - Refs", page.repo.name),
        Some(&header),
        None,
        content,
        start,
    )
    .map(Html)
}

pub async fn load<R: RepositoryPort>(
    repo_port: &R,
    repo_name: &str,
) -> Result<RefsPage, DomainError> {
    let repo = repo_port
        .get_repository(repo_name)
        .await?
        .ok_or(DomainError::RepoNotFound)?;
    let default_rev = repo_port.default_rev(&repo).await?;
    let branches = repo_port.list_refs(repo.id, 0).await?;
    let tags = repo_port.list_refs(repo.id, 1).await?;
    Ok(RefsPage {
        repo,
        default_rev,
        branches,
        tags,
    })
}

fn render_body(page: &RefsPage) -> Result<String, AppError> {
    let repo = &page.repo.name;
    RefsTemplate {
        branches: ref_rows(repo, &page.branches, false),
        tags: ref_rows(repo, &page.tags, true),
    }
    .render()
    .map_err(AppError::internal)
}

fn ref_rows(repo: &str, refs: &[RefRecord], tags: bool) -> Vec<RefRow> {
    refs.iter()
        .map(|reference| {
            let rev = if tags {
                format!("tag/{}", reference.show_name)
            } else {
                reference.show_name.clone()
            };
            let short = short_hash(&reference.commit_hash);
            RefRow {
                name: reference.show_name.clone(),
                tree_href: format!("/{repo}/-/tree/{rev}"),
                commit_href: format!("/{repo}/-/commit/{short}"),
                short,
                history_href: format!("/{repo}/-/history/{rev}"),
            }
        })
        .collect()
}
