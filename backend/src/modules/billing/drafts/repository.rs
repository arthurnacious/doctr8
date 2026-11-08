use chrono::{DateTime, Utc};
use sqlx::PgConnection;
use uuid::Uuid;

#[derive(sqlx::FromRow)]
pub struct DraftRow {
    pub id: Uuid,
    pub account_id: Uuid,
    /// jsonb read as text (parsed by the service).
    pub content: String,
    pub created_by: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

const COLUMNS: &str = "id, account_id, content::text AS content, created_by, created_at, updated_at";

pub async fn insert(conn: &mut PgConnection, tenant_id: Uuid, account_id: Uuid, content: &str, by: Uuid) -> Result<DraftRow, sqlx::Error> {
    sqlx::query_as(&format!(
        "INSERT INTO invoice_drafts (tenant_id, account_id, content, created_by) VALUES ($1, $2, $3::jsonb, $4) RETURNING {COLUMNS}"
    ))
    .bind(tenant_id)
    .bind(account_id)
    .bind(content)
    .bind(by)
    .fetch_one(conn)
    .await
}

pub async fn find(conn: &mut PgConnection, tenant_id: Uuid, id: Uuid, lock: bool) -> Result<Option<DraftRow>, sqlx::Error> {
    let suffix = if lock { " FOR UPDATE" } else { "" };
    sqlx::query_as(&format!("SELECT {COLUMNS} FROM invoice_drafts WHERE tenant_id = $1 AND id = $2{suffix}"))
        .bind(tenant_id)
        .bind(id)
        .fetch_optional(conn)
        .await
}

pub async fn for_account(conn: &mut PgConnection, tenant_id: Uuid, account_id: Uuid) -> Result<Vec<DraftRow>, sqlx::Error> {
    sqlx::query_as(&format!(
        "SELECT {COLUMNS} FROM invoice_drafts WHERE tenant_id = $1 AND account_id = $2 ORDER BY updated_at DESC"
    ))
    .bind(tenant_id)
    .bind(account_id)
    .fetch_all(conn)
    .await
}

pub async fn update(conn: &mut PgConnection, tenant_id: Uuid, id: Uuid, content: &str) -> Result<Option<DraftRow>, sqlx::Error> {
    sqlx::query_as(&format!(
        "UPDATE invoice_drafts SET content = $3::jsonb WHERE tenant_id = $1 AND id = $2 RETURNING {COLUMNS}"
    ))
    .bind(tenant_id)
    .bind(id)
    .bind(content)
    .fetch_optional(conn)
    .await
}

pub async fn delete(conn: &mut PgConnection, tenant_id: Uuid, id: Uuid) -> Result<bool, sqlx::Error> {
    Ok(sqlx::query("DELETE FROM invoice_drafts WHERE tenant_id = $1 AND id = $2")
        .bind(tenant_id)
        .bind(id)
        .execute(conn)
        .await?
        .rows_affected()
        > 0)
}
