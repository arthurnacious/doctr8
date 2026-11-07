use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::common::{
    error::{AppError, AppResult},
    password, validation,
};

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct UserResponse {
    pub id: Uuid,
    pub email: String,
    pub full_name: String,
    pub email_verified_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct RegisterRequest {
    pub email: String,
    pub full_name: String,
    pub password: String,
}

pub struct NewUser {
    pub email: String,
    pub full_name: String,
    pub password: String,
}

impl RegisterRequest {
    pub fn validate(self) -> AppResult<NewUser> {
        let len = self.password.chars().count();
        if !(password::MIN_LENGTH..=password::MAX_LENGTH).contains(&len) {
            return Err(AppError::validation(format!(
                "password must be {}-{} characters",
                password::MIN_LENGTH,
                password::MAX_LENGTH
            )));
        }
        Ok(NewUser {
            email: validation::email(&self.email)?,
            full_name: validation::text("full_name", &self.full_name, 200)?,
            password: self.password,
        })
    }
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct RoleRef {
    pub id: Uuid,
    pub name: String,
}

#[derive(Debug, Serialize)]
pub struct MemberResponse {
    pub user_id: Uuid,
    pub email: String,
    pub full_name: String,
    pub status: String,
    pub joined_at: Option<DateTime<Utc>>,
    pub roles: Vec<RoleRef>,
}

#[derive(Debug, Deserialize)]
pub struct SetMemberRolesRequest {
    /// The complete set of roles the member should have (replaces existing).
    pub role_ids: Vec<Uuid>,
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn register(password: &str) -> AppResult<NewUser> {
        serde_json::from_value::<RegisterRequest>(json!({
            "email": " Thabo@Example.CO.ZA ", "full_name": " Thabo M ", "password": password
        }))
        .unwrap()
        .validate()
    }

    #[test]
    fn normalises_email_and_name() {
        let u = register("correct horse battery").unwrap();
        assert_eq!(u.email, "thabo@example.co.za");
        assert_eq!(u.full_name, "Thabo M");
    }

    #[test]
    fn password_length_bounds_count_characters() {
        assert!(register(&"a".repeat(11)).is_err());
        assert!(register(&"a".repeat(12)).is_ok());
        assert!(register(&"a".repeat(256)).is_ok());
        assert!(register(&"a".repeat(257)).is_err());
        assert!(register(&"é".repeat(12)).is_ok(), "12 characters, 24 bytes");
    }

    #[test]
    fn password_is_not_trimmed() {
        assert_eq!(register("  spaces matter  ").unwrap().password, "  spaces matter  ");
    }

    #[test]
    fn error_never_echoes_password() {
        let msg = register("hunter2").err().unwrap().to_string();
        assert!(!msg.contains("hunter2"));
    }

    #[test]
    fn rejects_bad_email_or_blank_name() {
        let bad = |email: &str, name: &str| {
            serde_json::from_value::<RegisterRequest>(json!({"email": email, "full_name": name, "password": "correct horse battery"}))
                .unwrap()
                .validate()
                .is_err()
        };
        assert!(bad("nope", "A"));
        assert!(bad("a@b.co", "  "));
    }
}
