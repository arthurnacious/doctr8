mod common;
mod config;
mod modules;
mod state;

#[cfg(test)]
mod route_tests;

use anyhow::Context;
use sqlx::postgres::PgPoolOptions;
use tower_http::trace::TraceLayer;
use tracing_subscriber::EnvFilter;

use crate::{config::Config, state::AppState};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Local development convenience: load backend/.env if present. Variables
    // already set in the environment always win.
    let _ = dotenvy::dotenv();

    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| "info,doctr8_api=debug".into()),
        )
        .init();

    // `doctr8-api migrate` applies pending migrations as the schema owner,
    // never as the runtime role.
    if std::env::args().nth(1).as_deref() == Some("migrate") {
        return migrate().await;
    }

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

/// Apply pending migrations from ./migrations (embedded at build time).
/// Tracks state in `_sqlx_migrations`, so it's interchangeable with
/// `sqlx migrate run`.
async fn migrate() -> anyhow::Result<()> {
    let url = std::env::var("MIGRATION_DATABASE_URL")
        .context("MIGRATION_DATABASE_URL must be set (schema owner, not doctr8_app)")?;
    let pool = PgPoolOptions::new()
        .max_connections(1)
        .connect(&url)
        .await
        .context("connecting as migration user")?;
    sqlx::migrate!("./migrations").run(&pool).await.context("applying migrations")?;
    tracing::info!("migrations up to date");
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
