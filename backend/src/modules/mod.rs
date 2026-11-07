pub mod access;
pub mod billing;
pub mod coding;
pub mod people;

use axum::{Router, routing::get};

use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/health", get(|| async { "ok" }))
        .merge(access::router())
        .merge(people::router())
        .merge(coding::router())
        .merge(billing::router())
        .merge(crate::common::ws::router())
}
