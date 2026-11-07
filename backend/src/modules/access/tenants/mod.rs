//! Practices: create one, list mine, view/edit the current one.

pub mod dto;
pub mod handler;
pub mod repository;
pub mod service;

use axum::{Router, routing::get};

use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/tenants", get(handler::list_mine).post(handler::create))
        .route("/tenants/current", get(handler::current).patch(handler::update_current))
}
