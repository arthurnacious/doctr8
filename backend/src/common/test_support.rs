//! Shared helpers for unit tests.

use std::collections::HashSet;

use chrono::NaiveDate;
use uuid::Uuid;

use crate::{
    common::{auth::TenantUser, jwt::JwtKeys, permission::Permission},
    config::Config,
};

pub const SECRET: &str = "unit-test-secret-unit-test-secret-0123";
pub const ISSUER: &str = "doctr8-test";

pub const USER: Uuid = Uuid::from_u128(0xA11CE);
pub const SESSION: Uuid = Uuid::from_u128(0x5E55);
pub const TENANT: Uuid = Uuid::from_u128(0x7E4A47);

pub fn config() -> Config {
    Config {
        bind_addr: "127.0.0.1:0".into(),
        // Nothing listens on port 1: any test that reaches the database fails fast.
        database_url: "postgres://doctr8_app:x@127.0.0.1:1/doctr8".into(),
        database_role: None,
        db_max_connections: 1,
        jwt_secret: SECRET.into(),
        jwt_issuer: ISSUER.into(),
        access_token_ttl_secs: 600,
        refresh_token_ttl_secs: 3600,
        session_ttl_secs: 7200,
        cookie_secure: true,
    }
}

pub fn keys() -> JwtKeys {
    JwtKeys::new(SECRET.as_bytes(), ISSUER, 600)
}

pub fn token(tenant: Option<Uuid>, perms: &[Permission]) -> String {
    keys().issue(USER, SESSION, tenant, perms).unwrap().token
}

pub fn tenant_user(perms: &[Permission]) -> TenantUser {
    TenantUser {
        user_id: USER,
        session_id: SESSION,
        tenant_id: TENANT,
        permissions: perms.iter().copied().collect::<HashSet<_>>(),
    }
}

pub fn date(y: i32, m: u32, d: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(y, m, d).unwrap()
}

pub fn today() -> NaiveDate {
    date(2026, 11, 7)
}

/// Fake Postgres error so database-error mapping can be tested without a DB.
#[derive(Debug)]
pub struct FakeDbError {
    pub code: &'static str,
    pub constraint: Option<&'static str>,
    pub message: &'static str,
}

impl std::fmt::Display for FakeDbError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.message)
    }
}

impl std::error::Error for FakeDbError {}

impl sqlx::error::DatabaseError for FakeDbError {
    fn message(&self) -> &str {
        self.message
    }
    fn code(&self) -> Option<std::borrow::Cow<'_, str>> {
        Some(self.code.into())
    }
    fn constraint(&self) -> Option<&str> {
        self.constraint
    }
    fn table(&self) -> Option<&str> {
        Some("persons")
    }
    fn as_error(&self) -> &(dyn std::error::Error + Send + Sync + 'static) {
        self
    }
    fn as_error_mut(&mut self) -> &mut (dyn std::error::Error + Send + Sync + 'static) {
        self
    }
    fn into_error(self: Box<Self>) -> Box<dyn std::error::Error + Send + Sync + 'static> {
        self
    }
    fn kind(&self) -> sqlx::error::ErrorKind {
        sqlx::error::ErrorKind::Other
    }
}

pub fn db_error(code: &'static str, constraint: Option<&'static str>, message: &'static str) -> sqlx::Error {
    sqlx::Error::Database(Box::new(FakeDbError { code, constraint, message }))
}
