use askama::Template;
use axum::{
    body::Body,
    extract::{Path, Query, State},
    http::header,
    response::{Html, IntoResponse, Redirect, Response},
};
use serde::Deserialize;

use super::{
    PathView, RepoHeader, path_route, path_row, repo_context_at_rev_for_repo, require_repo,
    rev_indicator, split_tail,
};
use crate::data::{
    BlobView, CommitInfo, EntryKind, GitPort, RepositoryPort, RepositoryRecord, ResolvedRev,
};
use crate::error::{AppError, DomainError};
use crate::format::human_size;
use crate::web::AppState;

#[derive(Clone, Debug)]
pub struct BlobPage {
    pub repo: RepositoryRecord,
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
    name: String,
    latest: Option<CommitCard>,
    loc: usize,
    size: String,
    lines: Option<Vec<String>>,
    raw_href: String,
    permalink: String,
    download_href: String,
}

pub async fn handler(
    State(state): State<AppState>,
    Path((repo_name, tail)): Path<(String, String)>,
) -> Result<Response, AppError> {
    let config = state.config.clone();
    let result = async {
        let repo = require_repo(&state, &repo_name).await?;
        let (rev, path) =
            split_tail(&tail, false).map_err(|error| error.with_repo(repo.clone()))?;
        let ctx = repo_context_at_rev_for_repo(&state, repo, &rev)
            .await
            .map_err(|e| e.with_path(&rev, &path))?;
        let kind = state
            .git_port
            .path_kind(
                ctx.repo.path.clone(),
                ctx.rev.commit_hash.clone(),
                path.clone(),
            )
            .await
            .map_err(|e| AppError::from_domain(e, Some(ctx.repo.clone())).with_path(&rev, &path))?;
        if kind == Some(EntryKind::Tree) {
            return Ok(
                Redirect::temporary(&path_route(&repo_name, "tree", &rev, &path)).into_response(),
            );
        }
        if kind == Some(EntryKind::Commit) {
            return Err(AppError::not_found(
                "This path is a submodule, not a file in this repository.",
                Some(ctx.repo),
            )
            .with_path(&rev, &path));
        }
        let page = load(
            &state.repo_port,
            &state.git_port,
            &repo_name,
            &rev,
            path.clone(),
        )
        .await
        .map_err(|e| AppError::from_domain(e, Some(ctx.repo.clone())).with_path(&rev, &path))?;
        let content = render_body(&page)?;
        let raw_href = path_route(&page.repo.name, "raw", &page.rev_name, &page.path);
        let mut header = RepoHeader::new(&page.repo.name, Some(&page.rev_name), "files");
        header.rev = Some(rev_indicator(
            &page.repo.name,
            &page.rev,
            &page.path,
            "blob",
        ));
        header.path = Some(path_row(
            &page.repo.name,
            &page.rev_name,
            &page.path,
            true,
            PathView::Browse,
        ));
        let alternate = page
            .blob
            .text_lines
            .as_ref()
            .map(|_| ("", raw_href.as_str()));
        super::render(
            &config,
            &format!("{} - {}", page.repo.name, page.path),
            Some(&header),
            alternate,
            content,
        )
        .map(|html| Html(html).into_response())
    }
    .await;
    result.map_err(|error: AppError| error.with_config(&config))
}

#[derive(Default, Deserialize)]
pub struct RawQuery {
    download: Option<u8>,
}

