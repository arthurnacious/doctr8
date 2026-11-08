use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::common::{
    error::{AppError, AppResult},
    validation,
};

/// Largest single amount accepted: R100 million.
pub const MAX_AMOUNT_CENTS: i64 = 10_000_000_000;

/// Who owes it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Bucket {
    MedicalAid,
    Patient,
}

impl Bucket {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::MedicalAid => "medical_aid",
            Self::Patient => "patient",
        }
    }

    pub const fn other(self) -> Self {
        match self {
            Self::MedicalAid => Self::Patient,
            Self::Patient => Self::MedicalAid,
        }
    }

    pub fn from_db(s: &str) -> anyhow::Result<Self> {
        match s {
            "medical_aid" => Ok(Self::MedicalAid),
            "patient" => Ok(Self::Patient),
            other => anyhow::bail!("unknown bucket {other}"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TxnKind {
    Invoice,
    ClaimSubmitted,
    Remittance,
    PatientPayment,
    Refund,
    CreditNote,
    Journal,
    LiabilityTransfer,
    Reversal,
}

impl TxnKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Invoice => "invoice",
            Self::ClaimSubmitted => "claim_submitted",
            Self::Remittance => "remittance",
            Self::PatientPayment => "patient_payment",
            Self::Refund => "refund",
            Self::CreditNote => "credit_note",
            Self::Journal => "journal",
            Self::LiabilityTransfer => "liability_transfer",
            Self::Reversal => "reversal",
        }
    }

    pub fn from_db(s: &str) -> anyhow::Result<Self> {
        Ok(match s {
            "invoice" => Self::Invoice,
            "claim_submitted" => Self::ClaimSubmitted,
            "remittance" => Self::Remittance,
            "patient_payment" => Self::PatientPayment,
            "refund" => Self::Refund,
            "credit_note" => Self::CreditNote,
            "journal" => Self::Journal,
            "liability_transfer" => Self::LiabilityTransfer,
            "reversal" => Self::Reversal,
            other => anyhow::bail!("unknown transaction kind {other}"),
        })
    }

    /// Invoices are corrected with credit notes, credit notes and claim
    /// records stand, and reversals can't themselves be reversed.
    pub const fn reversible(self) -> bool {
        matches!(self, Self::Remittance | Self::PatientPayment | Self::Refund | Self::Journal | Self::LiabilityTransfer)
    }

    /// Statement wording.
    pub const fn label(self) -> &'static str {
        match self {
            Self::Invoice => "Invoice",
            Self::ClaimSubmitted => "Claim submitted",
            Self::Remittance => "Medical aid payment",
            Self::PatientPayment => "Payment received",
            Self::Refund => "Refund",
            Self::CreditNote => "Credit note",
            Self::Journal => "Journal",
            Self::LiabilityTransfer => "Liability transfer",
            Self::Reversal => "Reversal",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PaymentMethod {
    Cash,
    Card,
    Eft,
}

impl PaymentMethod {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Cash => "cash",
            Self::Card => "card",
            Self::Eft => "eft",
        }
    }
}

/// One signed ledger line. Positive = more owed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub invoice_id: Option<Uuid>,
    pub bucket: Bucket,
    pub amount_cents: i64,
}

/// Everything needed to post a transaction. Built by services only.
#[derive(Debug, Clone)]
pub struct NewTransaction {
    pub account_id: Uuid,
    pub kind: TxnKind,
    pub txn_date: NaiveDate,
    pub amount_cents: i64,
    pub payment_method: Option<PaymentMethod>,
    pub reference: Option<String>,
    pub notes: Option<String>,
    pub invoice_id: Option<Uuid>,
    pub claim_id: Option<Uuid>,
    pub credit_note_id: Option<Uuid>,
    pub remittance_id: Option<Uuid>,
    pub reason_type_id: Option<Uuid>,
    pub reverses_transaction_id: Option<Uuid>,
    /// Patient payments only: the practice's receipt number.
    pub receipt_no: Option<i64>,
    pub entries: Vec<Entry>,
}

