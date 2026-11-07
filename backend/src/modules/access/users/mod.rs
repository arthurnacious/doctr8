//! Users (global identities) and practice members (memberships + their roles).

pub mod dto;
pub mod handler;
pub mod repository;
pub mod service;

use axum::{
    Router,
    routing::{get, post, put},
};

use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/users", post(handler::register))
        .route("/users/me", get(handler::me))
        .route("/members", get(handler::list_members))
        .route("/members/{user_id}/roles", put(handler::set_member_roles))
}
