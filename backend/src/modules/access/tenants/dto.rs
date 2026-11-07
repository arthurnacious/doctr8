use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::common::{
    error::{AppError, AppResult},
    validation::{self, double_option},
};

/// How a practice organises patients into billing files (accounts).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FileMode {
    /// One patient per account.
    PatientPerFile,
    /// Many patients per account, exactly one main member (00).
    FamilyPerFile,
    /// A new account for every visit.
    DebtorPerVisit,
}

impl FileMode {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::PatientPerFile => "patient_per_file",
            Self::FamilyPerFile => "family_per_file",
            Self::DebtorPerVisit => "debtor_per_visit",
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::PatientPerFile => "patient per file",
            Self::FamilyPerFile => "family per file",
            Self::DebtorPerVisit => "new debtor per visit",
        }
    }

    pub fn from_db(value: &str) -> anyhow::Result<Self> {
        match value {
            "patient_per_file" => Ok(Self::PatientPerFile),
            "family_per_file" => Ok(Self::FamilyPerFile),
            "debtor_per_visit" => Ok(Self::DebtorPerVisit),
            other => anyhow::bail!("unknown file mode in database: {other}"),
        }
    }
}

/// A practice as shown in the practice picker.
#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct PracticeSummary {
    pub id: Uuid,
    pub slug: String,
    pub name: String,
}

/// Contact and banking details printed on statements. All optional.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, sqlx::FromRow)]
pub struct PracticeDetails {
    pub address_line1: Option<String>,
    pub address_line2: Option<String>,
    pub suburb: Option<String>,
    pub city: Option<String>,
    pub postal_code: Option<String>,
    pub phone: Option<String>,
    pub email: Option<String>,
    pub bank_name: Option<String>,
    pub bank_account_name: Option<String>,
    pub bank_account_number: Option<String>,
    pub bank_branch_code: Option<String>,
}

