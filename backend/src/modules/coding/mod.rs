//! Global coding reference: ICD-10, tariff codes, NAPPI codes, modifiers.
//! Read-only to practices; doctr8 maintains the lists (full imports later).

pub mod dto;
pub mod handler;
pub mod repository;
pub mod service;

use axum::{Router, routing::get};

use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/coding/icd10", get(handler::icd10))
        .route("/coding/tariffs", get(handler::tariffs))
        .route("/coding/tariff-price", get(handler::tariff_price))
        .route("/coding/nappi", get(handler::nappi))
        .route("/coding/modifiers", get(handler::modifiers))
}
