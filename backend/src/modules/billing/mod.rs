//! Billing & statements.
//!
//! * `providers`    — doctors named on invoices, and service centres
//! * `invoices`     — invoices, lines, credit notes, claims
//! * `transactions` — the money ledger: payments, refunds, remittances,
//!   journals, liability transfers, reversals
//! * `statements`   — account statement as of a date, with ageing
//!
//! Money is integer cents (`i64`) everywhere. Posted money is never edited
//! or deleted; corrections are new transactions.

pub mod invoices;
pub mod providers;
pub mod statements;
pub mod transactions;

use axum::Router;
use chrono::{Duration, NaiveDate, Utc};

use crate::state::AppState;

/// Today's date in South Africa (UTC+2, no daylight saving). Business dates
/// (invoice, payment, statement dates) are SA dates, not UTC dates.
pub fn today() -> NaiveDate {
    sa_date(Utc::now())
}

pub fn sa_date(at: chrono::DateTime<Utc>) -> NaiveDate {
    (at + Duration::hours(2)).date_naive()
}

pub fn router() -> Router<AppState> {
    Router::new()
        .merge(providers::router())
        .merge(invoices::router())
        .merge(transactions::router())
        .merge(statements::router())
}

#[cfg(test)]
mod tests {
    use chrono::TimeZone;

    use super::*;

    #[test]
    fn business_dates_are_south_african() {
        // 23:30 UTC on 6 Nov is 01:30 on 7 Nov in Johannesburg.
        assert_eq!(sa_date(Utc.with_ymd_and_hms(2026, 11, 6, 23, 30, 0).unwrap()), NaiveDate::from_ymd_opt(2026, 11, 7).unwrap());
        assert_eq!(sa_date(Utc.with_ymd_and_hms(2026, 11, 6, 21, 59, 0).unwrap()), NaiveDate::from_ymd_opt(2026, 11, 6).unwrap());
    }
}
