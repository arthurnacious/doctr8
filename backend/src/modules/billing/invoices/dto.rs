use std::collections::HashSet;

use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::{Decimal, RoundingStrategy, prelude::ToPrimitive};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    common::{
        error::{AppError, AppResult},
        validation,
    },
    modules::{
        billing::{providers::dto::place_of_service, transactions::dto::MAX_AMOUNT_CENTS},
        coding::dto::{DEFAULT_CODE_SET, normalise_icd10},
    },
};

pub const MAX_LINES: usize = 100;
pub const MAX_ICD10_PER_LINE: usize = 12;
pub const MAX_MODIFIERS_PER_LINE: usize = 10;

fn one() -> Decimal {
    Decimal::ONE
}

#[derive(Deserialize)]
pub struct CreateInvoiceRequest {
    pub account_id: Uuid,
    pub patient_id: Uuid,
    /// Defaults to today.
    #[serde(default)]
    pub invoice_date: Option<NaiveDate>,
    pub treating_provider_id: Uuid,
    #[serde(default)]
    pub referring_provider_id: Option<Uuid>,
    pub service_centre_id: Uuid,
    #[serde(default)]
    pub authorisation_number: Option<String>,
    pub lines: Vec<LineInput>,
}

#[derive(Deserialize)]
pub struct LineInput {
    /// Defaults to the invoice date.
    #[serde(default)]
    pub service_date: Option<NaiveDate>,
    #[serde(default)]
    pub tariff_code: Option<String>,
    /// Defaults to CCSA.
    #[serde(default)]
    pub code_set: Option<String>,
    #[serde(default)]
    pub nappi_code: Option<String>,
    /// Defaults to the code's description.
    #[serde(default)]
    pub description: Option<String>,
    /// Decimal, e.g. "1" or "0.5". Defaults to 1.
    #[serde(default = "one")]
    pub quantity: Decimal,
    pub unit_price_cents: i64,
    /// Defaults to quantity × unit price, rounded half away from zero.
    /// Give it explicitly when a modifier changes the amount.
    #[serde(default)]
    pub amount_cents: Option<i64>,
    /// Defaults to the service centre's place of service.
    #[serde(default)]
    pub place_of_service: Option<String>,
    /// First is the primary diagnosis.
    pub icd10_codes: Vec<String>,
    #[serde(default)]
    pub modifiers: Vec<String>,
    #[serde(default)]
    pub assistant_provider_id: Option<Uuid>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LineItem {
    Tariff { code_set: String, code: String },
    Nappi(String),
}

pub struct NewLine {
    pub service_date: NaiveDate,
    pub item: LineItem,
    pub description: Option<String>,
    pub quantity: Decimal,
    pub unit_price_cents: i64,
    pub amount_cents: i64,
    pub place_of_service: Option<String>,
    pub icd10_codes: Vec<String>,
    pub modifiers: Vec<String>,
    pub assistant_provider_id: Option<Uuid>,
}

pub struct NewInvoice {
    pub account_id: Uuid,
    pub patient_id: Uuid,
    pub invoice_date: NaiveDate,
    pub treating_provider_id: Uuid,
    pub referring_provider_id: Option<Uuid>,
    pub service_centre_id: Uuid,
    pub authorisation_number: Option<String>,
    pub lines: Vec<NewLine>,
}

impl NewInvoice {
    pub fn total_cents(&self) -> i64 {
        self.lines.iter().map(|l| l.amount_cents).sum()
    }
}

/// quantity × unit price in cents, rounded half away from zero.
pub fn line_amount(quantity: Decimal, unit_price_cents: i64) -> Option<i64> {
    quantity
        .checked_mul(Decimal::from(unit_price_cents))?
        .round_dp_with_strategy(0, RoundingStrategy::MidpointAwayFromZero)
        .to_i64()
}

impl CreateInvoiceRequest {
    pub fn validate(self, today: NaiveDate) -> AppResult<NewInvoice> {
        let invoice_date = self.invoice_date.unwrap_or(today);
        if invoice_date > today {
            return Err(AppError::validation("invoice_date can't be in the future"));
        }
        if self.lines.is_empty() || self.lines.len() > MAX_LINES {
            return Err(AppError::validation(format!("An invoice needs 1-{MAX_LINES} lines")));
        }
        if self.referring_provider_id.is_some_and(|r| r == self.treating_provider_id) {
            return Err(AppError::validation("The referring provider can't be the treating provider"));
        }
        let authorisation_number = match validation::optional_text("authorisation_number", self.authorisation_number.as_deref(), 60)? {
            None => None,
            Some(a) => {
                let a: String = a.chars().filter(|c| !c.is_whitespace()).collect();
                if a.len() > 40 || !a.chars().all(|c| c.is_ascii_alphanumeric() || c == '/' || c == '-') {
                    return Err(AppError::validation("authorisation_number must be up to 40 letters, digits, '/' or '-'"));
                }
                Some(a)
            }
        };
        let lines = self
            .lines
            .into_iter()
            .enumerate()
            .map(|(i, l)| l.validate(invoice_date).map_err(|e| prefixed(&format!("lines[{i}]"), e)))
            .collect::<AppResult<Vec<_>>>()?;

        let invoice = NewInvoice {
            account_id: self.account_id,
            patient_id: self.patient_id,
            invoice_date,
            treating_provider_id: self.treating_provider_id,
            referring_provider_id: self.referring_provider_id,
            service_centre_id: self.service_centre_id,
            authorisation_number,
            lines,
        };
        let total = invoice.total_cents();
        if total <= 0 {
            return Err(AppError::validation("An invoice total must be more than zero"));
        }
        if total > MAX_AMOUNT_CENTS {
            return Err(AppError::validation("The invoice total is too large"));
        }
        Ok(invoice)
    }
}

impl LineInput {
    fn validate(self, invoice_date: NaiveDate) -> AppResult<NewLine> {
        let service_date = self.service_date.unwrap_or(invoice_date);
        if service_date > invoice_date {
            return Err(AppError::validation("service_date can't be after the invoice date"));
        }

        let item = match (self.tariff_code.as_deref().map(str::trim), self.nappi_code.as_deref().map(str::trim)) {
            (Some(t), None) if !t.is_empty() => {
                let code = t.to_uppercase();
                if !(4..=6).contains(&code.len()) || !code.chars().all(|c| c.is_ascii_alphanumeric()) {
                    return Err(AppError::validation("tariff_code must be 4-6 letters or digits"));
                }
                let code_set = self.code_set.as_deref().map(str::trim).unwrap_or(DEFAULT_CODE_SET).to_uppercase();
                LineItem::Tariff { code_set, code }
            }
            (None, Some(n)) if !n.is_empty() => {
                if !(n.len() == 6 || n.len() == 9) || !n.chars().all(|c| c.is_ascii_digit()) {
                    return Err(AppError::validation("nappi_code must be 6 or 9 digits"));
                }
                LineItem::Nappi(n.to_owned())
            }
            _ => return Err(AppError::validation("Give exactly one of tariff_code or nappi_code")),
        };

        if self.quantity <= Decimal::ZERO || self.quantity > Decimal::from(9_999_999) || self.quantity.scale() > 3 {
            return Err(AppError::validation("quantity must be more than 0, below 10 million, with at most 3 decimals"));
        }
        if !(0..=MAX_AMOUNT_CENTS).contains(&self.unit_price_cents) {
            return Err(AppError::validation("unit_price_cents must be zero or more and within limits"));
        }
        let amount_cents = match self.amount_cents {
            Some(a) => a,
            None => line_amount(self.quantity, self.unit_price_cents).ok_or_else(|| AppError::validation("The line amount is too large"))?,
        };
        if !(0..=MAX_AMOUNT_CENTS).contains(&amount_cents) {
            return Err(AppError::validation("amount_cents must be zero or more and within limits"));
        }

        let icd10_codes: Vec<String> = self.icd10_codes.iter().map(|c| normalise_icd10(c)).collect();
        if icd10_codes.is_empty() || icd10_codes.len() > MAX_ICD10_PER_LINE {
            return Err(AppError::validation(format!("icd10_codes needs 1-{MAX_ICD10_PER_LINE} codes")));
        }
        if icd10_codes.iter().collect::<HashSet<_>>().len() != icd10_codes.len() {
            return Err(AppError::validation("icd10_codes contains a duplicate"));
        }

        let modifiers: Vec<String> = self.modifiers.iter().map(|m| m.trim().to_owned()).collect();
        if modifiers.len() > MAX_MODIFIERS_PER_LINE {
            return Err(AppError::validation(format!("At most {MAX_MODIFIERS_PER_LINE} modifiers per line")));
        }
        if modifiers.iter().any(|m| m.len() != 4 || !m.chars().all(|c| c.is_ascii_digit())) {
            return Err(AppError::validation("Modifiers are 4-digit codes"));
        }
        if modifiers.iter().collect::<HashSet<_>>().len() != modifiers.len() {
            return Err(AppError::validation("modifiers contains a duplicate"));
        }

        Ok(NewLine {
            service_date,
            item,
            description: validation::optional_text("description", self.description.as_deref(), 200)?,
            quantity: self.quantity.normalize(),
            unit_price_cents: self.unit_price_cents,
            amount_cents,
            place_of_service: self.place_of_service.as_deref().map(place_of_service).transpose()?,
            icd10_codes,
            modifiers,
            assistant_provider_id: self.assistant_provider_id,
        })
    }
}

fn prefixed(path: &str, err: AppError) -> AppError {
    match err {
        AppError::Validation(m) => AppError::Validation(format!("{path}: {m}")),
        other => other,
    }
}

// --- claims ----------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClaimStatus {
    Submitted,
    Accepted,
    Rejected,
    Paid,
}

