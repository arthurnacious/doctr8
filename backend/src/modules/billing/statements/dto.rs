use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::modules::billing::{
    invoices::dto::LineResponse,
    transactions::dto::{Bucket, TxnKind},
};

pub const BANDS: [&str; 7] = ["current", "30", "60", "90", "120", "150", "180+"];

#[derive(Deserialize)]
pub struct StatementQuery {
    /// Defaults to today.
    #[serde(default)]
    pub as_of: Option<NaiveDate>,
    /// Show transactions that were reversed (and their reversals) as of the
    /// statement date. They net to zero either way.
    #[serde(default)]
    pub include_reversed: bool,
    /// Only invoices that still have something owing.
    #[serde(default)]
    pub outstanding_only: bool,
}

/// One ledger entry joined with its transaction, as read for a statement.
#[derive(Debug, Clone)]
pub struct LedgerRow {
    pub transaction_id: Uuid,
    pub kind: TxnKind,
    pub txn_date: NaiveDate,
    pub reference: Option<String>,
    /// Kept for internal use; statements never print free-text notes.
    #[allow(dead_code)]
    pub notes: Option<String>,
    pub reason_name: Option<String>,
    pub payment_method: Option<String>,
    pub reverses: Option<Uuid>,
    /// Date of the reversal of this transaction, if any.
    pub reversed_on: Option<NaiveDate>,
    pub invoice_id: Option<Uuid>,
    /// None for transactions that don't move money (claim submitted).
    pub bucket: Option<Bucket>,
    pub amount_cents: i64,
    /// The date this money ages from: the invoice date for invoice money;
    /// for unallocated money, the transaction date — or, for a reversal, the
    /// date of what it reverses, so a journal and its later reversal land in
    /// the same ageing band and cancel out.
    pub age_from: NaiveDate,
}

#[derive(Serialize, Debug, Clone, PartialEq, Eq)]
pub struct StatementLine {
    pub transaction_id: Uuid,
    pub date: NaiveDate,
    pub kind: TxnKind,
    pub description: String,
    pub medical_aid_cents: i64,
    pub patient_cents: i64,
    /// This transaction was reversed on or before the statement date.
    pub reversed: bool,
    pub reverses_transaction_id: Option<Uuid>,
}

#[derive(Serialize, Debug, Clone, PartialEq, Eq, Default)]
pub struct Ageing {
    pub bands: [&'static str; 7],
    pub medical_aid: [i64; 7],
    pub patient: [i64; 7],
    pub total: [i64; 7],
}

#[derive(Serialize, Debug, Clone, PartialEq, Eq, Default)]
pub struct Totals {
    pub medical_aid_due_cents: i64,
    pub patient_due_cents: i64,
    pub total_due_cents: i64,
}

#[derive(Serialize)]
pub struct StatementPractice {
    pub name: String,
    pub pcns_number: Option<String>,
}

#[derive(Serialize)]
pub struct StatementAccount {
    pub id: Uuid,
    pub account_no: i64,
    pub status: String,
    pub file_mode: String,
}

/// Who the statement is addressed to (the guarantor).
#[derive(Serialize)]
pub struct StatementAddressee {
    pub person_id: Uuid,
    pub title: Option<String>,
    pub first_name: String,
    pub last_name: String,
    pub address_line1: Option<String>,
    pub address_line2: Option<String>,
    pub suburb: Option<String>,
    pub city: Option<String>,
    pub postal_code: Option<String>,
}

#[derive(Serialize)]
pub struct StatementMedicalAid {
    pub scheme_name: String,
    pub plan_name: String,
    pub membership_number: String,
    pub main_member_person_id: Uuid,
    pub main_member_name: String,
}

#[derive(Serialize, sqlx::FromRow)]
pub struct StatementPatient {
    pub patient_id: Uuid,
    pub person_id: Uuid,
    pub dependant_code: String,
    pub title: Option<String>,
    pub first_name: String,
    pub last_name: String,
    pub date_of_birth: Option<NaiveDate>,
}

#[derive(Serialize)]
pub struct StatementInvoice {
    pub id: Uuid,
    pub invoice_no: i64,
    pub invoice_date: NaiveDate,
    pub patient_id: Uuid,
    pub dependant_code: Option<String>,
    pub treating_provider: String,
    pub referring_provider: Option<String>,
    pub service_centre: String,
    pub authorisation_number: Option<String>,
    pub total_cents: i64,
    pub lines: Vec<LineResponse>,
    pub transactions: Vec<StatementLine>,
    pub medical_aid_due_cents: i64,
    pub patient_due_cents: i64,
}

#[derive(Serialize)]
pub struct StatementResponse {
    pub as_of: NaiveDate,
    pub generated_at: DateTime<Utc>,
    pub practice: StatementPractice,
    pub account: StatementAccount,
    pub addressee: StatementAddressee,
    pub medical_aid: Option<StatementMedicalAid>,
    pub patients: Vec<StatementPatient>,
    pub invoices: Vec<StatementInvoice>,
    /// Money on the account not allocated to an invoice (e.g. overpayments).
    pub unallocated: Vec<StatementLine>,
    pub totals: Totals,
    pub ageing: Ageing,
}
