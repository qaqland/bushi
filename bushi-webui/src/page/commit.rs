use std::time::Instant;

use askama::Template;
use axum::{
    extract::Path,
    extract::State,
    response::{Html, IntoResponse, Response},
};

use crate::data::{CommitDiffView, GitPort, RepositoryPort, RepositoryRecord};
use crate::error::{AppError, DomainError};
use crate::web::AppState;

use super::{ObjectRow, PathView, RepoHeader, Tab, commit_rev_indicator, path_row, repo_context};

#[derive(Clone, Debug)]
pub struct CommitPage {
    pub repo: RepositoryRecord,
    pub default_rev: String,
    pub diff: CommitDiffView,
}

struct ChangedFileRow {
    path: String,
    status: String,
    additions: usize,
    deletions: usize,
    blob_href: String,
    raw_href: String,
    history_href: String,
}

struct DiffLine {
    class: &'static str,
    text: String,
}

#[derive(Template)]
#[template(path = "commit.html")]
struct CommitTemplate {
    short_hash: String,
    subject: String,
    author: String,
    author_email: String,
    time: String,
    hash: String,
    parent: String,
    message: String,
    files_changed: usize,
    insertions: usize,
    deletions: usize,
    files: Vec<ChangedFileRow>,
    diff: Vec<DiffLine>,
}

pub async fn handler(
    State(state): State<AppState>,
    Path((repo_name, hash)): Path<(String, String)>,
) -> Result<Response, AppError> {
    if let Some(hash) = hash.strip_suffix(".patch") {
        return patch_response(state, repo_name, hash.to_string()).await;
    }
    let start = Instant::now();
    let repo_ctx = repo_context(&state, &repo_name).await?;
    let page = load(&state.repo_port, &state.git_port, &repo_name, &hash)
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
        rev: Some(commit_rev_indicator(
            &page.repo.name,
            &page.diff.commit.hash,
        )),
        object: Some(ObjectRow {
            short_hash: page.diff.commit.short_hash.clone(),
            subject: page.diff.commit.subject.clone(),
            commit_href: format!(
                "/{}/-/commit/{}",
                page.repo.name, page.diff.commit.short_hash
            ),
            patch_href: format!(
                "/{}/-/commit/{}.patch",
                page.repo.name, page.diff.commit.short_hash
            ),
        }),
        path: Some(path_row(
            &page.repo.name,
            &page.diff.commit.short_hash,
            "",
            false,
            PathView::None,
        )),
    };
    let html = super::render(
        &format!("{} - Commit", page.repo.name),
        Some(&header),
        None,
        content,
        start,
    )?;
    Ok(Html(html).into_response())
}

async fn patch_response(
    state: AppState,
    repo_name: String,
    hash: String,
) -> Result<Response, AppError> {
    let repo_ctx = repo_context(&state, &repo_name).await?;
    let page = load(&state.repo_port, &state.git_port, &repo_name, &hash)
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
            axum::http::header::CONTENT_TYPE,
            axum::http::HeaderValue::from_static("text/x-patch; charset=utf-8"),
        )],
        page.diff.patch_text,
    )
        .into_response())
}

pub async fn load<R: RepositoryPort, G: GitPort>(
    repo_port: &R,
    git_port: &G,
    repo_name: &str,
    hash: &str,
) -> Result<CommitPage, DomainError> {
    let repo = repo_port
        .get_repository(repo_name)
        .await?
        .ok_or(DomainError::RepoNotFound)?;
    let default_rev = repo_port.default_rev(&repo).await?;
    let rev = repo_port.resolve_rev(repo.id, hash).await?;
    let diff = git_port
        .commit_diff(repo.path.clone(), rev.commit_hash)
        .await?;
    Ok(CommitPage {
        repo,
        default_rev,
        diff,
    })
}

fn render_body(page: &CommitPage) -> Result<String, AppError> {
    let repo = &page.repo.name;
    let diff = &page.diff;
    let c = &diff.commit;
    let parent = c
        .parent_hashes
        .first()
        .cloned()
        .unwrap_or_else(|| "none".to_string());
    let files = diff
        .files
        .iter()
        .map(|file| ChangedFileRow {
            path: file.path.clone(),
            status: file.status.clone(),
            additions: file.additions,
            deletions: file.deletions,
            blob_href: format!("/{repo}/-/blob/{}/{}", c.short_hash, file.path),
            raw_href: format!("/{repo}/-/raw/{}/{}", c.short_hash, file.path),
            history_href: format!("/{repo}/-/history/{}/{}", c.short_hash, file.path),
        })
        .collect();
    let diff_lines = diff
        .patch_lines
        .iter()
        .map(|line| {
            let class = if line.starts_with('+') && !line.starts_with("+++") {
                "diff-add"
            } else if line.starts_with('-') && !line.starts_with("---") {
                "diff-del"
            } else {
                ""
            };
            DiffLine {
                class,
                text: line.clone(),
            }
        })
        .collect();
    CommitTemplate {
        short_hash: c.short_hash.clone(),
        subject: c.subject.clone(),
        author: c.author.clone(),
        author_email: c.author_email.clone(),
        time: c.time_label.clone(),
        hash: c.hash.clone(),
        parent,
        message: c.message.clone(),
        files_changed: diff.files_changed,
        insertions: diff.insertions,
        deletions: diff.deletions,
        files,
        diff: diff_lines,
    }
    .render()
    .map_err(AppError::internal)
}
