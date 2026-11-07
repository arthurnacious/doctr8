//! Accounts: the billing file — who pays, and which medical aid (if any).
//! Accounts are opened through `POST /patient-files`, never on their own,
//! because every account must have its patients from the moment it exists.

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
        .route("/accounts/{id}", get(handler::get).patch(handler::update))
        .route("/accounts/{id}/patients", post(handler::add_patient))
}
