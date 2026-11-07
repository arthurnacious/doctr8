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
