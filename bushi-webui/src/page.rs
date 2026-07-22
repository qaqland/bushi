use std::time::Instant;

use askama::Template;

use crate::data::{RepositoryPort, RepositoryRecord, ResolvedRev, short_hash};
use crate::error::AppError;
use crate::web::AppState;

pub mod blob;
pub mod commit;
pub mod history;
pub mod refs;
pub mod repo_list;
pub mod summary;
pub mod tree;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    None,
    Summary,
    Files,
    Refs,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum PathView {
    None,
    Browse,
    History,
}

#[derive(Clone)]
pub struct RevIndicator {
    pub symbol: String,
    pub name: String,
    pub name_href: String,
}

#[derive(Clone)]
pub struct ObjectRow {
    pub short_hash: String,
    pub subject: String,
    pub commit_href: String,
    pub patch_href: String,
}

#[derive(Clone)]
pub struct Crumb {
    pub label: String,
    pub href: String,
}

#[derive(Clone)]
pub struct ViewLink {
    pub label: String,
    pub href: String,
    pub active: bool,
}

#[derive(Clone)]
pub struct PathRow {
    pub crumbs: Vec<Crumb>,
    pub views: Vec<ViewLink>,
    pub history: ViewLink,
}

#[derive(Clone)]
pub struct RepoHeader {
    pub name: String,
    pub default_rev: String,
    pub tab: Tab,
    pub rev: Option<RevIndicator>,
    pub object: Option<ObjectRow>,
    pub path: Option<PathRow>,
}

pub struct RepoContext {
    pub repo: RepositoryRecord,
    pub default_rev: String,
    pub rev: ResolvedRev,
}

#[derive(Template)]
#[template(path = "header.html")]
struct HeaderTemplate<'a> {
    header: &'a RepoHeader,
}

#[derive(Template)]
#[template(path = "page.html")]
struct PageTemplate {
    title: String,
    header_html: String,
    alternate_markdown_href: String,
    alternate_text_href: String,
    content_html: String,
    render_ms: u128,
}

#[derive(Template)]
#[template(path = "error.html")]
struct ErrorTemplate {
    status: u16,
    reason: String,
    message: String,
}

pub fn render(
    title: &str,
    header: Option<&RepoHeader>,
    alternate: Option<(&str, &str)>,
    content_html: String,
    start: Instant,
) -> Result<String, AppError> {
    let header_html = match header {
        Some(header) => HeaderTemplate { header }
            .render()
            .map_err(AppError::internal)?,
        None => String::new(),
    };
    let (alternate_markdown_href, alternate_text_href) = match alternate {
        Some((md, txt)) => (md.to_string(), txt.to_string()),
        None => (String::new(), String::new()),
    };
    let page = PageTemplate {
        title: title.to_string(),
        header_html,
        alternate_markdown_href,
        alternate_text_href,
        content_html,
        render_ms: start.elapsed().as_millis(),
    };
    page.render().map_err(AppError::internal)
}

pub fn render_error(error: &AppError) -> String {
    let header = error.repo.as_ref().map(|repo| RepoHeader {
        name: repo.name.clone(),
        default_rev: error
            .default_rev
            .clone()
            .unwrap_or_else(|| "main".to_string()),
        tab: Tab::None,
        rev: None,
        object: None,
        path: None,
    });
    let body = ErrorTemplate {
        status: error.status.as_u16(),
        reason: error
            .status
            .canonical_reason()
            .unwrap_or("Error")
            .to_string(),
        message: error.message.clone(),
    }
    .render()
    .unwrap_or_else(|_| "template error".to_string());
    render("Error", header.as_ref(), None, body, Instant::now()).unwrap_or_else(|_| {
        format!(
            "<!doctype html><html><body><h1>Error {}</h1></body></html>",
            error.status.as_u16()
        )
    })
}

pub async fn repo_context(state: &AppState, repo_name: &str) -> Result<RepoContext, AppError> {
    let repo = require_repo(state, repo_name).await?;
    let default_rev = state
        .repo_port
        .default_rev(&repo)
        .await
        .map_err(AppError::internal)?;
    let rev = state
        .repo_port
        .resolve_rev(repo.id, &default_rev)
        .await
        .map_err(|e| AppError::from_domain(e, Some(repo.clone()), Some(default_rev.clone())))?;
    Ok(RepoContext {
        repo,
        default_rev,
        rev,
    })
}

