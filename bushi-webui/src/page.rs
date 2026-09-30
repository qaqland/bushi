use askama::Template;

use crate::config::{AppConfig, HeaderLink};
use crate::data::{RepositoryPort, RepositoryRecord, ResolvedRev, short_hash};
use crate::debug_timing;
use crate::error::AppError;
pub use crate::url::path_route;
use crate::url::{display_rev, query, repo_route, valid_path};
use crate::web::AppState;

pub mod blob;
pub mod commit;
pub mod history;
pub mod refs;
pub mod repo_list;
pub mod summary;
pub mod tree;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum PathView {
    None,
    Browse,
    History,
}

#[derive(Clone)]
pub struct RevIndicator {
    pub kind: &'static str,
    pub name: String,
    pub short_hash: String,
    pub hash_href: String,
    pub switch_href: String,
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
    pub home_href: String,
    pub files_href: String,
    pub refs_href: String,
    pub branches_href: String,
    pub tags_href: String,
    pub active: &'static str,
    pub rev: Option<RevIndicator>,
    pub path: Option<PathRow>,
}

impl RepoHeader {
    pub fn new(name: &str, rev: Option<&str>, active: &'static str) -> Self {
        let home_href = repo_route(name);
        let refs_href = format!("{home_href}/-/refs");
        let listing_href = |kind| {
            let mut pairs = vec![("kind", kind)];
            if let Some(rev) = rev {
                pairs.push(("rev", rev));
            }
            query(&refs_href, &pairs)
        };
        Self {
            name: name.to_string(),
            files_href: rev
                .map(|rev| path_route(name, "tree", rev, ""))
                .unwrap_or_else(|| format!("{home_href}/-/tree")),
            branches_href: listing_href("branches"),
            tags_href: listing_href("tags"),
            refs_href: rev
                .map(|rev| query(&refs_href, &[("rev", rev)]))
                .unwrap_or(refs_href),
            home_href,
            active,
            rev: None,
            path: None,
        }
    }
}

pub struct RepoContext {
    pub repo: RepositoryRecord,
    pub default_rev: String,
    pub rev: ResolvedRev,
}

#[derive(Template)]
#[template(path = "header.html")]
struct HeaderTemplate<'a> {
    header: Option<&'a RepoHeader>,
    links: &'a [HeaderLink],
}

#[derive(Template)]
#[template(path = "page.html")]
struct PageTemplate {
    title: String,
    rev: Option<RevIndicator>,
    header_html: String,
    alternate_markdown_href: String,
    alternate_text_href: String,
    content_html: String,
    generated: String,
    version: &'static str,
}

#[derive(Template)]
#[template(path = "error.html")]
struct ErrorTemplate {
    status: u16,
    reason: String,
    message: String,
    location: String,
    root_href: String,
    refs_href: String,
    back_href: String,
}

pub fn render(
    config: &AppConfig,
    title: &str,
    header: Option<&RepoHeader>,
    alternate: Option<(&str, &str)>,
    content_html: String,
) -> Result<String, AppError> {
    let header_html = {
        let _timer = debug_timing::start("render.header");
        HeaderTemplate {
            header,
            links: &config.header_links,
        }
        .render()
        .map_err(AppError::internal)?
    };
    let (alternate_markdown_href, alternate_text_href) = match alternate {
        Some((md, txt)) => (md.to_string(), txt.to_string()),
        None => (String::new(), String::new()),
    };
    let page = PageTemplate {
        title: title.to_string(),
        rev: header.and_then(|header| header.rev.clone()),
        header_html,
        alternate_markdown_href,
        alternate_text_href,
        content_html,
        generated: debug_timing::request_elapsed()
            .map(|elapsed| format!("{:.3}", elapsed.as_secs_f64()))
            .unwrap_or_default(),
        version: env!("CARGO_PKG_VERSION"),
    };
    let _timer = debug_timing::start("render.layout");
    page.render().map_err(AppError::internal)
}

