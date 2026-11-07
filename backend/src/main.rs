mod common;
mod config;
mod modules;
mod state;

#[cfg(test)]
mod route_tests;
#[cfg(test)]
mod ws_tests;

use anyhow::Context;
use sqlx::postgres::PgPoolOptions;
use tower_http::trace::TraceLayer;
use tracing_subscriber::EnvFilter;

use crate::{config::Config, state::AppState};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Local development convenience: load backend/.env if present. Variables
    // already set in the environment always win.
    // Look in the working directory (and its parents) first, then fall back
    // to the crate's own backend/.env so running from the repo root works.
    if dotenvy::dotenv().is_err() {
        let _ = dotenvy::from_path(concat!(env!("CARGO_MANIFEST_DIR"), "/.env"));
    }

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

    let role = config.database_role.clone();
    let pool = PgPoolOptions::new()
        .max_connections(config.db_max_connections)
        .after_connect(move |conn, _meta| {
            let role = role.clone();
            Box::pin(async move {
                if let Some(role) = role {
                    // Equivalent to SET ROLE, but parameterised.
                    sqlx::query("SELECT set_config('role', $1, false)").bind(role).execute(conn).await?;
                }
                Ok(())
            })
        })
        .connect(&config.database_url)
        .await
        .context("connecting to database")?;

    // Refuse to run with row-level security bypassed: tenant isolation
    // depends on it.
    let (user, bypasses): (String, bool) = sqlx::query_as(
        "SELECT current_user::text, rolsuper OR rolbypassrls FROM pg_roles WHERE rolname = current_user",
    )
    .fetch_one(&pool)
    .await?;
    if bypasses {
        anyhow::bail!(
            "the API is running as database role {user}, which bypasses row-level security; \
             set DATABASE_ROLE=doctr8_app (or connect as doctr8_app)"
        );
    }
    tracing::info!(role = %user, "database connected");

    // Fail fast if the Rust Permission enum and the permissions table disagree.
    common::permission::verify_catalogue(&pool).await?;

    // One LISTEN connection per process fans committed events out to sockets.
    let events = common::ws::EventBus::new();
    tokio::spawn(events.clone().listen(pool.clone()));

    let bind_addr = config.bind_addr.clone();
    // Spans record method + path only. Query strings and headers are never
    // logged: they can carry patient details or tokens.
    let trace = TraceLayer::new_for_http().make_span_with(|req: &axum::http::Request<_>| {
        tracing::info_span!("http", method = %req.method(), path = %req.uri().path())
    });
    let app = modules::router()
        .with_state(AppState::with_events(pool, config, events))
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
        .context("MIGRATION_DATABASE_URL must be set (a superuser, or a role with BYPASSRLS and CREATEROLE)")?;
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
