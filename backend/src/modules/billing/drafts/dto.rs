use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

use crate::{
    common::error::{AppError, AppResult},
    modules::billing::invoices::dto::CreateInvoiceRequest,
};

/// Drafts hold whatever the screen has so far, within reason.
pub const MAX_CONTENT_BYTES: usize = 200_000;

#[derive(Deserialize)]
pub struct CreateDraftRequest {
    pub account_id: Uuid,
    #[serde(default = "empty_object")]
    pub content: Value,
}

#[derive(Deserialize)]
pub struct UpdateDraftRequest {
    pub content: Value,
    /// The `updated_at` the client last saw. If someone else has saved
    /// since, the update is refused (409) instead of overwriting them.
    #[serde(default)]
    pub expected_updated_at: Option<DateTime<Utc>>,
}

fn empty_object() -> Value {
    Value::Object(Default::default())
}

#[derive(Serialize)]
pub struct DraftResponse {
    pub id: Uuid,
    pub account_id: Uuid,
    pub content: Value,
    pub created_by: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// What the invoice screen needs about the file: who can be invoiced (with
/// date of birth and sex, for ICD-10 checks) and the medical aid plan (for
/// tariff prices). Readable with billing.view alone.
#[derive(Serialize)]
pub struct BillingContext {
    pub account_id: Uuid,
    pub account_no: i64,
    pub status: String,
    pub medical_aid: Option<ContextMedicalAid>,
    pub patients: Vec<ContextPatient>,
}

#[derive(Serialize, sqlx::FromRow)]
pub struct ContextMedicalAid {
    pub plan_id: Uuid,
    pub scheme_name: String,
    pub plan_name: String,
    pub membership_number: String,
}

#[derive(Serialize, sqlx::FromRow)]
pub struct ContextPatient {
    pub patient_id: Uuid,
    pub dependant_code: String,
    pub title: Option<String>,
    pub first_name: String,
    pub last_name: String,
    pub date_of_birth: Option<NaiveDate>,
    pub sex: Option<String>,
    pub status: String,
}

/// A draft's content as stored: a JSON object, size-limited.
pub fn check_content(content: &Value) -> AppResult<String> {
    if !content.is_object() {
        return Err(AppError::validation("content must be a JSON object"));
    }
    let text = content.to_string();
    if text.len() > MAX_CONTENT_BYTES {
        return Err(AppError::validation("The draft is too large"));
    }
    Ok(text)
}

/// Read a draft as an invoice request for its account. Fields the invoice
/// doesn't know (screen-only details) are ignored.
pub fn as_invoice_request(account_id: Uuid, content: &Value) -> AppResult<CreateInvoiceRequest> {
    let mut obj = content.as_object().cloned().unwrap_or_default();
    obj.insert("account_id".into(), Value::String(account_id.to_string()));
    serde_json::from_value(Value::Object(obj)).map_err(|e| AppError::validation(incomplete(&e.to_string())))
}

/// "missing field `treating_provider_id`" → plain language.
fn incomplete(serde_message: &str) -> String {
    let field = serde_message.split('`').nth(1).unwrap_or("");
    let what = match field {
        "patient_id" => "choose the patient",
        "treating_provider_id" => "choose the treating doctor",
        "service_centre_id" => "choose the service centre",
        "lines" => "add at least one line",
        "unit_price_cents" => "every line needs a price",
        "icd10_codes" => "every line needs at least one ICD-10 code",
        _ => return format!("The draft isn't complete: {serde_message}"),
    };
    format!("The draft isn't complete: {what}")
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn content_must_be_a_bounded_object() {
        assert!(check_content(&json!({"lines": []})).is_ok());
        assert!(check_content(&json!([1, 2])).is_err());
        assert!(check_content(&json!({"x": "y".repeat(MAX_CONTENT_BYTES)})).is_err());
    }

    #[test]
    fn draft_reads_as_invoice_request_for_its_account() {
        let account = Uuid::from_u128(7);
        let content = json!({
            "patient_id": Uuid::nil(), "treating_provider_id": Uuid::from_u128(1), "service_centre_id": Uuid::nil(),
            "lines": [{"tariff_code": "0190", "unit_price_cents": 100, "icd10_codes": ["J06.9"]}],
            "account_id": Uuid::from_u128(99),
            "ui": {"descriptions": {"0190": "Consultation"}}
        });
        let r = as_invoice_request(account, &content).unwrap();
        assert_eq!(r.account_id, account, "the draft's account wins over anything in content");
        assert_eq!(r.lines.len(), 1);
    }

    #[test]
    fn incomplete_drafts_explain_what_is_missing() {
        let e = as_invoice_request(Uuid::nil(), &json!({"patient_id": Uuid::nil(), "lines": []})).err().unwrap().to_string();
        assert!(e.contains("choose the treating doctor"), "{e}");
        let e = as_invoice_request(
            Uuid::nil(),
            &json!({"patient_id": Uuid::nil(), "treating_provider_id": Uuid::nil(), "service_centre_id": Uuid::nil(),
                    "lines": [{"tariff_code": "0190", "unit_price_cents": 1}]}),
        )
        .err()
        .unwrap()
        .to_string();
        assert!(e.contains("ICD-10"), "{e}");
    }
}
