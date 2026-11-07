//! Row-level-security context for database work.
//!
//! Every unit of work runs in a transaction whose first statement sets
//! `app.user_id` / `app.tenant_id`. The settings are transaction-local
//! (`set_config(.., true)`), so they vanish on commit/rollback and a pooled
//! connection can never carry one request's tenant into another.

use sqlx::{PgConnection, PgPool, Postgres, Transaction};
use uuid::Uuid;

/// Who the database should consider "current" for RLS policies.
#[derive(Debug, Clone, Copy, Default)]
pub struct Scope {
    pub user_id: Option<Uuid>,
    pub tenant_id: Option<Uuid>,
}

impl Scope {
    /// No identity: tenant-scoped tables return nothing.
    pub const NONE: Scope = Scope { user_id: None, tenant_id: None };

    /// Signed-in user, no practice selected. Can read own memberships only.
    pub fn user(user_id: Uuid) -> Self {
        Self { user_id: Some(user_id), tenant_id: None }
    }

    /// Signed-in user acting inside a practice.
    pub fn tenant(user_id: Uuid, tenant_id: Uuid) -> Self {
        Self { user_id: Some(user_id), tenant_id: Some(tenant_id) }
    }
}

/// Begin a transaction with RLS context applied.
pub async fn begin(pool: &PgPool, scope: Scope) -> Result<Transaction<'static, Postgres>, sqlx::Error> {
    let mut tx = pool.begin().await?;
    set_scope(&mut tx, scope).await?;
    Ok(tx)
}

/// Change the RLS context mid-transaction (e.g. after verifying membership
/// during practice selection).
pub async fn set_scope(conn: &mut PgConnection, scope: Scope) -> Result<(), sqlx::Error> {
    // Empty string = unset; app.current_*_id() turn '' into NULL.
    let as_setting = |id: Option<Uuid>| id.map(|v| v.to_string()).unwrap_or_default();
    sqlx::query("SELECT set_config('app.user_id', $1, true), set_config('app.tenant_id', $2, true)")
        .bind(as_setting(scope.user_id))
        .bind(as_setting(scope.tenant_id))
        .execute(conn)
        .await?;
    Ok(())
}