impl NewTransaction {
    pub fn new(account_id: Uuid, kind: TxnKind, txn_date: NaiveDate, amount_cents: i64) -> Self {
        Self {
            account_id,
            kind,
            txn_date,
            amount_cents,
            payment_method: None,
            reference: None,
            notes: None,
            invoice_id: None,
            claim_id: None,
            credit_note_id: None,
            remittance_id: None,
            reason_type_id: None,
            reverses_transaction_id: None,
            receipt_no: None,
            entries: Vec::new(),
        }
    }
}

// --- responses -------------------------------------------------------------------

#[derive(Serialize)]
pub struct EntryResponse {
    pub invoice_id: Option<Uuid>,
    pub bucket: Bucket,
    pub amount_cents: i64,
}

#[derive(Serialize)]
pub struct TransactionResponse {
    pub id: Uuid,
    pub account_id: Uuid,
    pub kind: TxnKind,
    pub txn_date: NaiveDate,
    pub posted_at: DateTime<Utc>,
    pub amount_cents: i64,
    pub payment_method: Option<String>,
    pub reference: Option<String>,
    pub notes: Option<String>,
    pub invoice_id: Option<Uuid>,
    pub claim_id: Option<Uuid>,
    pub credit_note_id: Option<Uuid>,
    pub remittance_id: Option<Uuid>,
    pub reason: Option<ReasonRef>,
    pub reverses_transaction_id: Option<Uuid>,
    pub reversed_by_transaction_id: Option<Uuid>,
    pub receipt_no: Option<i64>,
    pub entries: Vec<EntryResponse>,
}

#[derive(Serialize, Clone)]
pub struct ReasonRef {
    pub id: Uuid,
    pub code: String,
    pub name: String,
}

#[derive(Serialize, Default, Debug, PartialEq, Eq)]
pub struct BalanceResponse {
    pub account_id: Uuid,
    pub medical_aid_cents: i64,
    pub patient_cents: i64,
    pub total_cents: i64,
    /// Patient money not allocated to any invoice (negative = credit).
    pub unallocated_cents: i64,
}

// --- requests --------------------------------------------------------------------

fn amount(field: &str, cents: i64) -> AppResult<i64> {
    if cents <= 0 {
        return Err(AppError::validation(format!("{field} must be more than zero")));
    }
    if cents > MAX_AMOUNT_CENTS {
        return Err(AppError::validation(format!("{field} is too large")));
    }
    Ok(cents)
}

fn date(field: &str, value: Option<NaiveDate>, today: NaiveDate) -> AppResult<NaiveDate> {
    let d = value.unwrap_or(today);
    if d > today {
        return Err(AppError::validation(format!("{field} can't be in the future")));
    }
    Ok(d)
}

fn reference(value: Option<&str>) -> AppResult<Option<String>> {
    validation::optional_text("reference", value, 60)
}

#[derive(Deserialize, Clone)]
pub struct Allocation {
    pub invoice_id: Uuid,
    pub amount_cents: i64,
}

fn allocations(list: Option<Vec<Allocation>>) -> AppResult<Option<Vec<Allocation>>> {
    let Some(list) = list else { return Ok(None) };
    if list.len() > 200 {
        return Err(AppError::validation("Too many allocations"));
    }
    let mut seen = std::collections::HashSet::new();
    for a in &list {
        amount("allocation amount_cents", a.amount_cents)?;
        if !seen.insert(a.invoice_id) {
            return Err(AppError::validation("Allocate to each invoice once"));
        }
    }
    Ok(Some(list))
}

#[derive(Deserialize)]
pub struct PaymentRequest {
    pub account_id: Uuid,
    #[serde(default)]
    pub txn_date: Option<NaiveDate>,
    pub method: PaymentMethod,
    pub amount_cents: i64,
    #[serde(default)]
    pub reference: Option<String>,
    #[serde(default)]
    pub notes: Option<String>,
    /// Omit to allocate oldest invoice first. Anything not allocated stays
    /// on the account as credit.
    #[serde(default)]
    pub allocations: Option<Vec<Allocation>>,
}

