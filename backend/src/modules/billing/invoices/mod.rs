//! Invoices (with lines, diagnoses and modifiers), credit notes and claims.

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
        .route("/invoices", post(handler::create))
        .route("/invoices/{id}", get(handler::get))
        .route("/accounts/{id}/invoices", get(handler::list_for_account))
        .route("/invoices/{id}/credit-notes", post(handler::credit))
        .route("/invoices/{id}/claims", post(handler::submit_claim))
        .route("/claims", get(handler::list_claims))
        .route("/claims/{id}/status", post(handler::set_claim_status))
}
