use askama::Template;
use axum::{
    extract::{Path, Query, State},
    response::{Html, IntoResponse, Redirect, Response},
};
use serde::Deserialize;

use super::{RepoHeader, require_repo};
use crate::data::{EntryKind, GitPort, RefRecord, RepositoryPort, RepositoryRecord, short_hash};
use crate::error::{AppError, DomainError};
use crate::format::utc_time_without_zone;
use crate::url::{display_rev, path_route, query, repo_route, valid_path};
use crate::web::AppState;

#[derive(Clone, Debug)]
pub struct RefsPage {
    pub repo: RepositoryRecord,
    pub branches: Vec<RefRecord>,
    pub tags: Vec<RefRecord>,
}

#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SwitchView {
    Tree,
    Blob,
    History,
}
impl SwitchView {
    fn op(self) -> &'static str {
        match self {
            Self::Tree => "tree",
            Self::Blob => "blob",
            Self::History => "history",
        }
    }
}

#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RefKind {
    Branches,
    Tags,
}

#[derive(Default, Deserialize)]
pub struct RefsQuery {
    kind: Option<RefKind>,
    rev: Option<String>,
    #[serde(default)]
    path: String,
    view: Option<SwitchView>,
    to: Option<String>,
}

struct RefRow {
    name: String,
    href: String,
    commit_href: String,
    short: String,
    time: String,
    current: bool,
    default: bool,
}

#[derive(Template)]
#[template(path = "refs.html")]
struct RefsTemplate {
    title: &'static str,
    show_branches: bool,
    show_tags: bool,
    branches: Vec<RefRow>,
    tags: Vec<RefRow>,
    switching: bool,
    context: String,
    cancel_href: String,
}

pub async fn handler(
    State(state): State<AppState>,
    Path(repo_name): Path<String>,
    input: Result<Query<RefsQuery>, axum::extract::rejection::QueryRejection>,
) -> Result<Response, AppError> {
    let result = async {
        let repo = require_repo(&state, &repo_name).await?;
        let Query(input) = input.map_err(|_| {
            AppError::bad_request("Invalid version switch parameters.", Some(repo.clone()))
        })?;
        if !valid_path(&input.path) || (input.to.is_some() && input.view.is_none()) {
            return Err(AppError::bad_request(
                "Invalid version switch context.",
                Some(repo),
            ));
        }
        let default = state
            .repo_port
            .default_rev(&repo)
            .await
            .map_err(|e| AppError::from_domain(e, Some(repo.clone())))?;
        let source = input.rev.as_deref().unwrap_or(&default);
        if let (Some(target), Some(view)) = (&input.to, input.view) {
            let back_href = path_route(&repo_name, view.op(), source, &input.path);
            let selection = async {
                let rev = state.repo_port.resolve_rev(repo.id, target).await?;
                let op = if matches!(view, SwitchView::History) {
                    "history"
                } else {
                    match state
                        .git_port
                        .path_kind(repo.path.clone(), rev.commit_hash, input.path.clone())
                        .await?
                    {
                        Some(EntryKind::Tree) => "tree",
                        Some(EntryKind::Blob) => "blob",
                        _ => return Err(DomainError::PathNotFound),
                    }
                };
                Ok::<_, DomainError>(
                    Redirect::temporary(&path_route(&repo_name, op, target, &input.path))
                        .into_response(),
                )
            }
            .await;
            return selection.map_err(|e| {
                let mut error =
                    AppError::from_domain(e, Some(repo.clone())).with_path(target, &input.path);
                error.back_href = back_href;
                error
            });
        }
        let page = load(&state.repo_port, &repo_name)
            .await
            .map_err(|e| AppError::from_domain(e, Some(repo)))?;
        let (title, active) = match (input.view, input.kind) {
            (Some(_), _) => ("Switch version", ""),
            (None, Some(RefKind::Branches)) => ("Branches", "branches"),
            (None, Some(RefKind::Tags)) => ("Tags", "tags"),
            (None, None) => ("Branches & tags", ""),
        };
        let header = RepoHeader::new(&repo_name, Some(source), active);
        let content = render_body(&page, &input, source, &default, title)?;
        super::render(
            &state.config,
            &format!("{repo_name} - {title}"),
            Some(&header),
            None,
            content,
        )
        .map(|html| Html(html).into_response())
    }
    .await;
    result.map_err(|e: AppError| e.with_config(&state.config))
}

pub async fn load<R: RepositoryPort>(
    repo_port: &R,
    repo_name: &str,
) -> Result<RefsPage, DomainError> {
    let repo = repo_port
        .get_repository(repo_name)
        .await?
        .ok_or(DomainError::RepoNotFound)?;
    let branches = repo_port.list_refs(repo.id, 0).await?;
    let tags = repo_port.list_refs(repo.id, 1).await?;
    Ok(RefsPage {
        repo,
        branches,
        tags,
    })
}

fn render_body(
    page: &RefsPage,
    input: &RefsQuery,
    source: &str,
    default: &str,
    title: &'static str,
) -> Result<String, AppError> {
    let _timer = crate::debug_timing::start("render.body");
    let repo = &page.repo.name;
    let rows = |refs: &[RefRecord], tags: bool| {
        refs.iter()
            .map(|reference| {
                let rev = if tags {
                    format!("tag/{}", reference.show_name)
                } else {
                    reference.show_name.clone()
                };
                let href = if let Some(view) = input.view {
                    query(
                        &format!("{}/-/refs", repo_route(repo)),
                        &[
                            ("rev", source),
                            ("path", &input.path),
                            ("view", view.op()),
                            ("to", &rev),
                        ],
                    )
                } else {
                    path_route(repo, "tree", &rev, "")
                };
                RefRow {
                    name: display_rev(&rev),
                    href,
                    commit_href: path_route(repo, "commit", &reference.commit_hash, ""),
                    short: short_hash(&reference.commit_hash),
                    time: utc_time_without_zone(reference.time),
                    current: rev == source,
                    default: rev == default,
                }
            })
            .collect()
    };
    RefsTemplate {
        title,
        show_branches: input.view.is_some() || !matches!(input.kind, Some(RefKind::Tags)),
        show_tags: input.view.is_some() || !matches!(input.kind, Some(RefKind::Branches)),
        branches: rows(&page.branches, false),
        tags: rows(&page.tags, true),
        switching: input.view.is_some(),
        context: format!(
            "Continue viewing {} of {}. Current version: {}.",
            if matches!(input.view, Some(SwitchView::History)) {
                "history"
            } else {
                "content"
            },
            if input.path.is_empty() {
                "the repository root"
            } else {
                &input.path
            },
            display_rev(source)
        ),
        cancel_href: input
            .view
            .map(|view| path_route(repo, view.op(), source, &input.path))
            .unwrap_or_default(),
    }
    .render()
    .map_err(AppError::internal)
}
