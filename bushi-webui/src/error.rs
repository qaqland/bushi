use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum DomainError {
    #[error("repository not found")]
    RepoNotFound,
    #[error("revision not found")]
    RevNotFound,
    #[error("path not found")]
    PathNotFound,
    #[error("ambiguous commit hash")]
    AmbiguousHash,
    #[error("sqlite error: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("git error: {0}")]
    Git(#[from] git2::Error),
    #[error("database pool error: {0}")]
    SqlitePool(#[from] deadpool_sqlite::PoolError),
    #[error("database worker error: {0}")]
    SqliteInteract(#[from] deadpool_sqlite::InteractError),
    #[error("background task error: {0}")]
    Join(#[from] tokio::task::JoinError),
    #[error("internal error: {0}")]
    Internal(String),
}

#[derive(Clone, Debug)]
pub struct AppError {
    pub status: StatusCode,
    pub message: String,
    pub repo: Option<crate::data::RepositoryRecord>,
    pub default_rev: Option<String>,
}

impl AppError {
    pub fn not_found(
        message: impl Into<String>,
        repo: Option<crate::data::RepositoryRecord>,
        default_rev: Option<String>,
    ) -> Self {
        Self {
            status: StatusCode::NOT_FOUND,
            message: message.into(),
            repo,
            default_rev,
        }
    }

    pub fn internal(message: impl std::fmt::Display) -> Self {
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            message: message.to_string(),
            repo: None,
            default_rev: None,
        }
    }

    pub fn from_domain(
        err: DomainError,
        repo: Option<crate::data::RepositoryRecord>,
        default_rev: Option<String>,
    ) -> Self {
        let status = match &err {
            DomainError::RepoNotFound
            | DomainError::RevNotFound
            | DomainError::PathNotFound
            | DomainError::AmbiguousHash => StatusCode::NOT_FOUND,
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        };
        Self {
            status,
            message: err.to_string(),
            repo,
            default_rev,
        }
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let status = self.status;
        let html = crate::page::render_error(&self);
        (status, [("content-type", "text/html; charset=utf-8")], html).into_response()
    }
}
