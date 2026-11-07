use chrono::{DateTime, Utc};
use sqlx::PgConnection;
use uuid::Uuid;

use super::dto::{NewUser, UserResponse};

// email is citext; cast to text for decoding and cast params to citext so
// comparisons stay case-insensitive.
const USER_COLUMNS: &str = "id, email::text AS email, full_name, email_verified_at, created_at";

pub async fn insert_user(
    conn: &mut PgConnection,
    new: &NewUser,
    password_hash: &str,
) -> Result<UserResponse, sqlx::Error> {
    sqlx::query_as(&format!(
        "INSERT INTO users (email, full_name, password_hash)
         VALUES ($1::citext, $2, $3)
         RETURNING {USER_COLUMNS}"
    ))
    .bind(&new.email)
    .bind(&new.full_name)
    .bind(password_hash)
    .fetch_one(conn)
    .await
}

pub async fn find_by_id(conn: &mut PgConnection, id: Uuid) -> Result<Option<UserResponse>, sqlx::Error> {
    sqlx::query_as(&format!("SELECT {USER_COLUMNS} FROM users WHERE id = $1"))
        .bind(id)
        .fetch_optional(conn)
        .await
}

// --- memberships ------------------------------------------------------------

pub async fn insert_membership(
    conn: &mut PgConnection,
    tenant_id: Uuid,
    user_id: Uuid,
    status: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO memberships (tenant_id, user_id, status, joined_at)
         VALUES ($1, $2, $3, CASE WHEN $3 = 'active' THEN now() END)",
    )
    .bind(tenant_id)
    .bind(user_id)
    .bind(status)
    .execute(conn)
    .await?;
    Ok(())
}

pub async fn membership_status(
    conn: &mut PgConnection,
    tenant_id: Uuid,
    user_id: Uuid,
) -> Result<Option<String>, sqlx::Error> {
    sqlx::query_scalar("SELECT status FROM memberships WHERE tenant_id = $1 AND user_id = $2")
        .bind(tenant_id)
        .bind(user_id)
        .fetch_optional(conn)
        .await
}

#[derive(sqlx::FromRow)]
pub struct MemberRow {
    pub user_id: Uuid,
    pub email: String,
    pub full_name: String,
    pub status: String,
    pub joined_at: Option<DateTime<Utc>>,
}

pub async fn list_members(conn: &mut PgConnection, tenant_id: Uuid) -> Result<Vec<MemberRow>, sqlx::Error> {
    sqlx::query_as(
        "SELECT m.user_id, u.email::text AS email, u.full_name, m.status, m.joined_at
           FROM memberships m
           JOIN users u ON u.id = m.user_id
          WHERE m.tenant_id = $1 AND m.status <> 'removed'
          ORDER BY lower(u.full_name)",
    )
    .bind(tenant_id)
    .fetch_all(conn)
    .await
}

// --- membership_roles ------------------------------------------------------

#[derive(sqlx::FromRow)]
pub struct MemberRoleRow {
    pub user_id: Uuid,
    pub id: Uuid,
    pub name: String,
}

/// Role assignments for the whole practice, or one member when `user_id` is set.
pub async fn list_member_roles(
    conn: &mut PgConnection,
    tenant_id: Uuid,
    user_id: Option<Uuid>,
) -> Result<Vec<MemberRoleRow>, sqlx::Error> {
    sqlx::query_as(
        "SELECT mr.user_id, r.id, r.name
           FROM membership_roles mr
           JOIN roles r ON r.tenant_id = mr.tenant_id AND r.id = mr.role_id
          WHERE mr.tenant_id = $1 AND ($2::uuid IS NULL OR mr.user_id = $2)
          ORDER BY r.is_system DESC, lower(r.name)",
    )
    .bind(tenant_id)
    .bind(user_id)
    .fetch_all(conn)
    .await
}

pub async fn add_member_role(
    conn: &mut PgConnection,
    tenant_id: Uuid,
    user_id: Uuid,
    role_id: Uuid,
    assigned_by: Uuid,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO membership_roles (tenant_id, user_id, role_id, assigned_by)
         VALUES ($1, $2, $3, $4)
         ON CONFLICT DO NOTHING",
    )
    .bind(tenant_id)
    .bind(user_id)
    .bind(role_id)
    .bind(assigned_by)
    .execute(conn)
    .await?;
    Ok(())
}

pub async fn remove_member_roles(
    conn: &mut PgConnection,
    tenant_id: Uuid,
    user_id: Uuid,
    role_ids: &[Uuid],
) -> Result<(), sqlx::Error> {
    sqlx::query("DELETE FROM membership_roles WHERE tenant_id = $1 AND user_id = $2 AND role_id = ANY($3)")
        .bind(tenant_id)
        .bind(user_id)
        .bind(role_ids)
        .execute(conn)
        .await?;
    Ok(())
}

/// Active members other than `excluding` who hold `role_id`.
pub async fn count_active_holders_excluding(
    conn: &mut PgConnection,
    tenant_id: Uuid,
    role_id: Uuid,
    excluding: Uuid,
) -> Result<i64, sqlx::Error> {
    sqlx::query_scalar(
        "SELECT count(*)
           FROM membership_roles mr
           JOIN memberships m ON m.tenant_id = mr.tenant_id AND m.user_id = mr.user_id
          WHERE mr.tenant_id = $1 AND mr.role_id = $2 AND mr.user_id <> $3 AND m.status = 'active'",
    )
    .bind(tenant_id)
    .bind(role_id)
    .bind(excluding)
    .fetch_one(conn)
    .await
}
