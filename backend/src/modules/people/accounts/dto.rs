use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::common::{
    error::AppResult,
    validation::{self, double_option},
};

#[derive(Serialize)]
pub struct AccountResponse {
    pub id: Uuid,
    pub account_no: i64,
    /// The mode this account was opened under (fixed for its lifetime).
    pub file_mode: String,
    pub status: String,
    pub guarantor: PersonRef,
    /// `null` = private account.
    pub medical_aid: Option<MedicalAidRef>,
    pub patients: Vec<AccountPatient>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Serialize)]
pub struct PersonRef {
    pub id: Uuid,
    pub first_name: String,
    pub last_name: String,
}

#[derive(Serialize)]
pub struct MedicalAidRef {
    pub membership_id: Uuid,
    pub scheme_name: String,
    pub plan_name: String,
    pub membership_number: String,
    pub main_member_person_id: Uuid,
}

#[derive(Serialize, sqlx::FromRow)]
pub struct AccountPatient {
    pub patient_id: Uuid,
    pub person_id: Uuid,
    pub first_name: String,
    pub last_name: String,
    pub dependant_code: String,
    pub status: String,
}

#[derive(Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AccountStatus {
    Open,
    Closed,
}

impl AccountStatus {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Open => "open",
            Self::Closed => "closed",
        }
    }
}

#[derive(Deserialize)]
pub struct UpdateAccountRequest {
    #[serde(default)]
    pub guarantor_person_id: Option<Uuid>,
    /// Omit to leave unchanged, `null` to make the account private.
    #[serde(default, deserialize_with = "double_option")]
    pub membership_id: Option<Option<Uuid>>,
    #[serde(default)]
    pub status: Option<AccountStatus>,
}

#[derive(Deserialize)]
pub struct AddPatientRequest {
    pub person_id: Uuid,
    /// Defaults to 00 for the principal member; on private accounts other
    /// patients get the next free code. Required for medical aid dependants.
    #[serde(default)]
    pub dependant_code: Option<String>,
}

impl AddPatientRequest {
    pub fn validated_code(&self) -> AppResult<Option<String>> {
        self.dependant_code.as_deref().map(validation::dependant_code).transpose()
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn update_membership_absent_null_value() {
        let u = |v| serde_json::from_value::<UpdateAccountRequest>(v).unwrap();
        assert!(u(json!({})).membership_id.is_none());
        assert_eq!(u(json!({"membership_id": null})).membership_id, Some(None), "null = make private");
        assert_eq!(u(json!({"membership_id": Uuid::from_u128(5)})).membership_id, Some(Some(Uuid::from_u128(5))));
    }

    #[test]
    fn statuses() {
        let u: UpdateAccountRequest = serde_json::from_value(json!({"status": "closed"})).unwrap();
        assert_eq!(u.status.map(AccountStatus::as_str), Some("closed"));
        assert_eq!(AccountStatus::Open.as_str(), "open");
        assert!(serde_json::from_value::<UpdateAccountRequest>(json!({"status": "deleted"})).is_err());
    }

    #[test]
    fn add_patient_code_validation() {
        let r = |v| serde_json::from_value::<AddPatientRequest>(v).unwrap();
        assert_eq!(r(json!({"person_id": Uuid::nil()})).validated_code().unwrap(), None);
        assert_eq!(r(json!({"person_id": Uuid::nil(), "dependant_code": "03"})).validated_code().unwrap().as_deref(), Some("03"));
        assert!(r(json!({"person_id": Uuid::nil(), "dependant_code": "3"})).validated_code().is_err());
    }
}