pub struct ValidPayment {
    pub account_id: Uuid,
    pub txn_date: NaiveDate,
    pub method: PaymentMethod,
    pub amount_cents: i64,
    pub reference: Option<String>,
    pub notes: Option<String>,
    pub allocations: Option<Vec<Allocation>>,
}

impl PaymentRequest {
    pub fn validate(self, today: NaiveDate) -> AppResult<ValidPayment> {
        let amount_cents = amount("amount_cents", self.amount_cents)?;
        let allocations = allocations(self.allocations)?;
        if let Some(list) = &allocations
            && list.iter().map(|a| a.amount_cents).sum::<i64>() > amount_cents
        {
            return Err(AppError::validation("Allocations add up to more than the payment"));
        }
        Ok(ValidPayment {
            account_id: self.account_id,
            txn_date: date("txn_date", self.txn_date, today)?,
            method: self.method,
            amount_cents,
            reference: reference(self.reference.as_deref())?,
            notes: validation::optional_text("notes", self.notes.as_deref(), 500)?,
            allocations,
        })
    }
}

#[derive(Deserialize)]
pub struct RefundRequest {
    pub account_id: Uuid,
    #[serde(default)]
    pub txn_date: Option<NaiveDate>,
    pub method: PaymentMethod,
    pub amount_cents: i64,
    #[serde(default)]
    pub reference: Option<String>,
    #[serde(default)]
    pub notes: Option<String>,
}

pub struct ValidRefund {
    pub account_id: Uuid,
    pub txn_date: NaiveDate,
    pub method: PaymentMethod,
    pub amount_cents: i64,
    pub reference: Option<String>,
    pub notes: Option<String>,
}

impl RefundRequest {
    pub fn validate(self, today: NaiveDate) -> AppResult<ValidRefund> {
        Ok(ValidRefund {
            account_id: self.account_id,
            txn_date: date("txn_date", self.txn_date, today)?,
            method: self.method,
            amount_cents: amount("amount_cents", self.amount_cents)?,
            reference: reference(self.reference.as_deref())?,
            notes: validation::optional_text("notes", self.notes.as_deref(), 500)?,
        })
    }
}

#[derive(Deserialize)]
pub struct JournalEntryInput {
    #[serde(default)]
    pub invoice_id: Option<Uuid>,
    pub bucket: Bucket,
    /// Signed: negative reduces what's owed (write-off), positive increases it.
    pub amount_cents: i64,
}

#[derive(Deserialize)]
pub struct JournalRequest {
    pub account_id: Uuid,
    pub reason_type_id: Uuid,
    #[serde(default)]
    pub txn_date: Option<NaiveDate>,
    pub notes: String,
    pub entries: Vec<JournalEntryInput>,
}

pub struct ValidJournal {
    pub account_id: Uuid,
    pub reason_type_id: Uuid,
    pub txn_date: NaiveDate,
    pub notes: String,
    pub entries: Vec<Entry>,
}

impl JournalRequest {
    pub fn validate(self, today: NaiveDate) -> AppResult<ValidJournal> {
        if self.entries.is_empty() || self.entries.len() > 20 {
            return Err(AppError::validation("A journal needs 1-20 entries"));
        }
        let mut entries = Vec::with_capacity(self.entries.len());
        for e in self.entries {
            if e.amount_cents == 0 || e.amount_cents.abs() > MAX_AMOUNT_CENTS {
                return Err(AppError::validation("Journal entry amounts must be non-zero and within limits"));
            }
            if e.invoice_id.is_none() && e.bucket == Bucket::MedicalAid {
                return Err(AppError::validation("Medical aid entries must name an invoice"));
            }
            entries.push(Entry { invoice_id: e.invoice_id, bucket: e.bucket, amount_cents: e.amount_cents });
        }
        Ok(ValidJournal {
            account_id: self.account_id,
            reason_type_id: self.reason_type_id,
            txn_date: date("txn_date", self.txn_date, today)?,
            notes: validation::text("notes", &self.notes, 500)?,
            entries,
        })
    }
}

