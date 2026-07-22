use axum::{
    Router,
    extract::{Request, State},
    http::header,
    middleware,
    response::IntoResponse,
    routing::get,
};

use crate::config::AppConfig;
use crate::data::{GitRepository, RepositoryPort, SqliteRepository};
use crate::error::AppError;
use crate::page;

#[derive(Clone)]
pub struct AppState {
    pub repo_port: SqliteRepository,
    pub git_port: GitRepository,
    pub config: AppConfig,
}

impl AppState {
    pub fn new(repo_port: SqliteRepository, git_port: GitRepository, config: AppConfig) -> Self {
        Self {
            repo_port,
            git_port,
            config,
        }
    }
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", get(page::repo_list::handler))
        .route("/static/bushi.css", get(css))
        .route("/{repo}", get(page::summary::handler))
        .route("/{repo}/-/refs", get(page::refs::handler))
        .route("/{repo}/-/tree", get(page::tree::default_handler))
        .route("/{repo}/-/tree/{*tail}", get(page::tree::handler))
        .route("/{repo}/-/blob/{*tail}", get(page::blob::handler))
        .route("/{repo}/-/raw/{*tail}", get(page::blob::raw_blob))
        .route("/{repo}/-/history/{*tail}", get(page::history::handler))
        .route("/{repo}/-/commit/{hash}", get(page::commit::handler))
        .fallback(not_found)
        .layer(middleware::from_fn(crate::debug_timing::scope_request))
}

async fn css() -> impl IntoResponse {
    (
        [(header::CONTENT_TYPE, "text/css; charset=utf-8")],
        include_str!("../static/bushi.css"),
    )
}

async fn not_found(State(state): State<AppState>, request: Request) -> impl IntoResponse {
    let repo_name = request
        .uri()
        .path()
        .trim_start_matches('/')
        .split('/')
        .next()
        .filter(|name| !name.is_empty());
    let repo_name = repo_name.and_then(crate::url::decode);
    let repo = match repo_name {
        Some(name) => state.repo_port.get_repository(&name).await.ok().flatten(),
        None => None,
    };
    AppError::not_found(format!("path not found: {}", request.uri().path()), repo)
        .with_config(&state.config)
}
