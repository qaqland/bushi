use askama::Template;
use axum::{extract::Path, extract::State, response::Html};

use crate::data::{CommitInfo, GitPort, ReadmeView, RepositoryPort, RepositoryRecord, ResolvedRev};
use crate::error::{AppError, DomainError};
use crate::web::AppState;

use super::{RepoHeader, path_route};

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
    name: String,
    description: String,
    browse_href: String,
    default_rev: String,
    readme_html: Option<String>,
    recent: Vec<RecentCommit>,
    more_href: String,
}

const RECENT_COMMITS: usize = 5;

pub async fn handler(
    State(state): State<AppState>,
    Path(repo_name): Path<String>,
) -> Result<Html<String>, AppError> {
    let config = state.config.clone();
    let result = async {
        let repo = super::require_repo(&state, &repo_name).await?;
        let page = load(&state.repo_port, &state.git_port, &repo_name)
            .await
            .map_err(|e| AppError::from_domain(e, Some(repo)))?;
        let content = render_body(&page)?;
        let header = RepoHeader::new(&page.repo.name, Some(&page.default_rev), "overview");
        let title = format!("{} - Overview", page.repo.name);
        super::render(&config, &title, Some(&header), None, content).map(Html)
    }
    .await;
    result.map_err(|error| error.with_config(&config))
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
        .read_readme(repo.path.clone(), rev.clone(), repo.name.clone())
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
    let _timer = crate::debug_timing::start("render.body");
    let repo_name = &page.repo.name;
    let recent = page
        .recent
        .iter()
        .map(|commit| RecentCommit {
            href: path_route(repo_name, "commit", &commit.hash, ""),
            short: commit.short_hash.clone(),
            subject: commit.subject.clone(),
            author: commit.author.clone(),
            time: commit.time_label.clone(),
        })
        .collect();
    SummaryTemplate {
        name: page.repo.name.clone(),
        description: super::repo_list::read_description(&page.repo.path),
        browse_href: path_route(repo_name, "tree", &page.default_rev, ""),
        default_rev: crate::url::display_rev(&page.default_rev),
        readme_html: page.readme.as_ref().map(|readme| readme.html.clone()),
        recent,
        more_href: path_route(repo_name, "history", &page.default_rev, ""),
    }
    .render()
    .map_err(AppError::internal)
}