#[derive(Deserialize)]
pub struct TransferRequest {
    pub invoice_id: Uuid,
    /// Who the liability moves away from.
    pub from: Bucket,
    pub amount_cents: i64,
    #[serde(default)]
    pub txn_date: Option<NaiveDate>,
    pub notes: String,
}

pub struct ValidTransfer {
    pub invoice_id: Uuid,
    pub from: Bucket,
    pub amount_cents: i64,
    pub txn_date: NaiveDate,
    pub notes: String,
}

impl TransferRequest {
    pub fn validate(self, today: NaiveDate) -> AppResult<ValidTransfer> {
        Ok(ValidTransfer {
            invoice_id: self.invoice_id,
            from: self.from,
            amount_cents: amount("amount_cents", self.amount_cents)?,
            txn_date: date("txn_date", self.txn_date, today)?,
            notes: validation::text("notes", &self.notes, 500)?,
        })
    }
}

#[derive(Deserialize)]
pub struct ReverseRequest {
    #[serde(default)]
    pub txn_date: Option<NaiveDate>,
    pub notes: String,
}

// --- journal reason types --------------------------------------------------------

#[derive(Serialize, sqlx::FromRow)]
pub struct ReasonTypeResponse {
    pub id: Uuid,
    pub code: String,
    pub name: String,
    pub is_active: bool,
}

#[derive(Deserialize)]
pub struct ReasonTypeInput {
    pub code: String,
    pub name: String,
}

impl ReasonTypeInput {
    pub fn validate(self) -> AppResult<(String, String)> {
        let code = self.code.trim().to_lowercase();
        let ok = (2..=40).contains(&code.len())
            && code.starts_with(|c: char| c.is_ascii_lowercase())
            && code.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_');
        if !ok {
            return Err(AppError::validation("code must be 2-40 characters of a-z, 0-9 and _, starting with a letter"));
        }
        Ok((code, validation::text("name", &self.name, 100)?))
    }
}

/// The code is fixed once created (reports may key on it).
#[derive(Deserialize)]
pub struct ReasonTypeUpdate {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub is_active: Option<bool>,
}

// --- remittances -----------------------------------------------------------------

#[derive(Deserialize)]
pub struct RemittanceRequest {
    pub scheme_id: Uuid,
    pub reference: String,
    pub received_date: NaiveDate,
    pub total_cents: i64,
    #[serde(default)]
    pub notes: Option<String>,
}

pub struct ValidRemittance {
    pub scheme_id: Uuid,
    pub reference: String,
    pub received_date: NaiveDate,
    pub total_cents: i64,
    pub notes: Option<String>,
}

impl RemittanceRequest {
    pub fn validate(self, today: NaiveDate) -> AppResult<ValidRemittance> {
        Ok(ValidRemittance {
            scheme_id: self.scheme_id,
            reference: validation::text("reference", &self.reference, 60)?,
            received_date: date("received_date", Some(self.received_date), today)?,
            total_cents: amount("total_cents", self.total_cents)?,
            notes: validation::optional_text("notes", self.notes.as_deref(), 500)?,
        })
    }
}

#[derive(Deserialize)]
pub struct AllocateRemittanceRequest {
    pub account_id: Uuid,
    #[serde(default)]
    pub txn_date: Option<NaiveDate>,
    /// What the scheme paid per invoice.
    pub lines: Vec<Allocation>,
}

impl AllocateRemittanceRequest {
    pub fn validate(self, today: NaiveDate) -> AppResult<(Uuid, NaiveDate, Vec<Allocation>)> {
        let lines = allocations(Some(self.lines))?.unwrap_or_default();
        if lines.is_empty() {
            return Err(AppError::validation("lines must allocate to at least one invoice"));
        }
        Ok((self.account_id, date("txn_date", self.txn_date, today)?, lines))
    }
}

#[derive(Deserialize)]
pub struct RemittanceFilter {
    #[serde(default)]
    pub unallocated_only: bool,
    #[serde(default)]
    pub limit: Option<i64>,
}

