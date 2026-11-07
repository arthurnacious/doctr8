//! The money ledger: patient payments, refunds, medical aid remittances,
//! journals, liability transfers and reversals; journal reason types.

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
        .route("/accounts/{id}/transactions", get(handler::list_for_account))
        .route("/accounts/{id}/balance", get(handler::balance))
        .route("/payments", post(handler::payment))
        .route("/refunds", post(handler::refund))
        .route("/journals", post(handler::journal))
        .route("/liability-transfers", post(handler::transfer))
        .route("/transactions/{id}", get(handler::get))
        .route("/transactions/{id}/reverse", post(handler::reverse))
        .route("/journal-reason-types", get(handler::list_reasons).post(handler::create_reason))
        .route("/journal-reason-types/{id}", put(handler::update_reason))
        .route("/remittances", get(handler::list_remittances).post(handler::create_remittance))
        .route("/remittances/{id}", get(handler::get_remittance))
        .route("/remittances/{id}/allocations", post(handler::allocate_remittance))
}
