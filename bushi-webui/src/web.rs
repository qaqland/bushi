use axum::{Router, extract::Request, http::header, response::IntoResponse, routing::get};

use crate::data::{GitRepository, SqliteRepository};
use crate::error::AppError;
use crate::page;

#[derive(Clone)]
pub struct AppState {
    pub repo_port: SqliteRepository,
    pub git_port: GitRepository,
}

impl AppState {
    pub fn new(repo_port: SqliteRepository, git_port: GitRepository) -> Self {
        Self {
            repo_port,
            git_port,
        }
    }
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", get(page::repo_list::handler))
        .route("/static/bushi.css", get(css))
        .route("/{repo}", get(page::summary::handler))
        .route("/{repo}/-/refs", get(page::refs::handler))
        .route("/{repo}/-/tree/{*tail}", get(page::tree::handler))
        .route("/{repo}/-/blob/{*tail}", get(page::blob::handler))
        .route("/{repo}/-/raw/{*tail}", get(page::blob::raw_blob))
        .route("/{repo}/-/history/{*tail}", get(page::history::handler))
        .route("/{repo}/-/commit/{hash}", get(page::commit::handler))
        .fallback(not_found)
}

async fn css() -> impl IntoResponse {
    (
        [(header::CONTENT_TYPE, "text/css; charset=utf-8")],
        include_str!("../static/bushi.css"),
    )
}

async fn not_found(request: Request) -> impl IntoResponse {
    AppError::not_found(
        format!("path not found: {}", request.uri().path()),
        None,
        None,
    )
}
