use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    common::permission::Permission,
    modules::access::{tenants::dto::PracticeSummary, users::dto::UserResponse},
};

#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    pub email: String,
    pub password: String,
    /// Optionally go straight into a practice. If omitted and the user
    /// belongs to exactly one, that one is selected.
    #[serde(default)]
    pub tenant_id: Option<Uuid>,
}

#[derive(Debug, Deserialize)]
pub struct SwitchPracticeRequest {
    pub tenant_id: Uuid,
}

#[derive(Debug, Serialize)]
pub struct AuthResponse {
    pub access_token: String,
    pub token_type: &'static str,
    pub expires_in: i64,
    pub user: UserResponse,
    /// The practice this token is for; `null` means "show the picker".
    pub practice: Option<PracticeSummary>,
    pub permissions: Vec<Permission>,
    /// Every practice the user can switch to.
    pub practices: Vec<PracticeSummary>,
}

/// What the service hands back to the handler: the JSON body plus the new
/// refresh token, which the handler puts in a cookie.
pub struct SessionTokens {
    pub body: AuthResponse,
    pub refresh_token: String,
    pub refresh_max_age_secs: i64,
}
