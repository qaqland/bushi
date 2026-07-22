use std::time::Instant;

use askama::Template;
use axum::{extract::Path, extract::State, response::Html};

use crate::data::{CommitInfo, GitPort, ReadmeView, RepositoryPort, RepositoryRecord, ResolvedRev};
use crate::error::{AppError, DomainError};
use crate::web::AppState;

use super::{PathView, RepoHeader, Tab, path_row, rev_indicator};

#[derive(Clone, Debug)]
pub struct SummaryPage {
    pub repo: RepositoryRecord,
    pub default_rev: String,
    pub rev: ResolvedRev,
    pub readme: Option<ReadmeView>,
    pub recent: Vec<CommitInfo>,
}

struct RecentCommit {
    href: String,
    short: String,
    subject: String,
    author: String,
    time: String,
}

#[derive(Template)]
#[template(path = "summary.html")]
struct SummaryTemplate {
    readme_html: Option<String>,
    recent: Vec<RecentCommit>,
    more_href: String,
}

const RECENT_COMMITS: usize = 10;

pub async fn handler(
    State(state): State<AppState>,
    Path(repo_name): Path<String>,
) -> Result<Html<String>, AppError> {
    let start = Instant::now();
    let page = load(&state.repo_port, &state.git_port, &repo_name)
        .await
        .map_err(|e| AppError::from_domain(e, None, None))?;
    let content = render_body(&page)?;
    let header = RepoHeader {
        name: page.repo.name.clone(),
        default_rev: page.default_rev.clone(),
        tab: Tab::Summary,
        rev: Some(rev_indicator(&page.repo.name, &page.rev)),
        object: None,
        path: Some(path_row(
            &page.repo.name,
            &page.default_rev,
            "",
            false,
            PathView::None,
        )),
    };
    let title = format!("{} - Summary", page.repo.name);
    super::render(&title, Some(&header), None, content, start).map(Html)
}

pub async fn load<R: RepositoryPort, G: GitPort>(
    repo_port: &R,
    git_port: &G,
    repo_name: &str,
) -> Result<SummaryPage, DomainError> {
    let repo = repo_port
        .get_repository(repo_name)
        .await?
        .ok_or(DomainError::RepoNotFound)?;
    let default_rev = repo_port.default_rev(&repo).await?;
    let rev = repo_port.resolve_rev(repo.id, &default_rev).await?;
    let readme = git_port
        .read_readme(repo.path.clone(), rev.commit_hash.clone())
        .await?;
    let history = repo_port
        .log(repo.id, &rev.commit_hash, None, RECENT_COMMITS)
        .await?;
    let recent = if history.hashes.is_empty() {
        Vec::new()
    } else {
        git_port
            .read_commits(repo.path.clone(), history.hashes)
            .await?
    };
    Ok(SummaryPage {
        repo,
        default_rev,
        rev,
        readme,
        recent,
    })
}

fn render_body(page: &SummaryPage) -> Result<String, AppError> {
    let repo_name = &page.repo.name;
    let recent = page
        .recent
        .iter()
        .map(|commit| RecentCommit {
            href: format!("/{repo_name}/-/commit/{}", commit.short_hash),
            short: commit.short_hash.clone(),
            subject: commit.subject.clone(),
            author: commit.author.clone(),
            time: commit.time_label.clone(),
        })
        .collect();
    SummaryTemplate {
        readme_html: page.readme.as_ref().map(|readme| readme.html.clone()),
        recent,
        more_href: format!("/{repo_name}/-/history/{}", page.default_rev),
    }
    .render()
    .map_err(AppError::internal)
}
