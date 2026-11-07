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