#[derive(Serialize, sqlx::FromRow)]
pub struct RemittanceSummary {
    pub id: Uuid,
    pub scheme_id: Uuid,
    pub scheme_name: String,
    pub reference: String,
    pub received_date: NaiveDate,
    pub total_cents: i64,
    pub allocated_cents: i64,
    pub unallocated_cents: i64,
    pub notes: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Serialize, sqlx::FromRow)]
pub struct RemittanceAllocation {
    pub transaction_id: Uuid,
    pub kind: String,
    pub account_id: Uuid,
    pub account_no: i64,
    pub txn_date: NaiveDate,
    /// Positive for an allocation, negative for its reversal.
    pub amount_cents: i64,
}

#[derive(Serialize)]
pub struct RemittanceResponse {
    #[serde(flatten)]
    pub remittance: RemittanceSummary,
    pub allocations: Vec<RemittanceAllocation>,
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::common::test_support::{date as d, today};

    #[test]
    fn enums_round_trip() {
        for k in [TxnKind::Invoice, TxnKind::ClaimSubmitted, TxnKind::Remittance, TxnKind::PatientPayment, TxnKind::Refund,
                  TxnKind::CreditNote, TxnKind::Journal, TxnKind::LiabilityTransfer, TxnKind::Reversal] {
            assert_eq!(TxnKind::from_db(k.as_str()).unwrap(), k);
            assert_eq!(serde_json::to_value(k).unwrap(), k.as_str());
            assert!(!k.label().is_empty());
        }
        assert!(TxnKind::from_db("gift").is_err());
        for b in [Bucket::MedicalAid, Bucket::Patient] {
            assert_eq!(Bucket::from_db(b.as_str()).unwrap(), b);
            assert_eq!(b.other().other(), b);
        }
        assert!(Bucket::from_db("insurer").is_err());
    }

    #[test]
    fn reversible_kinds_match_the_database_rule() {
        // Mirrors D8409 in migrations/0003.
        for k in [TxnKind::Invoice, TxnKind::ClaimSubmitted, TxnKind::CreditNote, TxnKind::Reversal] {
            assert!(!k.reversible(), "{k:?}");
        }
        for k in [TxnKind::Remittance, TxnKind::PatientPayment, TxnKind::Refund, TxnKind::Journal, TxnKind::LiabilityTransfer] {
            assert!(k.reversible(), "{k:?}");
        }
    }

    fn payment(v: serde_json::Value) -> AppResult<ValidPayment> {
        serde_json::from_value::<PaymentRequest>(v).unwrap().validate(today())
    }

    #[test]
    fn payment_rules() {
        let a = Uuid::nil();
        let p = payment(json!({"account_id": a, "method": "card", "amount_cents": 5000})).unwrap();
        assert_eq!(p.txn_date, today(), "defaults to today");
        assert!(p.allocations.is_none());
        assert!(payment(json!({"account_id": a, "method": "card", "amount_cents": 0})).is_err());
        assert!(payment(json!({"account_id": a, "method": "card", "amount_cents": -1})).is_err());
        assert!(payment(json!({"account_id": a, "method": "card", "amount_cents": MAX_AMOUNT_CENTS + 1})).is_err());
        assert!(payment(json!({"account_id": a, "method": "card", "amount_cents": 1, "txn_date": "2026-11-08"})).is_err(), "future");
        assert!(serde_json::from_value::<PaymentRequest>(json!({"account_id": a, "method": "cheque", "amount_cents": 1})).is_err());
    }

    #[test]
    fn payment_allocations() {
        let (a, i, j) = (Uuid::nil(), Uuid::from_u128(1), Uuid::from_u128(2));
        assert!(payment(json!({"account_id": a, "method": "eft", "amount_cents": 100,
            "allocations": [{"invoice_id": i, "amount_cents": 60}, {"invoice_id": j, "amount_cents": 40}]})).is_ok());
        assert!(payment(json!({"account_id": a, "method": "eft", "amount_cents": 100,
            "allocations": [{"invoice_id": i, "amount_cents": 60}, {"invoice_id": j, "amount_cents": 41}]})).is_err(), "over-allocated");
        assert!(payment(json!({"account_id": a, "method": "eft", "amount_cents": 100,
            "allocations": [{"invoice_id": i, "amount_cents": 10}, {"invoice_id": i, "amount_cents": 10}]})).is_err(), "duplicate");
        assert!(payment(json!({"account_id": a, "method": "eft", "amount_cents": 100,
            "allocations": [{"invoice_id": i, "amount_cents": 0}]})).is_err());
    }

