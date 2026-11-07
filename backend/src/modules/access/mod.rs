//! Practices & access domain.

pub mod auth;
pub mod roles;
pub mod tenants;
pub mod users;

use axum::Router;

use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .merge(auth::router())
        .merge(tenants::router())
        .merge(users::router())
        .merge(roles::router())
}
