use std::time::Instant;

use askama::Template;
use axum::{extract::Path, extract::Query, extract::State, response::Html};
use serde::Deserialize;

use crate::data::{GitPort, LogPage, RepositoryPort, RepositoryRecord, ResolvedRev};
use crate::error::{AppError, DomainError};
use crate::web::AppState;

use super::{
    PathView, RepoHeader, Tab, path_route, path_row, repo_context_at_rev, rev_indicator, split_tail,
};

#[derive(Clone, Debug)]
pub struct HistoryPageView {
    pub repo: RepositoryRecord,
    pub default_rev: String,
    pub rev: ResolvedRev,
    pub rev_name: String,
    pub path: Option<String>,
    pub log: LogPage,
}

#[derive(Deserialize)]
pub struct HistoryQuery {
    after: Option<String>,
}

struct HistoryEntry {
    href: String,
    short: String,
    subject: String,
    author: String,
    time: String,
}

#[derive(Template)]
#[template(path = "history.html")]
struct HistoryTemplate {
    entries: Vec<HistoryEntry>,
    older_href: Option<String>,
}

pub async fn handler(
    State(state): State<AppState>,
    Path((repo_name, tail)): Path<(String, String)>,
    Query(query): Query<HistoryQuery>,
) -> Result<Html<String>, AppError> {
    let (rev, path) = split_tail(&tail, false)?;
    let path = (!path.is_empty()).then_some(path);
    let start = Instant::now();
    let repo_ctx = repo_context_at_rev(&state, &repo_name, &rev).await?;
    let page = load(
        &state.repo_port,
        &state.git_port,
        &repo_name,
        &rev,
        path,
        query.after.as_deref(),
        30,
    )
    .await
    .map_err(|e| {
        AppError::from_domain(
            e,
            Some(repo_ctx.repo.clone()),
            Some(repo_ctx.default_rev.clone()),
        )
    })?;
    let content = render_body(&page)?;
    let is_file = match page.path.as_deref() {
        Some(path) => state
            .repo_port
            .path_kind(path)
            .await
            .map_err(AppError::internal)?
            .unwrap_or(true),
        None => false,
    };
    let header = RepoHeader {
        name: repo_ctx.repo.name.clone(),
        default_rev: page.default_rev.clone(),
        tab: Tab::Files,
        rev: Some(rev_indicator(&repo_ctx.repo.name, &repo_ctx.rev)),
        object: None,
        path: Some(path_row(
            &repo_ctx.repo.name,
            &page.rev_name,
            page.path.as_deref().unwrap_or(""),
            is_file,
            PathView::History,
        )),
    };
    super::render(
        &format!("{} - History", repo_ctx.repo.name),
        Some(&header),
        None,
        content,
        start,
    )
    .map(Html)
}

pub async fn load<R: RepositoryPort, G: GitPort>(
    repo_port: &R,
    git_port: &G,
    repo_name: &str,
    rev_name: &str,
    path: Option<String>,
    after: Option<&str>,
    limit: usize,
) -> Result<HistoryPageView, DomainError> {
    let repo = repo_port
        .get_repository(repo_name)
        .await?
        .ok_or(DomainError::RepoNotFound)?;
    let default_rev = repo_port.default_rev(&repo).await?;
    let rev = repo_port.resolve_rev(repo.id, rev_name).await?;
    let history = match path.as_deref() {
        Some(path) => {
            repo_port
                .path_history(repo.id, path, &rev.commit_hash, after, limit)
                .await?
        }
        None => {
            repo_port
                .log(repo.id, &rev.commit_hash, after, limit)
                .await?
        }
    };
    let entries = if history.hashes.is_empty() {
        Vec::new()
    } else {
        git_port
            .read_commits(repo.path.clone(), history.hashes)
            .await?
    };
    Ok(HistoryPageView {
        repo,
        default_rev,
        rev,
        rev_name: rev_name.to_string(),
        path,
        log: LogPage {
            entries,
            next_after: history.next_after,
        },
    })
}

fn render_body(page: &HistoryPageView) -> Result<String, AppError> {
    let repo = &page.repo.name;
    let rev = &page.rev_name;
    let entries = page
        .log
        .entries
        .iter()
        .map(|entry| HistoryEntry {
            href: format!("/{repo}/-/commit/{}", entry.short_hash),
            short: entry.short_hash.clone(),
            subject: entry.subject.clone(),
            author: entry.author.clone(),
            time: entry.time_label.clone(),
        })
        .collect();
    let older_href = page.log.next_after.as_ref().map(|after| {
        format!(
            "{}?after={after}",
            path_route(repo, "history", rev, page.path.as_deref().unwrap_or(""))
        )
    });
    HistoryTemplate {
        entries,
        older_href,
    }
    .render()
    .map_err(AppError::internal)
}
