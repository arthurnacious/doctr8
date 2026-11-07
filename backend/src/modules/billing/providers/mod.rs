//! Providers (treating, referring and assisting doctors) and service centres.

pub mod dto;
pub mod handler;
pub mod repository;
pub mod service;

use axum::{Router, routing::get};

use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/providers", get(handler::list).post(handler::create))
        .route("/providers/{id}", get(handler::get).put(handler::replace))
        .route("/service-centres", get(handler::list_centres).post(handler::create_centre))
        .route("/service-centres/{id}", axum::routing::put(handler::replace_centre))
}
