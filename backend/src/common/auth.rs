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

#[cfg(test)]
mod tests {
    use axum::http::Request;

    use super::*;
    use crate::common::test_support::{SESSION, TENANT, USER, keys, token};

    fn parts(authorization: Option<&str>) -> Parts {
        let mut builder = Request::builder().uri("/x");
        if let Some(value) = authorization {
            builder = builder.header(AUTHORIZATION, value);
        }
        builder.body(()).unwrap().into_parts().0
    }

    async fn auth_user(authorization: Option<&str>) -> Result<AuthUser, AppError> {
        AuthUser::from_request_parts(&mut parts(authorization), &keys()).await
    }

    async fn tenant_user(authorization: Option<&str>) -> Result<TenantUser, AppError> {
        TenantUser::from_request_parts(&mut parts(authorization), &keys()).await
    }

    #[tokio::test]
    async fn missing_or_malformed_header_is_unauthorized() {
        let good = token(None, &[]);
        for header in [None, Some(""), Some("Basic dXNlcjpwYXNz"), Some(good.as_str()), Some("Bearer "), Some("bearer x")] {
            assert!(matches!(auth_user(header).await, Err(AppError::Unauthorized)), "{header:?}");
        }
    }

    #[tokio::test]
    async fn valid_token_yields_identity_and_permissions() {
        let header = format!("Bearer {}", token(Some(TENANT), &[Permission::RolesView]));
        let user = auth_user(Some(&header)).await.unwrap();
        assert_eq!(user.user_id, USER);
        assert_eq!(user.session_id, SESSION);
        assert_eq!(user.tenant_id, Some(TENANT));
        assert_eq!(user.permissions, HashSet::from([Permission::RolesView]));
        assert_eq!(user.scope().tenant_id, Some(TENANT));
        assert_eq!(user.scope().user_id, Some(USER));
    }

    #[tokio::test]
    async fn unknown_permission_keys_in_token_are_ignored() {
        // A token issued before a permission was removed from the code.
        let mut claims = keys().verify(&token(Some(TENANT), &[Permission::RolesView])).unwrap();
        claims.perms.push("legacy.removed".into());
        let forged = jsonwebtoken::encode(
            &jsonwebtoken::Header::default(),
            &claims,
            &jsonwebtoken::EncodingKey::from_secret(crate::common::test_support::SECRET.as_bytes()),
        )
        .unwrap();
        let user = auth_user(Some(&format!("Bearer {forged}"))).await.unwrap();
        assert_eq!(user.permissions, HashSet::from([Permission::RolesView]));
    }

    #[tokio::test]
    async fn identity_is_cached_per_request() {
        let header = format!("Bearer {}", token(None, &[]));
        let mut p = parts(Some(&header));
        AuthUser::from_request_parts(&mut p, &keys()).await.unwrap();
        assert!(p.extensions.get::<AuthUser>().is_some());
        // Second extraction succeeds from cache even after the header is gone.
        p.headers.remove(AUTHORIZATION);
        assert!(AuthUser::from_request_parts(&mut p, &keys()).await.is_ok());
    }

    #[tokio::test]
    async fn tenant_user_requires_a_selected_practice() {
        let no_practice = format!("Bearer {}", token(None, Permission::ALL));
        assert!(matches!(tenant_user(Some(&no_practice)).await, Err(AppError::Forbidden(_))));

        let with_practice = format!("Bearer {}", token(Some(TENANT), &[Permission::PatientsView]));
        let user = tenant_user(Some(&with_practice)).await.unwrap();
        assert_eq!(user.tenant_id, TENANT);
        assert!(user.has(Permission::PatientsView));
        assert!(!user.has(Permission::PatientsManage));
        let scope = user.scope();
        assert_eq!((scope.user_id, scope.tenant_id), (Some(USER), Some(TENANT)));
    }

    #[tokio::test]
    async fn tenant_user_without_token_is_unauthorized_not_forbidden() {
        assert!(matches!(tenant_user(None).await, Err(AppError::Unauthorized)));
    }
}