impl ClaimStatus {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Submitted => "submitted",
            Self::Accepted => "accepted",
            Self::Rejected => "rejected",
            Self::Paid => "paid",
        }
    }

    pub fn from_db(s: &str) -> anyhow::Result<Self> {
        Ok(match s {
            "submitted" => Self::Submitted,
            "accepted" => Self::Accepted,
            "rejected" => Self::Rejected,
            "paid" => Self::Paid,
            other => anyhow::bail!("unknown claim status {other}"),
        })
    }

    /// Mirrors app.claims_status() in migrations/0003.
    pub const fn can_become(self, next: Self) -> bool {
        matches!(
            (self, next),
            (Self::Submitted, Self::Accepted | Self::Rejected | Self::Paid) | (Self::Accepted, Self::Paid | Self::Rejected)
        )
    }

    pub const fn is_open(self) -> bool {
        matches!(self, Self::Submitted | Self::Accepted)
    }
}

/// Where an open claim goes once the scheme has paid something and
/// `medical_aid_due_after` is still owed by the scheme.
pub fn claim_status_after_payment(current: ClaimStatus, medical_aid_due_after: i64) -> Option<ClaimStatus> {
    if !current.is_open() {
        return None;
    }
    if medical_aid_due_after <= 0 {
        Some(ClaimStatus::Paid)
    } else if current == ClaimStatus::Submitted {
        Some(ClaimStatus::Accepted)
    } else {
        None
    }
}

