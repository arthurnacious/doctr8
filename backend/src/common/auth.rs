//! JWT extractors.
//!
//! * `AuthUser`   — any valid access token (practice may not be selected yet).
//! * `TenantUser` — valid access token *with* a selected practice.

use std::collections::HashSet;

use axum::{
    extract::{FromRef, FromRequestParts},
    http::{header::AUTHORIZATION, request::Parts},
};
use uuid::Uuid;

use crate::common::{
    db::Scope,
    error::AppError,
    jwt::JwtKeys,
    permission::Permission,
};

#[derive(Debug, Clone)]
pub struct AuthUser {
    pub user_id: Uuid,
    pub session_id: Uuid,
    pub tenant_id: Option<Uuid>,
    pub permissions: HashSet<Permission>,
}

impl AuthUser {
    pub fn scope(&self) -> Scope {
        Scope { user_id: Some(self.user_id), tenant_id: self.tenant_id }
    }
}

impl<S> FromRequestParts<S> for AuthUser
where
    S: Send + Sync,
    JwtKeys: FromRef<S>,
{
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        // Decode once per request even if several extractors ask.
        if let Some(user) = parts.extensions.get::<AuthUser>() {
            return Ok(user.clone());
        }

        let token = parts
            .headers
            .get(AUTHORIZATION)
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.strip_prefix("Bearer "))
            .ok_or(AppError::Unauthorized)?;

        let claims = JwtKeys::from_ref(state).verify(token)?;
        let user = AuthUser {
            user_id: claims.sub,
            session_id: claims.sid,
            tenant_id: claims.tid,
            permissions: claims.perms.iter().filter_map(|k| Permission::from_key(k)).collect(),
        };
        parts.extensions.insert(user.clone());
        Ok(user)
    }
}

#[derive(Debug, Clone)]
pub struct TenantUser {
    pub user_id: Uuid,
    pub session_id: Uuid,
    pub tenant_id: Uuid,
    pub permissions: HashSet<Permission>,
}

impl TenantUser {
    pub fn has(&self, permission: Permission) -> bool {
        self.permissions.contains(&permission)
    }

    pub fn scope(&self) -> Scope {
        Scope::tenant(self.user_id, self.tenant_id)
    }
}

impl<S> FromRequestParts<S> for TenantUser
where
    S: Send + Sync,
    JwtKeys: FromRef<S>,
{
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let user = AuthUser::from_request_parts(parts, state).await?;
        let tenant_id = user
            .tenant_id
            .ok_or(AppError::Forbidden("Select a practice first"))?;
        Ok(TenantUser {
            user_id: user.user_id,
            session_id: user.session_id,
            tenant_id,
            permissions: user.permissions,
        })
    }
}
