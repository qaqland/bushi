use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
};
use thiserror::Error;

use crate::config::AppConfig;

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
    pub repo: Option<Box<crate::data::RepositoryRecord>>,
    pub config: Option<Box<AppConfig>>,
    pub rev: Option<String>,
    pub path: Option<String>,
    pub back_href: String,
}

impl AppError {
    pub fn not_found(
        message: impl Into<String>,
        repo: Option<crate::data::RepositoryRecord>,
    ) -> Self {
        Self {
            status: StatusCode::NOT_FOUND,
            message: message.into(),
            repo: repo.map(Box::new),
            config: None,
            rev: None,
            path: None,
            back_href: String::new(),
        }
    }

    pub fn bad_request(
        message: impl Into<String>,
        repo: Option<crate::data::RepositoryRecord>,
    ) -> Self {
        let mut error = Self::not_found(message, repo);
        error.status = StatusCode::BAD_REQUEST;
        error
    }

    pub fn internal(message: impl std::fmt::Display) -> Self {
        tracing::error!(error = %message, "internal request error");
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            message: "internal server error".to_string(),
            repo: None,
            config: None,
            rev: None,
            path: None,
            back_href: String::new(),
        }
    }

    pub fn from_domain(err: DomainError, repo: Option<crate::data::RepositoryRecord>) -> Self {
        let status = match &err {
            DomainError::RepoNotFound
            | DomainError::RevNotFound
            | DomainError::PathNotFound
            | DomainError::AmbiguousHash => StatusCode::NOT_FOUND,
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        };
        let message = if status == StatusCode::INTERNAL_SERVER_ERROR {
            tracing::error!(error = %err, "internal domain error");
            "internal server error".to_string()
        } else {
            err.to_string()
        };
        Self {
            status,
            message,
            repo: repo.map(Box::new),
            config: None,
            rev: None,
            path: None,
            back_href: String::new(),
        }
    }

    pub fn with_path(mut self, rev: &str, path: &str) -> Self {
        self.rev = Some(rev.to_string());
        self.path = Some(path.to_string());
        self
    }

    pub fn with_config(mut self, config: &AppConfig) -> Self {
        self.config = Some(Box::new(config.clone()));
        self
    }

    pub fn with_repo(mut self, repo: crate::data::RepositoryRecord) -> Self {
        self.repo = Some(Box::new(repo));
        self
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let status = self.status;
        let config = self.config.clone().unwrap_or_default();
        let html = crate::page::render_error(&self, &config);
        (status, [("content-type", "text/html; charset=utf-8")], html).into_response()
    }
}
