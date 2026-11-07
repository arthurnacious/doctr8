use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::common::{error::AppResult, validation};

#[derive(Serialize)]
pub struct SchemeResponse {
    pub id: Uuid,
    pub name: String,
    pub code: Option<String>,
    pub plans: Vec<PlanResponse>,
}

#[derive(Serialize)]
pub struct PlanResponse {
    pub id: Uuid,
    pub name: String,
    pub code: Option<String>,
}

#[derive(Serialize, sqlx::FromRow)]
pub struct MembershipResponse {
    pub id: Uuid,
    pub scheme_id: Uuid,
    pub scheme_name: String,
    pub plan_id: Uuid,
    pub plan_name: String,
    pub membership_number: String,
    pub main_member_person_id: Uuid,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Deserialize)]
pub struct CreateMembershipRequest {
    pub scheme_id: Uuid,
    pub plan_id: Uuid,
    pub membership_number: String,
    pub main_member_person_id: Uuid,
}

pub struct NewMembership {
    pub scheme_id: Uuid,
    pub plan_id: Uuid,
    pub membership_number: String,
    pub main_member_person_id: Uuid,
}

impl CreateMembershipRequest {
    pub fn validate(self) -> AppResult<NewMembership> {
        Ok(NewMembership {
            scheme_id: self.scheme_id,
            plan_id: self.plan_id,
            membership_number: validation::membership_number(&self.membership_number)?,
            main_member_person_id: self.main_member_person_id,
        })
    }
}

/// The scheme can't change — that's a different membership.
#[derive(Deserialize)]
pub struct UpdateMembershipRequest {
    #[serde(default)]
    pub plan_id: Option<Uuid>,
    #[serde(default)]
    pub membership_number: Option<String>,
    #[serde(default)]
    pub main_member_person_id: Option<Uuid>,
}

pub struct MembershipChanges {
    pub plan_id: Option<Uuid>,
    pub membership_number: Option<String>,
    pub main_member_person_id: Option<Uuid>,
}

impl UpdateMembershipRequest {
    pub fn validate(self) -> AppResult<MembershipChanges> {
        Ok(MembershipChanges {
            plan_id: self.plan_id,
            membership_number: self
                .membership_number
                .map(|n| validation::membership_number(&n))
                .transpose()?,
            main_member_person_id: self.main_member_person_id,
        })
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn create_normalises_membership_number() {
        let m = serde_json::from_value::<CreateMembershipRequest>(json!({
            "scheme_id": Uuid::from_u128(1), "plan_id": Uuid::from_u128(2),
            "membership_number": " ab 12 34 ", "main_member_person_id": Uuid::from_u128(3)
        }))
        .unwrap()
        .validate()
        .unwrap();
        assert_eq!(m.membership_number, "AB1234");
        assert_eq!((m.scheme_id, m.plan_id, m.main_member_person_id), (Uuid::from_u128(1), Uuid::from_u128(2), Uuid::from_u128(3)));
    }

    #[test]
    fn create_rejects_bad_number() {
        let r = serde_json::from_value::<CreateMembershipRequest>(json!({
            "scheme_id": Uuid::from_u128(1), "plan_id": Uuid::from_u128(2),
            "membership_number": "ab#1", "main_member_person_id": Uuid::from_u128(3)
        }))
        .unwrap();
        assert!(r.validate().is_err());
    }

    #[test]
    fn update_leaves_absent_fields_alone() {
        let c = serde_json::from_value::<UpdateMembershipRequest>(json!({})).unwrap().validate().unwrap();
        assert!(c.plan_id.is_none() && c.membership_number.is_none() && c.main_member_person_id.is_none());
        let c = serde_json::from_value::<UpdateMembershipRequest>(json!({"membership_number": "x1"})).unwrap().validate().unwrap();
        assert_eq!(c.membership_number.as_deref(), Some("X1"));
    }

    #[test]
    fn scheme_change_is_not_accepted() {
        // Unknown fields are ignored, so scheme_id can't sneak into an update.
        let c = serde_json::from_value::<UpdateMembershipRequest>(json!({"scheme_id": Uuid::from_u128(9)})).unwrap().validate().unwrap();
        assert!(c.plan_id.is_none());
    }
}
