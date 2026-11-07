//! Patients (a person on an account, with a dependant code) and the one-step
//! "open a patient file" endpoint.

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
        .route("/patient-files", post(handler::create_file))
        .route("/patients/{id}", get(handler::get).patch(handler::update))
}