pub fn render_error(error: &AppError, config: &AppConfig) -> String {
    let header = error
        .repo
        .as_ref()
        .map(|repo| RepoHeader::new(&repo.name, error.rev.as_deref(), ""));
    let body = {
        let _timer = debug_timing::start("render.error_body");
        ErrorTemplate {
            status: error.status.as_u16(),
            reason: error
                .status
                .canonical_reason()
                .unwrap_or("Error")
                .to_string(),
            message: error.message.clone(),
            location: error
                .rev
                .as_ref()
                .map(|rev| {
                    format!(
                        "{} at {}",
                        error
                            .path
                            .as_deref()
                            .filter(|p| !p.is_empty())
                            .unwrap_or("Repository root"),
                        display_rev(rev)
                    )
                })
                .unwrap_or_default(),
            root_href: header
                .as_ref()
                .map(|h| h.files_href.clone())
                .unwrap_or_else(|| "/".to_string()),
            refs_href: header
                .as_ref()
                .map(|h| h.refs_href.clone())
                .unwrap_or_default(),
            back_href: error.back_href.clone(),
        }
        .render()
        .unwrap_or_else(|_| "template error".to_string())
    };
    render(config, "Error", header.as_ref(), None, body).unwrap_or_else(|_| {
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
        .map_err(|e| AppError::from_domain(e, Some(repo.clone())))?;
    let rev = state
        .repo_port
        .resolve_rev(repo.id, &default_rev)
        .await
        .map_err(|e| AppError::from_domain(e, Some(repo.clone())))?;
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
    repo_context_at_rev_for_repo(state, repo, rev_name).await
}

pub async fn repo_context_at_rev_for_repo(
    state: &AppState,
    repo: RepositoryRecord,
    rev_name: &str,
) -> Result<RepoContext, AppError> {
    let default_rev = state
        .repo_port
        .default_rev(&repo)
        .await
        .map_err(|e| AppError::from_domain(e, Some(repo.clone())))?;
    let rev = state
        .repo_port
        .resolve_rev(repo.id, rev_name)
        .await
        .map_err(|e| AppError::from_domain(e, Some(repo.clone())))?;
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
        .ok_or_else(|| AppError::not_found("repository not found", None))
}

pub fn split_tail(tail: &str, require_path: bool) -> Result<(String, String), AppError> {
    let parts: Vec<&str> = tail.split('/').filter(|part| !part.is_empty()).collect();
    if parts.is_empty() {
        return Err(AppError::not_found("revision not found", None));
    }

    let (rev, path_start) = if parts[0] == "tag" {
        if parts.len() < 2 {
            return Err(AppError::not_found("tag revision not found", None));
        }
        (format!("tag/{}", parts[1]), 2)
    } else {
        (parts[0].to_string(), 1)
    };
    let path = parts[path_start..].join("/");
    if !valid_path(&path) {
        return Err(AppError::not_found("invalid repository path", None));
    }
    if require_path && path.is_empty() {
        return Err(AppError::not_found("path not found", None));
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

pub fn rev_indicator(repo: &str, rev: &ResolvedRev, path: &str, op: &str) -> RevIndicator {
    let short = short_hash(&rev.commit_hash);
    let is_commit = rev.input.len() >= 8 && rev.input.bytes().all(|b| b.is_ascii_hexdigit());
    RevIndicator {
        kind: if rev.input.starts_with("tag/") {
            "Tag"
        } else if is_commit {
            "Commit"
        } else {
            "Branch"
        },
        name: if is_commit {
            short.clone()
        } else {
            display_rev(&rev.input)
        },
        short_hash: short,
        hash_href: path_route(repo, "commit", &rev.commit_hash, ""),
        switch_href: query(
            &format!("{}/-/refs", repo_route(repo)),
            &[("rev", &rev.input), ("path", path), ("view", op)],
        ),
    }
}

pub fn path_row(repo: &str, rev: &str, path: &str, is_file: bool, active: PathView) -> PathRow {
    let mut crumbs = vec![Crumb {
        label: "Root".to_string(),
        href: path_route(
            repo,
            if active == PathView::History {
                "history"
            } else {
                "tree"
            },
            rev,
            "",
        ),
    }];
    let parts: Vec<&str> = path.split('/').filter(|part| !part.is_empty()).collect();
    let mut current = String::new();
    for (idx, part) in parts.iter().enumerate() {
        if !current.is_empty() {
            current.push('/');
        }
        current.push_str(part);
        let last = idx + 1 == parts.len();
        let href = if active == PathView::History {
            path_route(repo, "history", rev, &current)
        } else if last && is_file {
            path_route(repo, "blob", rev, &current)
        } else {
            path_route(repo, "tree", rev, &current)
        };
        crumbs.push(Crumb {
            label: part.to_string(),
            href,
        });
    }

    let views = vec![ViewLink {
        label: "Content".to_string(),
        href: path_route(repo, if is_file { "blob" } else { "tree" }, rev, path),
        active: active == PathView::Browse,
    }];
    let history = ViewLink {
        label: "History".to_string(),
        href: path_route(repo, "history", rev, path),
        active: active == PathView::History,
    };
    PathRow {
        crumbs,
        views,
        history,
    }
}
