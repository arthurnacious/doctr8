//! Persons: anyone the practice knows — patients, guarantors, main members.

pub mod dto;
pub mod handler;
pub mod repository;
pub mod service;

use axum::{
    Router,
    routing::{get, post},
};

use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/persons", post(handler::create))
        // POST so names and ID numbers travel in the body, never in URLs
        // (which end up in proxy and access logs).
        .route("/persons/search", post(handler::search))
        .route("/persons/{id}", get(handler::get).put(handler::replace))
        .route("/persons/{id}/files", get(handler::files))
}
