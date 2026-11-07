use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::common::{
    error::AppResult,
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

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct TenantResponse {
    pub id: Uuid,
    pub slug: String,
    pub name: String,
    pub pcns_number: Option<String>,
    pub status: String,
    pub file_mode: String,
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
}

pub struct TenantChanges {
    pub name: Option<String>,
    pub pcns_number: Option<Option<String>>,
    pub file_mode: Option<FileMode>,
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
        })
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

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
