//! A practice's roles and the permissions granted to them.

pub mod dto;
pub mod handler;
pub mod repository;
pub mod service;

use axum::{
    Router,
    routing::{get, put},
};

use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/permissions", get(handler::catalogue))
        .route("/roles", get(handler::list).post(handler::create))
        .route("/roles/{id}", get(handler::get).patch(handler::update).delete(handler::delete))
        .route("/roles/{id}/permissions", put(handler::set_permissions))
}
