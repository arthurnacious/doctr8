mod common;
mod config;
mod modules;
mod state;

use anyhow::Context;
use sqlx::postgres::PgPoolOptions;
use tower_http::trace::TraceLayer;
use tracing_subscriber::EnvFilter;

use crate::{config::Config, state::AppState};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| "info,doctr8_api=debug".into()),
        )
        .init();

    let config = Config::from_env()?;

    let pool = PgPoolOptions::new()
        .max_connections(config.db_max_connections)
        .connect(&config.database_url)
        .await
        .context("connecting to database")?;

    // Fail fast if the Rust Permission enum and the permissions table disagree.
    common::permission::verify_catalogue(&pool).await?;

    let bind_addr = config.bind_addr.clone();
    // Spans record method + path only. Query strings and headers are never
    // logged: they can carry patient details or tokens.
    let trace = TraceLayer::new_for_http().make_span_with(|req: &axum::http::Request<_>| {
        tracing::info_span!("http", method = %req.method(), path = %req.uri().path())
    });
    let app = modules::router()
        .with_state(AppState::new(pool, config))
        .layer(trace);

    let listener = tokio::net::TcpListener::bind(&bind_addr)
        .await
        .with_context(|| format!("binding {bind_addr}"))?;
    tracing::info!(addr = %bind_addr, "doctr8 api listening");

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;
    Ok(())
}

async fn shutdown_signal() {
    let ctrl_c = async {
        let _ = tokio::signal::ctrl_c().await;
    };
    #[cfg(unix)]
    let terminate = async {
        if let Ok(mut s) = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
        {
            s.recv().await;
        }
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
}
