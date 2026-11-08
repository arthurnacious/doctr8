//! Invoice drafts: an invoice being prepared. Free-form and editable, with
//! no number and no money. Finalising validates it and posts a real invoice
//! (which can then never be edited) in the same transaction.

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
        .route("/invoice-drafts", post(handler::create))
        .route("/invoice-drafts/{id}", get(handler::get).put(handler::update).delete(handler::delete))
        .route("/invoice-drafts/{id}/finalise", post(handler::finalise))
        .route("/accounts/{id}/invoice-drafts", get(handler::list_for_account))
        .route("/accounts/{id}/billing-context", get(handler::context))
}
