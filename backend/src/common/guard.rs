//! Permission guard as an extractor — the NestJS `@UseGuards(...)` equivalent.
//!
//! ```ignore
//! async fn create(auth: Require<perm::RolesManage>, ...) { auth.user ... }
//! ```
//! The request is rejected with 401/403 before the handler body runs.

use std::{marker::PhantomData, ops::Deref};

use axum::{
    extract::{FromRef, FromRequestParts},
    http::request::Parts,
};

use crate::common::{
    auth::TenantUser,
    error::AppError,
    jwt::JwtKeys,
    permission::PermissionMarker,
};

pub struct Require<P: PermissionMarker> {
    pub user: TenantUser,
    _permission: PhantomData<fn() -> P>,
}

impl<P: PermissionMarker> Deref for Require<P> {
    type Target = TenantUser;
    fn deref(&self) -> &TenantUser {
        &self.user
    }
}

impl<P, S> FromRequestParts<S> for Require<P>
where
    P: PermissionMarker,
    S: Send + Sync,
    JwtKeys: FromRef<S>,
{
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let user = TenantUser::from_request_parts(parts, state).await?;
        if !user.has(P::PERMISSION) {
            return Err(AppError::forbidden());
        }
        Ok(Self { user, _permission: PhantomData })
    }
}

#[cfg(test)]
mod tests {
    use axum::http::{Request, header::AUTHORIZATION};

    use super::*;
    use crate::common::{
        permission::{Permission, perm},
        test_support::{TENANT, keys, token},
    };

    async fn guard<P: PermissionMarker>(tenant: Option<uuid::Uuid>, perms: Option<&[Permission]>) -> Result<Require<P>, AppError> {
        let mut builder = Request::builder();
        if let Some(perms) = perms {
            builder = builder.header(AUTHORIZATION, format!("Bearer {}", token(tenant, perms)));
        }
        let mut parts = builder.body(()).unwrap().into_parts().0;
        Require::<P>::from_request_parts(&mut parts, &keys()).await
    }

    #[tokio::test]
    async fn passes_with_the_required_permission() {
        let ok = guard::<perm::RolesManage>(Some(TENANT), Some(&[Permission::RolesManage])).await.unwrap();
        assert_eq!(ok.tenant_id, TENANT, "derefs to the TenantUser");
        assert!(ok.user.has(Permission::RolesManage));
    }

    #[tokio::test]
    async fn forbids_without_the_required_permission() {
        let everything_else: Vec<Permission> =
            Permission::ALL.iter().copied().filter(|p| *p != Permission::RolesManage).collect();
        let err = guard::<perm::RolesManage>(Some(TENANT), Some(&everything_else)).await.err().unwrap();
        assert!(matches!(err, AppError::Forbidden(_)));
    }

    #[tokio::test]
    async fn view_permission_does_not_imply_manage() {
        let err = guard::<perm::PatientsManage>(Some(TENANT), Some(&[Permission::PatientsView])).await.err().unwrap();
        assert!(matches!(err, AppError::Forbidden(_)));
    }

    #[tokio::test]
    async fn forbids_without_a_practice_even_with_all_permissions() {
        let err = guard::<perm::RolesView>(None, Some(Permission::ALL)).await.err().unwrap();
        assert!(matches!(err, AppError::Forbidden("Select a practice first")));
    }

    #[tokio::test]
    async fn unauthorized_without_a_token() {
        let err = guard::<perm::RolesView>(None, None).await.err().unwrap();
        assert!(matches!(err, AppError::Unauthorized));
    }
}