#[derive(Deserialize)]
pub struct SubmitClaimRequest {
    #[serde(default)]
    pub submitted_date: Option<NaiveDate>,
    /// Defaults to what the medical aid currently owes on the invoice.
    #[serde(default)]
    pub claimed_cents: Option<i64>,
}

#[derive(Deserialize)]
pub struct ClaimStatusRequest {
    pub status: ClaimStatus,
    /// e.g. the scheme's rejection reason. No patient details.
    #[serde(default)]
    pub reason: Option<String>,
}

#[derive(Deserialize)]
pub struct ClaimsFilter {
    #[serde(default)]
    pub status: Option<ClaimStatus>,
    #[serde(default)]
    pub limit: Option<i64>,
}

#[derive(Serialize, sqlx::FromRow)]
pub struct ClaimResponse {
    pub id: Uuid,
    pub invoice_id: Uuid,
    pub invoice_no: i64,
    pub account_id: Uuid,
    pub status: String,
    pub claimed_cents: i64,
    pub submitted_date: NaiveDate,
    pub status_reason: Option<String>,
    pub updated_at: DateTime<Utc>,
}

// --- credit notes ----------------------------------------------------------------

#[derive(Deserialize)]
pub struct CreditLineInput {
    pub invoice_line_id: Uuid,
    /// Defaults to everything not yet credited on the line.
    #[serde(default)]
    pub amount_cents: Option<i64>,
}

#[derive(Deserialize)]
pub struct CreditNoteRequest {
    #[serde(default)]
    pub credit_date: Option<NaiveDate>,
    pub reason: String,
    pub lines: Vec<CreditLineInput>,
}

