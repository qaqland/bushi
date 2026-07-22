use std::{env, net::SocketAddr};

use anyhow::Result;
use tokio::net::TcpListener;
use tracing_subscriber::EnvFilter;

use bushi_webui::data::{GitRepository, SqliteRepository};
use bushi_webui::web::{AppState, router};

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    let database = env::var("BUSHI_DATABASE").unwrap_or_else(|_| "test.db".to_string());
    let bind = env::var("BUSHI_BIND").unwrap_or_else(|_| "127.0.0.1:3000".to_string());

    let repo_port = SqliteRepository::new(database)?;
    let git_port = GitRepository::new();
    let state = AppState::new(repo_port, git_port);

    let addr: SocketAddr = bind.parse()?;
    let listener = TcpListener::bind(addr).await?;
    tracing::info!(%addr, "listening");
    axum::serve(listener, router().with_state(state))
        .with_graceful_shutdown(shutdown_signal())
        .await?;
    Ok(())
}

async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
    tracing::info!("signal received, starting graceful shutdown");
}