impl PracticeDetails {
    /// Trim, normalise and check every field; blanks become `None`.
    pub fn validate(self) -> AppResult<Self> {
        let digits = |field: &str, v: Option<String>, len: std::ops::RangeInclusive<usize>| -> AppResult<Option<String>> {
            let Some(v) = v.map(|v| v.chars().filter(|c| !c.is_whitespace() && *c != '-').collect::<String>()) else {
                return Ok(None);
            };
            if v.is_empty() {
                return Ok(None);
            }
            if !v.chars().all(|c| c.is_ascii_digit()) || !len.contains(&v.len()) {
                let (lo, hi) = (len.start(), len.end());
                let n = if lo == hi { format!("{lo}") } else { format!("{lo} to {hi}") };
                return Err(AppError::validation(format!("{field} must be {n} digits")));
            }
            Ok(Some(v))
        };
        let blank = |v: Option<String>| v.filter(|s| !s.trim().is_empty());
        Ok(Self {
            address_line1: validation::optional_text("address_line1", self.address_line1.as_deref(), 200)?,
            address_line2: validation::optional_text("address_line2", self.address_line2.as_deref(), 200)?,
            suburb: validation::optional_text("suburb", self.suburb.as_deref(), 100)?,
            city: validation::optional_text("city", self.city.as_deref(), 100)?,
            postal_code: blank(self.postal_code).map(|v| validation::postal_code(&v)).transpose()?,
            phone: blank(self.phone).map(|v| validation::phone("phone", &v)).transpose()?,
            email: blank(self.email).map(|v| validation::email(&v)).transpose()?,
            bank_name: validation::optional_text("bank_name", self.bank_name.as_deref(), 100)?,
            bank_account_name: validation::optional_text("bank_account_name", self.bank_account_name.as_deref(), 100)?,
            bank_account_number: digits("bank_account_number", self.bank_account_number, 6..=16)?,
            bank_branch_code: digits("bank_branch_code", self.bank_branch_code, 6..=6)?,
        })
    }
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct TenantResponse {
    pub id: Uuid,
    pub slug: String,
    pub name: String,
    pub pcns_number: Option<String>,
    pub status: String,
    pub file_mode: String,
    #[sqlx(flatten)]
    pub details: PracticeDetails,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct CreateTenantRequest {
    pub name: String,
    pub slug: String,
    #[serde(default)]
    pub pcns_number: Option<String>,
}

pub struct NewTenant {
    pub name: String,
    pub slug: String,
    pub pcns_number: Option<String>,
}

impl CreateTenantRequest {
    pub fn validate(self) -> AppResult<NewTenant> {
        Ok(NewTenant {
            name: validation::text("name", &self.name, 200)?,
            slug: validation::slug(&self.slug)?,
            pcns_number: match self.pcns_number.as_deref().map(str::trim) {
                None | Some("") => None,
                Some(v) => Some(validation::pcns_number(v)?),
            },
        })
    }
}

#[derive(Debug, Deserialize)]
pub struct UpdateTenantRequest {
    #[serde(default)]
    pub name: Option<String>,
    /// Omit to leave unchanged, `null` to clear.
    #[serde(default, deserialize_with = "double_option")]
    pub pcns_number: Option<Option<String>>,
    /// Applies to accounts opened from now on; existing accounts keep the
    /// mode they were opened under.
    #[serde(default)]
    pub file_mode: Option<FileMode>,
    /// Contact and banking details, replaced as a whole (send them all).
    #[serde(default)]
    pub details: Option<PracticeDetails>,
}

pub struct TenantChanges {
    pub name: Option<String>,
    pub pcns_number: Option<Option<String>>,
    pub file_mode: Option<FileMode>,
    pub details: Option<PracticeDetails>,
}

impl UpdateTenantRequest {
    pub fn validate(self) -> AppResult<TenantChanges> {
        Ok(TenantChanges {
            name: self.name.map(|n| validation::text("name", &n, 200)).transpose()?,
            pcns_number: match self.pcns_number {
                None => None,
                Some(None) => Some(None),
                Some(Some(v)) if v.trim().is_empty() => Some(None),
                Some(Some(v)) => Some(Some(validation::pcns_number(&v)?)),
            },
            file_mode: self.file_mode,
            details: self.details.map(PracticeDetails::validate).transpose()?,
        })
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    fn details(v: serde_json::Value) -> AppResult<PracticeDetails> {
        serde_json::from_value::<PracticeDetails>(v).unwrap().validate()
    }

    #[test]
    fn practice_details_normalise_and_blank_out() {
        let d = details(json!({
            "address_line1": " 1 Main Rd ", "postal_code": "4001", "phone": "031 555 1234",
            "email": " Accounts@Clinic.co.za ", "bank_name": "First Bank", "bank_account_number": "62 1234 5678",
            "bank_branch_code": "250-655", "city": "", "suburb": "  "
        }))
        .unwrap();
        assert_eq!(d.address_line1.as_deref(), Some("1 Main Rd"));
        assert_eq!(d.phone.as_deref(), Some("0315551234"));
        assert_eq!(d.email.as_deref(), Some("accounts@clinic.co.za"));
        assert_eq!(d.bank_account_number.as_deref(), Some("6212345678"));
        assert_eq!(d.bank_branch_code.as_deref(), Some("250655"));
        assert_eq!((d.city, d.suburb), (None, None), "blank = not set");
        assert_eq!(details(json!({})).unwrap(), PracticeDetails::default());
    }

    #[test]
    fn practice_details_reject_bad_bank_numbers() {
        assert!(details(json!({"bank_branch_code": "12345"})).is_err());
        assert!(details(json!({"bank_account_number": "12345"})).is_err());
        assert!(details(json!({"bank_account_number": "12a456789"})).is_err());
        assert!(details(json!({"postal_code": "400"})).is_err());
        assert!(details(json!({"email": "nope"})).is_err());
        let e = details(json!({"bank_branch_code": "1"})).unwrap_err().to_string();
        assert!(e.contains("6 digits"), "{e}");
    }

    use super::*;

    fn create(v: serde_json::Value) -> AppResult<NewTenant> {
        serde_json::from_value::<CreateTenantRequest>(v).unwrap().validate()
    }

    fn update(v: serde_json::Value) -> AppResult<TenantChanges> {
        serde_json::from_value::<UpdateTenantRequest>(v).unwrap().validate()
    }

    #[test]
    fn create_normalises() {
        let t = create(json!({"name": "  Dr A  ", "slug": "Dr-A", "pcns_number": "012 3456"})).unwrap();
        assert_eq!((t.name.as_str(), t.slug.as_str(), t.pcns_number.as_deref()), ("Dr A", "dr-a", Some("0123456")));
    }

    #[test]
    fn create_blank_pcns_is_none() {
        assert_eq!(create(json!({"name": "A", "slug": "abc", "pcns_number": "  "})).unwrap().pcns_number, None);
        assert_eq!(create(json!({"name": "A", "slug": "abc"})).unwrap().pcns_number, None);
    }

    #[test]
    fn create_rejects_bad_fields() {
        assert!(create(json!({"name": " ", "slug": "abc"})).is_err());
        assert!(create(json!({"name": "A", "slug": "a"})).is_err());
        assert!(create(json!({"name": "A", "slug": "abc", "pcns_number": "123"})).is_err());
        assert!(create(json!({"name": "x".repeat(201), "slug": "abc"})).is_err());
    }

    #[test]
    fn update_pcns_absent_null_blank_value() {
        assert!(update(json!({})).unwrap().pcns_number.is_none());
        assert_eq!(update(json!({"pcns_number": null})).unwrap().pcns_number, Some(None));
        assert_eq!(update(json!({"pcns_number": ""})).unwrap().pcns_number, Some(None));
        assert_eq!(update(json!({"pcns_number": "7654321"})).unwrap().pcns_number, Some(Some("7654321".into())));
        assert!(update(json!({"pcns_number": "x"})).is_err());
    }

    #[test]
    fn update_file_mode() {
        assert_eq!(update(json!({"file_mode": "family_per_file"})).unwrap().file_mode, Some(FileMode::FamilyPerFile));
        assert!(serde_json::from_value::<UpdateTenantRequest>(json!({"file_mode": "chaos"})).is_err());
    }

    #[test]
    fn file_mode_round_trips_through_db_and_json() {
        for m in [FileMode::PatientPerFile, FileMode::FamilyPerFile, FileMode::DebtorPerVisit] {
            assert_eq!(FileMode::from_db(m.as_str()).unwrap(), m);
            assert_eq!(serde_json::to_value(m).unwrap(), json!(m.as_str()));
            assert!(!m.label().is_empty());
        }
        assert!(FileMode::from_db("nonsense").is_err());
    }
}