impl CreditNoteRequest {
    pub fn validate(self, today: NaiveDate) -> AppResult<(NaiveDate, String, Vec<CreditLineInput>)> {
        let date = self.credit_date.unwrap_or(today);
        if date > today {
            return Err(AppError::validation("credit_date can't be in the future"));
        }
        if self.lines.is_empty() || self.lines.len() > MAX_LINES {
            return Err(AppError::validation("A credit note needs at least one line"));
        }
        if self.lines.iter().map(|l| l.invoice_line_id).collect::<HashSet<_>>().len() != self.lines.len() {
            return Err(AppError::validation("Credit each line once per credit note"));
        }
        if self.lines.iter().any(|l| l.amount_cents.is_some_and(|a| a <= 0)) {
            return Err(AppError::validation("Credit amounts must be more than zero"));
        }
        Ok((date, validation::text("reason", &self.reason, 500)?, self.lines))
    }
}

// --- responses -------------------------------------------------------------------

#[derive(Serialize)]
pub struct ProviderRef {
    pub id: Uuid,
    pub name: String,
    pub practice_number: Option<String>,
    pub hpcsa_number: Option<String>,
}

#[derive(Serialize)]
pub struct CentreRef {
    pub id: Uuid,
    pub kind: String,
    pub name: String,
}

#[derive(Serialize, sqlx::FromRow)]
pub struct LineResponse {
    #[serde(skip)]
    pub invoice_id: Uuid,
    pub id: Uuid,
    pub line_no: i16,
    pub service_date: NaiveDate,
    pub tariff_code: Option<String>,
    pub nappi_code: Option<String>,
    pub description: String,
    pub quantity: Decimal,
    pub unit_price_cents: i64,
    pub amount_cents: i64,
    pub credited_cents: i64,
    pub place_of_service: String,
    pub icd10_codes: Vec<String>,
    pub modifiers: Vec<String>,
    pub assistant_provider_id: Option<Uuid>,
}

#[derive(Serialize, sqlx::FromRow)]
pub struct CreditNoteLineResponse {
    #[serde(skip)]
    pub credit_note_id: Uuid,
    pub invoice_line_id: Uuid,
    pub amount_cents: i64,
}

#[derive(Serialize)]
pub struct CreditNoteResponse {
    pub id: Uuid,
    pub credit_note_no: i64,
    pub credit_date: NaiveDate,
    pub reason: String,
    pub total_cents: i64,
    pub lines: Vec<CreditNoteLineResponse>,
}

#[derive(Serialize)]
pub struct InvoiceResponse {
    pub id: Uuid,
    pub invoice_no: i64,
    pub account_id: Uuid,
    pub patient_id: Uuid,
    pub invoice_date: NaiveDate,
    pub treating_provider: ProviderRef,
    pub referring_provider: Option<ProviderRef>,
    pub service_centre: CentreRef,
    pub authorisation_number: Option<String>,
    pub total_cents: i64,
    pub credited_cents: i64,
    pub medical_aid_due_cents: i64,
    pub patient_due_cents: i64,
    pub lines: Vec<LineResponse>,
    pub credit_notes: Vec<CreditNoteResponse>,
    pub claims: Vec<ClaimResponse>,
    pub created_at: DateTime<Utc>,
}

#[derive(Serialize, sqlx::FromRow)]
pub struct InvoiceSummary {
    pub id: Uuid,
    pub invoice_no: i64,
    pub invoice_date: NaiveDate,
    pub patient_id: Uuid,
    pub total_cents: i64,
    pub medical_aid_due_cents: i64,
    pub patient_due_cents: i64,
    pub claim_status: Option<String>,
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use serde_json::{Value, json};

    use super::*;
    use crate::common::test_support::today;

    fn base(lines: Value) -> Value {
        json!({
            "account_id": Uuid::nil(), "patient_id": Uuid::nil(), "treating_provider_id": Uuid::from_u128(1),
            "service_centre_id": Uuid::nil(), "lines": lines
        })
    }

    fn line(extra: Value) -> Value {
        let mut l = json!({"tariff_code": "0190", "unit_price_cents": 52000, "icd10_codes": ["J06.9"]});
        l.as_object_mut().unwrap().extend(extra.as_object().unwrap().clone());
        l
    }

