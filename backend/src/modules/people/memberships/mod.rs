//! Medical aid memberships (scheme + plan + number + main member) and the
//! global scheme/plan catalogue.

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
        .route("/medical-schemes", get(handler::schemes))
        .route("/medical-aid-memberships", post(handler::create))
        .route("/medical-aid-memberships/{id}", get(handler::get).patch(handler::update))
}
