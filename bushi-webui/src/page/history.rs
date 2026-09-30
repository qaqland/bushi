use askama::Template;
use axum::{extract::Path, extract::Query, extract::State, response::Html};
use serde::Deserialize;

use crate::data::{EntryKind, GitPort, LogPage, RepositoryPort, RepositoryRecord, ResolvedRev};
use crate::error::{AppError, DomainError};
use crate::web::AppState;

use super::{
    PathView, RepoHeader, path_route, path_row, repo_context_at_rev_for_repo, require_repo,
    rev_indicator, split_tail,
};

#[derive(Clone, Debug)]
pub struct HistoryPageView {
    pub repo: RepositoryRecord,
    pub rev: ResolvedRev,
    pub rev_name: String,
    pub path: Option<String>,
    pub log: LogPage,
    pub is_file: bool,
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
    latest_href: String,
    title: String,
}

pub async fn handler(
    State(state): State<AppState>,
    Path((repo_name, tail)): Path<(String, String)>,
    query: Result<Query<HistoryQuery>, axum::extract::rejection::QueryRejection>,
) -> Result<Html<String>, AppError> {
    let config = state.config.clone();
    let result = async {
        let repo = require_repo(&state, &repo_name).await?;
        let Query(query) = query.map_err(|_| {
            AppError::bad_request("Invalid history parameters.", Some(repo.clone()))
        })?;
        let (rev, path) =
            split_tail(&tail, false).map_err(|error| error.with_repo(repo.clone()))?;
        let path = (!path.is_empty()).then_some(path);
        let repo_ctx = repo_context_at_rev_for_repo(&state, repo, &rev)
            .await
            .map_err(|e| e.with_path(&rev, path.as_deref().unwrap_or("")))?;
        let page = load(
            &state.repo_port,
            &state.git_port,
            &repo_name,
            &rev,
            path.clone(),
            query.after.as_deref(),
            30,
        )
        .await
        .map_err(|e| {
            AppError::from_domain(e, Some(repo_ctx.repo.clone()))
                .with_path(&rev, path.as_deref().unwrap_or(""))
        })?;
        let content = render_body(&page, query.after.is_some())?;
        let path = page.path.as_deref().unwrap_or("");
        let mut header = RepoHeader::new(&repo_ctx.repo.name, Some(&page.rev_name), "files");
        header.rev = Some(rev_indicator(
            &repo_ctx.repo.name,
            &page.rev,
            path,
            "history",
        ));
        header.path = Some(path_row(
            &repo_ctx.repo.name,
            &page.rev_name,
            path,
            page.is_file,
            PathView::History,
        ));
        super::render(
            &config,
            &format!("{} - History", repo_ctx.repo.name),
            Some(&header),
            None,
            content,
        )
        .map(Html)
    }
    .await;
    result.map_err(|error| error.with_config(&config))
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
    let rev = repo_port.resolve_rev(repo.id, rev_name).await?;
    let kind = git_port
        .path_kind(
            repo.path.clone(),
            rev.commit_hash.clone(),
            path.clone().unwrap_or_default(),
        )
        .await?;
    let history = match path.as_deref() {
        Some(path) => {
            let indexed_path = if kind == Some(EntryKind::Tree) {
                format!("{path}/")
            } else {
                path.to_string()
            };
            repo_port
                .path_history(repo.id, &indexed_path, &rev.commit_hash, after, limit)
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
    let previous_kind = if kind.is_none() {
        if let Some(parent) = entries
            .first()
            .and_then(|entry| entry.parent_hashes.first())
        {
            git_port
                .path_kind(
                    repo.path.clone(),
                    parent.clone(),
                    path.clone().unwrap_or_default(),
                )
                .await?
        } else {
            None
        }
    } else {
        kind
    };
    let is_file = previous_kind != Some(EntryKind::Tree) && path.is_some();
    Ok(HistoryPageView {
        repo,
        rev,
        rev_name: rev_name.to_string(),
        path,
        is_file,
        log: LogPage {
            entries,
            next_after: history.next_after,
        },
    })
}

fn render_body(page: &HistoryPageView, paginated: bool) -> Result<String, AppError> {
    let _timer = crate::debug_timing::start("render.body");
    let repo = &page.repo.name;
    let rev = &page.rev_name;
    let entries = page
        .log
        .entries
        .iter()
        .map(|entry| HistoryEntry {
            href: path_route(repo, "commit", &entry.hash, ""),
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
        latest_href: if paginated {
            path_route(repo, "history", rev, page.path.as_deref().unwrap_or(""))
        } else {
            String::new()
        },
        title: page
            .path
            .as_ref()
            .map(|path| format!("History of {path}"))
            .unwrap_or_else(|| "Commit history".to_string()),
    }
    .render()
    .map_err(AppError::internal)
}
