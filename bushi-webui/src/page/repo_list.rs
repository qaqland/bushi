use std::time::Instant;

use askama::Template;
use axum::{extract::State, response::Html};

use crate::data::RepositoryPort;
use crate::error::{AppError, DomainError};
use crate::format::relative_time;
use crate::web::AppState;

#[derive(Clone, Debug)]
pub struct RepoListRow {
    pub name: String,
    pub description: String,
    pub updated: Option<i64>,
}

struct RepoRow {
    href: String,
    name: String,
    description: String,
    updated: String,
}

#[derive(Template)]
#[template(path = "repo_list.html")]
struct RepoListTemplate {
    rows: Vec<RepoRow>,
}

pub async fn handler(State(state): State<AppState>) -> Result<Html<String>, AppError> {
    let start = Instant::now();
    let rows = load(&state.repo_port).await.map_err(AppError::internal)?;
    let content = render_body(&rows)?;
    super::render("bushi", None, None, content, start).map(Html)
}

pub async fn load<R: RepositoryPort>(repo_port: &R) -> Result<Vec<RepoListRow>, DomainError> {
    let repos = repo_port.list_repositories().await?;
    let mut rows = Vec::new();
    for repo in repos {
        let default_rev = repo_port
            .default_rev(&repo)
            .await
            .unwrap_or_else(|_| "master".to_string());
        let updated = repo_port
            .ref_time(repo.id, &default_rev)
            .await
            .ok()
            .flatten();
        let description = read_description(&repo.path);
        rows.push(RepoListRow {
            name: repo.name,
            description,
            updated,
        });
    }
    Ok(rows)
}

fn render_body(rows: &[RepoListRow]) -> Result<String, AppError> {
    let rows = rows
        .iter()
        .map(|row| RepoRow {
            href: format!("/{}", row.name),
            name: row.name.clone(),
            description: row.description.clone(),
            updated: row
                .updated
                .map(relative_time)
                .unwrap_or_else(|| "--".to_string()),
        })
        .collect();
    RepoListTemplate { rows }
        .render()
        .map_err(AppError::internal)
}

fn read_description(repo_path: &str) -> String {
    let path = std::path::Path::new(repo_path).join("description");
    let Ok(content) = std::fs::read_to_string(path) else {
        return String::new();
    };
    let description = content.trim();
    if description.is_empty() {
        return String::new();
    }
    description.to_string()
}
