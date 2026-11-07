use sqlx::PgConnection;
use uuid::Uuid;

use super::dto::{NewTenant, PracticeSummary, TenantChanges, TenantResponse};

const COLUMNS: &str = "id, slug, name, pcns_number, status, file_mode, created_at, updated_at, \
    address_line1, address_line2, suburb, city, postal_code, phone, email, \
    bank_name, bank_account_name, bank_account_number, bank_branch_code";

/// Practices where the user is an active member and the practice is active.
/// Works with only `app.user_id` set (own_memberships_read / member_tenant_read).
pub async fn list_active_for_user(
    conn: &mut PgConnection,
    user_id: Uuid,
) -> Result<Vec<PracticeSummary>, sqlx::Error> {
    sqlx::query_as(
        "SELECT t.id, t.slug, t.name
           FROM memberships m
           JOIN tenants t ON t.id = m.tenant_id
          WHERE m.user_id = $1 AND m.status = 'active' AND t.status = 'active'
          ORDER BY lower(t.name)",
    )
    .bind(user_id)
    .fetch_all(conn)
    .await
}

pub async fn insert(
    conn: &mut PgConnection,
    id: Uuid,
    new: &NewTenant,
) -> Result<TenantResponse, sqlx::Error> {
    sqlx::query_as(&format!(
        "INSERT INTO tenants (id, slug, name, pcns_number) VALUES ($1, $2, $3, $4) RETURNING {COLUMNS}"
    ))
    .bind(id)
    .bind(&new.slug)
    .bind(&new.name)
    .bind(&new.pcns_number)
    .fetch_one(conn)
    .await
}

pub async fn find(conn: &mut PgConnection, id: Uuid) -> Result<Option<TenantResponse>, sqlx::Error> {
    sqlx::query_as(&format!("SELECT {COLUMNS} FROM tenants WHERE id = $1"))
        .bind(id)
        .fetch_optional(conn)
        .await
}

pub async fn update(
    conn: &mut PgConnection,
    id: Uuid,
    changes: &TenantChanges,
) -> Result<Option<TenantResponse>, sqlx::Error> {
    let details = changes.details.clone().unwrap_or_default();
    sqlx::query_as(&format!(
        "UPDATE tenants
            SET name        = COALESCE($2, name),
                pcns_number = CASE WHEN $3 THEN $4 ELSE pcns_number END,
                file_mode   = COALESCE($5, file_mode),
                address_line1       = CASE WHEN $6 THEN $7  ELSE address_line1 END,
                address_line2       = CASE WHEN $6 THEN $8  ELSE address_line2 END,
                suburb              = CASE WHEN $6 THEN $9  ELSE suburb END,
                city                = CASE WHEN $6 THEN $10 ELSE city END,
                postal_code         = CASE WHEN $6 THEN $11 ELSE postal_code END,
                phone               = CASE WHEN $6 THEN $12 ELSE phone END,
                email               = CASE WHEN $6 THEN $13 ELSE email END,
                bank_name           = CASE WHEN $6 THEN $14 ELSE bank_name END,
                bank_account_name   = CASE WHEN $6 THEN $15 ELSE bank_account_name END,
                bank_account_number = CASE WHEN $6 THEN $16 ELSE bank_account_number END,
                bank_branch_code    = CASE WHEN $6 THEN $17 ELSE bank_branch_code END
          WHERE id = $1
      RETURNING {COLUMNS}"
    ))
    .bind(id)
    .bind(&changes.name)
    .bind(changes.pcns_number.is_some())
    .bind(changes.pcns_number.clone().flatten())
    .bind(changes.file_mode.map(|m| m.as_str()))
    .bind(changes.details.is_some())
    .bind(details.address_line1)
    .bind(details.address_line2)
    .bind(details.suburb)
    .bind(details.city)
    .bind(details.postal_code)
    .bind(details.phone)
    .bind(details.email)
    .bind(details.bank_name)
    .bind(details.bank_account_name)
    .bind(details.bank_account_number)
    .bind(details.bank_branch_code)
    .fetch_optional(conn)
    .await
}
