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
