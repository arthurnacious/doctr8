use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    common::{
        error::{AppError, AppResult},
        validation::{self, double_option},
    },
    modules::people::{
        memberships::dto::NewMembership,
        persons::dto::{NewPerson, PersonInput},
    },
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
    /// Open a new membership and switch the account to it, in one step. The
    /// main member is the file's principal (its 00 patient). Can't be sent
    /// with `membership_id`.
    #[serde(default)]
    pub new_membership: Option<NewAccountMembership>,
}

#[derive(Deserialize)]
pub struct NewAccountMembership {
    pub scheme_id: Uuid,
    pub plan_id: Uuid,
    pub membership_number: String,
}

impl NewAccountMembership {
    pub fn validate(self, main_member_person_id: Uuid) -> AppResult<NewMembership> {
        Ok(NewMembership {
            scheme_id: self.scheme_id,
            plan_id: self.plan_id,
            membership_number: validation::membership_number(&self.membership_number)?,
            main_member_person_id,
        })
    }
}

impl UpdateAccountRequest {
    pub fn check(&self) -> AppResult<()> {
        if self.new_membership.is_some() && self.membership_id.is_some() {
            return Err(AppError::validation("Send membership_id or new_membership, not both"));
        }
        Ok(())
    }
}

/// Add someone already in the practice (`person_id`) or a new person
/// (`person`, created in the same transaction so a rejected add leaves no
/// orphan). Exactly one of the two.
#[derive(Deserialize)]
pub struct AddPatientRequest {
    #[serde(default)]
    pub person_id: Option<Uuid>,
    #[serde(default)]
    pub person: Option<PersonInput>,
    /// Defaults to 00 for the principal member; on private accounts other
    /// patients get the next free code. Required for medical aid dependants.
    #[serde(default)]
    pub dependant_code: Option<String>,
}

pub enum NewPatientPerson {
    Existing(Uuid),
    New(Box<NewPerson>),
}

impl AddPatientRequest {
    pub fn validated_code(&self) -> AppResult<Option<String>> {
        self.dependant_code.as_deref().map(validation::dependant_code).transpose()
    }

    pub fn validate(self, today: NaiveDate) -> AppResult<(NewPatientPerson, Option<String>)> {
        let code = self.validated_code()?;
        let person = match (self.person_id, self.person) {
            (Some(id), None) => NewPatientPerson::Existing(id),
            (None, Some(p)) => NewPatientPerson::New(Box::new(p.validate(today).map_err(|e| prefixed("person", e))?)),
            _ => return Err(AppError::validation("Send person_id or person, exactly one")),
        };
        Ok((person, code))
    }
}

fn prefixed(field: &str, e: AppError) -> AppError {
    match e {
        AppError::Validation(m) => AppError::Validation(format!("{field}: {m}")),
        other => other,
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

    fn today() -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, 1, 1).unwrap()
    }

    #[test]
    fn add_patient_takes_exactly_one_person() {
        let r = |v| serde_json::from_value::<AddPatientRequest>(v).unwrap().validate(today());
        assert!(matches!(r(json!({"person_id": Uuid::nil()})), Ok((NewPatientPerson::Existing(_), None))));
        assert!(matches!(
            r(json!({"person": {"first_name": "Kid", "last_name": "Dube"}, "dependant_code": "02"})),
            Ok((NewPatientPerson::New(_), Some(ref c))) if c == "02"
        ));
        assert!(r(json!({})).is_err(), "neither");
        assert!(r(json!({"person_id": Uuid::nil(), "person": {"first_name": "A", "last_name": "B"}})).is_err(), "both");
    }

    #[test]
    fn add_patient_new_person_errors_name_the_field() {
        let e = serde_json::from_value::<AddPatientRequest>(json!({"person": {"first_name": "", "last_name": "B"}}))
            .unwrap()
            .validate(today())
            .err()
            .unwrap()
            .to_string();
        assert!(e.contains("person: "), "{e}");
    }

    #[test]
    fn new_membership_and_membership_id_are_exclusive() {
        let u = |v| serde_json::from_value::<UpdateAccountRequest>(v).unwrap();
        let m = json!({"scheme_id": Uuid::from_u128(1), "plan_id": Uuid::from_u128(2), "membership_number": " ab 12 "});
        assert!(u(json!({"new_membership": m})).check().is_ok());
        assert!(u(json!({"new_membership": m, "membership_id": null})).check().is_err());
        let n = u(json!({"new_membership": m})).new_membership.unwrap().validate(Uuid::from_u128(3)).unwrap();
        assert_eq!((n.membership_number.as_str(), n.main_member_person_id), ("AB12", Uuid::from_u128(3)));
    }

    #[test]
    fn add_patient_code_validation() {
        let r = |v| serde_json::from_value::<AddPatientRequest>(v).unwrap();
        assert_eq!(r(json!({"person_id": Uuid::nil()})).validated_code().unwrap(), None);
        assert_eq!(r(json!({"person_id": Uuid::nil(), "dependant_code": "03"})).validated_code().unwrap().as_deref(), Some("03"));
        assert!(r(json!({"person_id": Uuid::nil(), "dependant_code": "3"})).validated_code().is_err());
    }
}