pub async fn raw_blob(
    State(state): State<AppState>,
    Path((repo_name, tail)): Path<(String, String)>,
    query: Result<Query<RawQuery>, axum::extract::rejection::QueryRejection>,
) -> Result<Response, AppError> {
    let config = state.config.clone();
    let result = async {
        let repo = require_repo(&state, &repo_name).await?;
        let Query(query) = query.map_err(|_| {
            AppError::bad_request("Invalid download parameters.", Some(repo.clone()))
        })?;
        let (rev, path) = split_tail(&tail, true).map_err(|error| error.with_repo(repo.clone()))?;
        let ctx = repo_context_at_rev_for_repo(&state, repo, &rev)
            .await
            .map_err(|e| e.with_path(&rev, &path))?;
        let blob = state
            .git_port
            .read_blob(ctx.repo.path.clone(), ctx.rev.commit_hash, path.clone())
            .await
            .map_err(|e| AppError::from_domain(e, Some(ctx.repo)).with_path(&rev, &path))?;
        let content_type = raw_content_type(&blob.bytes, blob.text_lines.is_some());
        let download = query.download == Some(1) || content_type == "application/octet-stream";
        let disposition = if download {
            format!(
                "attachment; filename*=UTF-8''{}",
                crate::url::segment(path.rsplit('/').next().unwrap_or("file")).replace(':', "%3A")
            )
        } else {
            "inline".to_string()
        };
        Ok((
            [
                (header::CONTENT_TYPE, content_type.to_string()),
                (header::CONTENT_DISPOSITION, disposition),
                (header::X_CONTENT_TYPE_OPTIONS, "nosniff".to_string()),
                (
                    header::CONTENT_SECURITY_POLICY,
                    "default-src 'none'; sandbox".to_string(),
                ),
            ],
            Body::from(blob.bytes),
        )
            .into_response())
    }
    .await;
    result.map_err(|error: AppError| error.with_config(&config))
}

fn raw_content_type(bytes: &[u8], text: bool) -> &'static str {
    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        "image/png"
    } else if bytes.starts_with(b"\xff\xd8\xff") {
        "image/jpeg"
    } else if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
        "image/gif"
    } else if bytes.starts_with(b"RIFF") && bytes.get(8..12) == Some(b"WEBP") {
        "image/webp"
    } else if text {
        "text/plain; charset=utf-8"
    } else {
        "application/octet-stream"
    }
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
    let rev = repo_port.resolve_rev(repo.id, rev_name).await?;
    let blob = git_port
        .read_blob(repo.path.clone(), rev.commit_hash.clone(), path.clone())
        .await?;
    let latest = match repo_port
        .latest_change(repo.id, &rev.commit_hash, &path)
        .await?
    {
        Some(hash) => git_port
            .read_commits(repo.path.clone(), vec![hash])
            .await?
            .pop(),
        None => None,
    };
    Ok(BlobPage {
        repo,
        rev,
        rev_name: rev_name.to_string(),
        path,
        blob,
        latest,
    })
}

fn render_body(page: &BlobPage) -> Result<String, AppError> {
    let _timer = crate::debug_timing::start("render.body");
    let latest = page.latest.as_ref().map(|commit| CommitCard {
        href: path_route(&page.repo.name, "commit", &commit.hash, ""),
        short: commit.short_hash.clone(),
        subject: commit.subject.clone(),
        author: commit.author.clone(),
        time: commit.time_label.clone(),
    });
    let raw_href = path_route(&page.repo.name, "raw", &page.rev.commit_hash, &page.path);
    BlobTemplate {
        name: page
            .path
            .rsplit('/')
            .next()
            .unwrap_or(&page.path)
            .to_string(),
        latest,
        loc: page.blob.text_lines.as_ref().map_or(0, Vec::len),
        size: human_size(page.blob.size as u64),
        lines: page.blob.text_lines.clone(),
        permalink: path_route(&page.repo.name, "blob", &page.rev.commit_hash, &page.path),
        download_href: crate::url::query(&raw_href, &[("download", "1")]),
        raw_href,
    }
    .render()
    .map_err(AppError::internal)
}

#[cfg(test)]
mod tests {
    use super::raw_content_type;
    #[test]
    fn raw_never_executes_html_or_svg() {
        assert_eq!(
            raw_content_type(b"<script>alert(1)</script>", true),
            "text/plain; charset=utf-8"
        );
        assert_eq!(
            raw_content_type(b"<svg onload='alert(1)'></svg>", true),
            "text/plain; charset=utf-8"
        );
        assert_eq!(raw_content_type(b"\x89PNG\r\n\x1a\n", false), "image/png");
        assert_eq!(
            raw_content_type(b"\0binary", false),
            "application/octet-stream"
        );
    }
}