pub async fn repo_context_at_rev(
    state: &AppState,
    repo_name: &str,
    rev_name: &str,
) -> Result<RepoContext, AppError> {
    let repo = require_repo(state, repo_name).await?;
    let default_rev = state
        .repo_port
        .default_rev(&repo)
        .await
        .map_err(AppError::internal)?;
    let rev = state
        .repo_port
        .resolve_rev(repo.id, rev_name)
        .await
        .map_err(|e| AppError::from_domain(e, Some(repo.clone()), Some(default_rev.clone())))?;
    Ok(RepoContext {
        repo,
        default_rev,
        rev,
    })
}

pub async fn require_repo(state: &AppState, repo_name: &str) -> Result<RepositoryRecord, AppError> {
    state
        .repo_port
        .get_repository(repo_name)
        .await
        .map_err(AppError::internal)?
        .ok_or_else(|| AppError::not_found("repository not found", None, None))
}

pub fn split_tail(tail: &str, require_path: bool) -> Result<(String, String), AppError> {
    let parts: Vec<&str> = tail.split('/').filter(|part| !part.is_empty()).collect();
    if parts.is_empty() {
        return Err(AppError::not_found("revision not found", None, None));
    }

    let (rev, path_start) = if parts[0] == "tag" {
        if parts.len() < 2 {
            return Err(AppError::not_found("tag revision not found", None, None));
        }
        (format!("tag/{}", parts[1]), 2)
    } else {
        (parts[0].to_string(), 1)
    };
    let path = parts[path_start..].join("/");
    if require_path && path.is_empty() {
        return Err(AppError::not_found("path not found", None, None));
    }
    Ok((rev, path))
}

pub fn join_path(parent: &str, child: &str) -> String {
    if parent.is_empty() {
        child.to_string()
    } else {
        format!("{parent}/{child}")
    }
}

pub fn parent_path(path: &str) -> String {
    path.rsplit_once('/')
        .map(|(parent, _)| parent.to_string())
        .unwrap_or_default()
}

pub fn path_route(repo: &str, op: &str, rev: &str, path: &str) -> String {
    if path.is_empty() {
        format!("/{repo}/-/{op}/{rev}")
    } else {
        format!("/{repo}/-/{op}/{rev}/{path}")
    }
}

pub fn rev_indicator(repo: &str, rev: &ResolvedRev) -> RevIndicator {
    let short = short_hash(&rev.commit_hash);
    let commit_href = format!("/{repo}/-/commit/{short}");
    if let Some(tag) = rev.input.strip_prefix("tag/") {
        RevIndicator {
            symbol: "=".to_string(),
            name: tag.to_string(),
            name_href: path_route(repo, "tree", &rev.input, ""),
        }
    } else if rev.input.len() >= 8 && rev.input.bytes().all(|b| b.is_ascii_hexdigit()) {
        RevIndicator {
            symbol: "@".to_string(),
            name: short,
            name_href: commit_href,
        }
    } else {
        RevIndicator {
            symbol: "~".to_string(),
            name: rev.input.clone(),
            name_href: path_route(repo, "tree", &rev.input, ""),
        }
    }
}

pub fn commit_rev_indicator(repo: &str, hash: &str) -> RevIndicator {
    let short = short_hash(hash);
    RevIndicator {
        symbol: "@".to_string(),
        name: short.clone(),
        name_href: format!("/{repo}/-/commit/{short}"),
    }
}

pub fn path_row(repo: &str, rev: &str, path: &str, is_file: bool, active: PathView) -> PathRow {
    let mut crumbs = vec![Crumb {
        label: "root".to_string(),
        href: path_route(repo, "tree", rev, ""),
    }];
    let parts: Vec<&str> = path.split('/').filter(|part| !part.is_empty()).collect();
    let mut current = String::new();
    for (idx, part) in parts.iter().enumerate() {
        if !current.is_empty() {
            current.push('/');
        }
        current.push_str(part);
        let last = idx + 1 == parts.len();
        let href = if last && is_file {
            path_route(repo, "blob", rev, &current)
        } else {
            path_route(repo, "tree", rev, &current)
        };
        crumbs.push(Crumb {
            label: part.to_string(),
            href,
        });
    }

    let views = if is_file {
        vec![
            ViewLink {
                label: "browse".to_string(),
                href: path_route(repo, "blob", rev, path),
                active: active == PathView::Browse,
            },
            ViewLink {
                label: "raw".to_string(),
                href: path_route(repo, "raw", rev, path),
                active: false,
            },
        ]
    } else {
        vec![ViewLink {
            label: "browse".to_string(),
            href: path_route(repo, "tree", rev, path),
            active: active == PathView::Browse,
        }]
    };
    let history = ViewLink {
        label: "history".to_string(),
        href: path_route(repo, "history", rev, path),
        active: active == PathView::History,
    };
    PathRow {
        crumbs,
        views,
        history,
    }
}