    #[test]
    fn journal_rules() {
        let j = |v| serde_json::from_value::<JournalRequest>(v).unwrap().validate(today());
        let a = Uuid::nil();
        let ok = j(json!({"account_id": a, "reason_type_id": a, "notes": "write off", "entries": [{"bucket": "patient", "amount_cents": -500}]})).unwrap();
        assert_eq!(ok.entries[0], Entry { invoice_id: None, bucket: Bucket::Patient, amount_cents: -500 });
        assert!(j(json!({"account_id": a, "reason_type_id": a, "notes": " ", "entries": [{"bucket": "patient", "amount_cents": -5}]})).is_err(), "notes required");
        assert!(j(json!({"account_id": a, "reason_type_id": a, "notes": "x", "entries": []})).is_err());
        assert!(j(json!({"account_id": a, "reason_type_id": a, "notes": "x", "entries": [{"bucket": "patient", "amount_cents": 0}]})).is_err());
        assert!(j(json!({"account_id": a, "reason_type_id": a, "notes": "x", "entries": [{"bucket": "medical_aid", "amount_cents": -5}]})).is_err(),
            "medical aid money is always on an invoice");
    }

    #[test]
    fn transfer_and_refund_rules() {
        let t = |v| serde_json::from_value::<TransferRequest>(v).unwrap().validate(today());
        assert!(t(json!({"invoice_id": Uuid::nil(), "from": "medical_aid", "amount_cents": 100, "notes": "co-payment"})).is_ok());
        assert!(t(json!({"invoice_id": Uuid::nil(), "from": "medical_aid", "amount_cents": 100, "notes": ""})).is_err());
        assert!(t(json!({"invoice_id": Uuid::nil(), "from": "patient", "amount_cents": -1, "notes": "x"})).is_err());
        let r = |v| serde_json::from_value::<RefundRequest>(v).unwrap().validate(today());
        assert!(r(json!({"account_id": Uuid::nil(), "method": "eft", "amount_cents": 100})).is_ok());
        assert!(r(json!({"account_id": Uuid::nil(), "method": "eft", "amount_cents": 0})).is_err());
    }

    #[test]
    fn reason_codes() {
        let v = |code: &str| ReasonTypeInput { code: code.into(), name: "N".into() }.validate();
        assert_eq!(v(" Fee_Waiver ").unwrap().0, "fee_waiver");
        for bad in ["x", "1abc", "has space", "dash-ed", &"a".repeat(41)] {
            assert!(v(bad).is_err(), "{bad}");
        }
        assert!(ReasonTypeInput { code: "ok_code".into(), name: " ".into() }.validate().is_err());
    }

    #[test]
    fn remittance_rules() {
        let r = |v| serde_json::from_value::<RemittanceRequest>(v).unwrap().validate(today());
        assert!(r(json!({"scheme_id": Uuid::nil(), "reference": "RA-1", "received_date": "2026-11-01", "total_cents": 1})).is_ok());
        assert!(r(json!({"scheme_id": Uuid::nil(), "reference": "RA-1", "received_date": "2026-12-01", "total_cents": 1})).is_err());
        assert!(r(json!({"scheme_id": Uuid::nil(), "reference": " ", "received_date": "2026-11-01", "total_cents": 1})).is_err());
        let a = |v| serde_json::from_value::<AllocateRemittanceRequest>(v).unwrap().validate(today());
        assert!(a(json!({"account_id": Uuid::nil(), "lines": []})).is_err());
        let (_, when, lines) = a(json!({"account_id": Uuid::nil(), "txn_date": "2026-11-02", "lines": [{"invoice_id": Uuid::nil(), "amount_cents": 5}]})).unwrap();
        assert_eq!((when, lines.len()), (d(2026, 11, 2), 1));
    }
}