    fn validate(v: Value) -> AppResult<NewInvoice> {
        serde_json::from_value::<CreateInvoiceRequest>(v).unwrap().validate(today())
    }

    fn err(v: Value) -> String {
        validate(v).err().expect("expected an error").to_string()
    }

    #[test]
    fn minimal_invoice_defaults() {
        let i = validate(base(json!([line(json!({}))]))).unwrap();
        assert_eq!(i.invoice_date, today());
        let l = &i.lines[0];
        assert_eq!(l.service_date, today());
        assert_eq!(l.item, LineItem::Tariff { code_set: "CCSA".into(), code: "0190".into() });
        assert_eq!(l.quantity, Decimal::ONE);
        assert_eq!(l.amount_cents, 52000);
        assert_eq!(l.place_of_service, None, "filled from the service centre later");
        assert_eq!(i.total_cents(), 52000);
    }

    #[test]
    fn line_amount_rounds_half_away_from_zero() {
        assert_eq!(line_amount(Decimal::from_str("1.5").unwrap(), 333), Some(500)); // 499.5 → 500
        assert_eq!(line_amount(Decimal::from_str("0.333").unwrap(), 100), Some(33));
        assert_eq!(line_amount(Decimal::from_str("2").unwrap(), 0), Some(0));
        assert_eq!(line_amount(Decimal::MAX, i64::MAX), None, "overflow is detected, not a panic");
    }

    #[test]
    fn quantity_accepts_strings_and_numbers() {
        let a = validate(base(json!([line(json!({"quantity": "1.5", "unit_price_cents": 1000}))]))).unwrap();
        let b = validate(base(json!([line(json!({"quantity": 1.5, "unit_price_cents": 1000}))]))).unwrap();
        assert_eq!(a.lines[0].amount_cents, 1500);
        assert_eq!(b.lines[0].amount_cents, 1500);
    }

    #[test]
    fn explicit_amount_overrides_calculation() {
        let i = validate(base(json!([line(json!({"amount_cents": 39000, "modifiers": ["0018"]}))]))).unwrap();
        assert_eq!(i.lines[0].amount_cents, 39000);
        assert_eq!(i.lines[0].modifiers, vec!["0018"]);
    }

    #[test]
    fn nappi_lines() {
        let i = validate(base(json!([{"nappi_code": "716541001", "unit_price_cents": 1250, "quantity": 2, "icd10_codes": ["J06.9"]}]))).unwrap();
        assert_eq!(i.lines[0].item, LineItem::Nappi("716541001".into()));
        assert_eq!(i.lines[0].amount_cents, 2500);
    }

    #[test]
    fn exactly_one_item_code() {
        assert!(err(base(json!([line(json!({"nappi_code": "716541"}))]))).contains("exactly one"));
        assert!(err(base(json!([{"unit_price_cents": 1, "icd10_codes": ["J06.9"]}]))).contains("exactly one"));
        assert!(err(base(json!([line(json!({"tariff_code": "01"}))]))).contains("tariff_code"));
        assert!(err(base(json!([{"nappi_code": "12345", "unit_price_cents": 1, "icd10_codes": ["J06.9"]}]))).contains("nappi_code"));
    }

    #[test]
    fn diagnoses_required_normalised_and_unique() {
        assert!(err(base(json!([line(json!({"icd10_codes": []}))]))).contains("icd10_codes"));
        let i = validate(base(json!([line(json!({"icd10_codes": ["j069", "Z00.0"]}))]))).unwrap();
        assert_eq!(i.lines[0].icd10_codes, vec!["J06.9", "Z00.0"]);
        assert!(err(base(json!([line(json!({"icd10_codes": ["J06.9", "j069"]}))]))).contains("duplicate"));
        let many: Vec<String> = (0..13).map(|n| format!("A{:02}", n)).collect();
        assert!(validate(base(json!([line(json!({"icd10_codes": many}))]))).is_err());
    }

    #[test]
    fn modifiers_are_four_digits_and_unique() {
        assert!(validate(base(json!([line(json!({"modifiers": ["18"]}))]))).is_err());
        assert!(validate(base(json!([line(json!({"modifiers": ["0018", "0018"]}))]))).is_err());
    }

