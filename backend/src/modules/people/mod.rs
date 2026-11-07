//! Patients & medical aid domain.
//!
//! PRIVACY: this module handles personal information (POPIA special
//! personal information, in the case of health data). Never log names, ID
//! numbers, contact details or addresses — log UUIDs only. DTOs that carry
//! personal data deliberately don't derive `Debug`, so they can't be
//! accidentally `?`-formatted into a log line.

pub mod accounts;
pub mod memberships;
pub mod patients;
pub mod persons;

use axum::Router;

use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .merge(persons::router())
        .merge(memberships::router())
        .merge(accounts::router())
        .merge(patients::router())
}
