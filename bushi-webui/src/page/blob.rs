use std::time::Instant;

use askama::Template;
use axum::{
    body::Body,
    extract::Path,
    extract::State,
    http::{HeaderValue, header},
    response::{Html, IntoResponse, Response},
};

use crate::data::{BlobView, CommitInfo, GitPort, RepositoryPort, RepositoryRecord, ResolvedRev};
use crate::error::{AppError, DomainError};
use crate::format::human_size;
use crate::web::AppState;

use super::{PathView, RepoHeader, Tab, path_row, repo_context_at_rev, rev_indicator, split_tail};

#[derive(Clone, Debug)]
pub struct BlobPage {
    pub repo: RepositoryRecord,
    pub default_rev: String,
    pub rev: ResolvedRev,
    pub rev_name: String,
    pub path: String,
    pub blob: BlobView,
    pub latest: Option<CommitInfo>,
}

struct CommitCard {
    href: String,
    short: String,
    subject: String,
    author: String,
    time: String,
}

#[derive(Template)]
#[template(path = "blob.html")]
struct BlobTemplate {
    latest: Option<CommitCard>,
    loc: usize,
    size: String,
    lines: Option<Vec<String>>,
}

pub async fn handler(
    State(state): State<AppState>,
    Path((repo_name, tail)): Path<(String, String)>,
) -> Result<Html<String>, AppError> {
    let (rev, path) = split_tail(&tail, true)?;
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
    let raw_href = format!("/{}/-/raw/{}/{}", page.repo.name, page.rev_name, page.path);
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
            true,
            PathView::Browse,
        )),
    };
    let html = super::render(
        &format!("{} - Blob {}", page.repo.name, page.path),
        Some(&header),
        Some(("", raw_href.as_str())),
        content,
        start,
    )?;
    Ok(Html(html))
}

pub async fn raw_blob(
    State(state): State<AppState>,
    Path((repo_name, tail)): Path<(String, String)>,
) -> Result<Response, AppError> {
    let (rev, path) = split_tail(&tail, true)?;
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
    Ok((
        [(
            header::CONTENT_TYPE,
            HeaderValue::from_static("application/octet-stream"),
        )],
        Body::from(page.blob.bytes),
    )
        .into_response())
}

pub async fn load<R: RepositoryPort, G: GitPort>(
    repo_port: &R,
    git_port: &G,
    repo_name: &str,
    rev_name: &str,
    path: String,
) -> Result<BlobPage, DomainError> {
    let repo = repo_port
        .get_repository(repo_name)
        .await?
        .ok_or(DomainError::RepoNotFound)?;
    let default_rev = repo_port.default_rev(&repo).await?;
    let rev = repo_port.resolve_rev(repo.id, rev_name).await?;
    let blob = git_port
        .read_blob(repo.path.clone(), rev.commit_hash.clone(), path.clone())
        .await?;
    let latest = match repo_port
        .latest_change(repo.id, &rev.commit_hash, &path)
        .await?
    {
        Some(hash) => {
            let mut commits = git_port.read_commits(repo.path.clone(), vec![hash]).await?;
            commits.pop()
        }
        None => None,
    };
    Ok(BlobPage {
        repo,
        default_rev,
        rev,
        rev_name: rev_name.to_string(),
        path,
        blob,
        latest,
    })
}

fn render_body(page: &BlobPage) -> Result<String, AppError> {
    let latest = page.latest.as_ref().map(|commit| CommitCard {
        href: format!("/{}/-/commit/{}", page.repo.name, commit.short_hash),
        short: commit.short_hash.clone(),
        subject: commit.subject.clone(),
        author: commit.author.clone(),
        time: commit.time_label.clone(),
    });
    let loc = page
        .blob
        .text_lines
        .as_ref()
        .map(|lines| lines.len())
        .unwrap_or(0);
    BlobTemplate {
        latest,
        loc,
        size: human_size(page.blob.size as u64),
        lines: page.blob.text_lines.clone(),
    }
    .render()
    .map_err(AppError::internal)
}