    #[test]
    fn dates() {
        assert!(err(base(json!([line(json!({"service_date": "2026-11-08"}))]))).contains("after the invoice date"));
        let mut v = base(json!([line(json!({}))]));
        v["invoice_date"] = json!("2026-11-08");
        assert!(err(v).contains("future"));
        let mut v = base(json!([line(json!({"service_date": "2026-11-01"}))]));
        v["invoice_date"] = json!("2026-11-02");
        assert!(validate(v).is_ok(), "services before the invoice date are normal");
    }

    #[test]
    fn quantities_and_amounts_are_bounded() {
        for q in [json!(0), json!(-1), json!("0.0001"), json!(10_000_000)] {
            assert!(validate(base(json!([line(json!({"quantity": q}))]))).is_err(), "{q}");
        }
        assert!(validate(base(json!([line(json!({"unit_price_cents": -1}))]))).is_err());
        assert!(validate(base(json!([line(json!({"amount_cents": -1}))]))).is_err());
    }

    #[test]
    fn zero_total_is_rejected() {
        assert!(err(base(json!([line(json!({"unit_price_cents": 0}))]))).contains("more than zero"));
    }

    #[test]
    fn line_errors_name_the_line() {
        let e = err(base(json!([line(json!({})), line(json!({"icd10_codes": []}))])));
        assert!(e.starts_with("lines[1]: "), "{e}");
    }

    #[test]
    fn line_count_bounds() {
        assert!(validate(base(json!([]))).is_err());
        let many: Vec<Value> = (0..=MAX_LINES).map(|_| line(json!({}))).collect();
        assert!(validate(base(json!(many))).is_err());
    }

    #[test]
    fn referring_and_authorisation() {
        let mut v = base(json!([line(json!({}))]));
        v["referring_provider_id"] = json!(Uuid::from_u128(1));
        assert!(err(v).contains("referring"));
        let mut v = base(json!([line(json!({}))]));
        v["authorisation_number"] = json!(" AUTH 123/45 ");
        assert_eq!(validate(v).unwrap().authorisation_number.as_deref(), Some("AUTH123/45"));
        let mut v = base(json!([line(json!({}))]));
        v["authorisation_number"] = json!("bad#auth");
        assert!(validate(v).is_err());
    }

    #[test]
    fn claim_transitions_match_the_database() {
        use ClaimStatus::*;
        let all = [Submitted, Accepted, Rejected, Paid];
        let allowed = [(Submitted, Accepted), (Submitted, Rejected), (Submitted, Paid), (Accepted, Paid), (Accepted, Rejected)];
        for from in all {
            for to in all {
                assert_eq!(from.can_become(to), allowed.contains(&(from, to)), "{from:?} -> {to:?}");
            }
            assert_eq!(ClaimStatus::from_db(from.as_str()).unwrap(), from);
        }
        assert!(Submitted.is_open() && Accepted.is_open() && !Rejected.is_open() && !Paid.is_open());
    }

    #[test]
    fn claim_status_follows_payment() {
        use ClaimStatus::*;
        assert_eq!(claim_status_after_payment(Submitted, 0), Some(Paid));
        assert_eq!(claim_status_after_payment(Submitted, 1500), Some(Accepted), "part-paid implies accepted");
        assert_eq!(claim_status_after_payment(Accepted, 1500), None);
        assert_eq!(claim_status_after_payment(Accepted, -10), Some(Paid));
        assert_eq!(claim_status_after_payment(Rejected, 0), None, "closed claims are left alone");
        assert_eq!(claim_status_after_payment(Paid, 0), None);
    }

    #[test]
    fn credit_note_request_rules() {
        let c = |v| serde_json::from_value::<CreditNoteRequest>(v).unwrap().validate(today());
        let l = Uuid::from_u128(1);
        assert!(c(json!({"reason": "billed twice", "lines": [{"invoice_line_id": l}]})).is_ok());
        assert!(c(json!({"reason": " ", "lines": [{"invoice_line_id": l}]})).is_err());
        assert!(c(json!({"reason": "x", "lines": []})).is_err());
        assert!(c(json!({"reason": "x", "lines": [{"invoice_line_id": l}, {"invoice_line_id": l}]})).is_err());
        assert!(c(json!({"reason": "x", "lines": [{"invoice_line_id": l, "amount_cents": 0}]})).is_err());
        assert!(c(json!({"reason": "x", "credit_date": "2026-11-08", "lines": [{"invoice_line_id": l}]})).is_err());
    }
}
