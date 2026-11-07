//! Login, refresh-token rotation, practice switching and logout.
//!
//! The refresh token lives in an HttpOnly cookie scoped to `/auth`; the access
//! token is returned in the body for the client to send as `Authorization: Bearer`.

pub mod dto;
pub mod handler;
pub mod repository;
pub mod service;

use axum::{Router, routing::post};

use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/auth/login", post(handler::login))
        .route("/auth/refresh", post(handler::refresh))
        .route("/auth/switch-practice", post(handler::switch_practice))
        .route("/auth/logout", post(handler::logout))
}
