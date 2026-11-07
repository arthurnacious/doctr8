//! Account statement as of a date.

pub mod dto;
pub mod handler;
pub mod repository;
pub mod service;

use axum::{Router, routing::get};

use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new().route("/accounts/{id}/statement", get(handler::statement))
}
