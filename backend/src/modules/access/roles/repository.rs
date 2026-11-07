use chrono::{DateTime, Utc};
use sqlx::PgConnection;
use uuid::Uuid;

#[derive(Debug, sqlx::FromRow)]
pub struct RoleRow {
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub is_system: bool,
    pub member_count: i64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, sqlx::FromRow)]
pub struct RolePermissionRow {
    pub role_id: Uuid,
    pub permission_key: String,
}

const ROLE_SELECT: &str = "
    SELECT r.id, r.name, r.description, r.is_system, r.created_at, r.updated_at,
           (SELECT count(*) FROM membership_roles mr
             WHERE mr.tenant_id = r.tenant_id AND mr.role_id = r.id) AS member_count
      FROM roles r";

// tenant_id is passed explicitly everywhere even though RLS already filters:
// the query stays correct on its own, and RLS is the backstop.

pub async fn list(conn: &mut PgConnection, tenant_id: Uuid) -> Result<Vec<RoleRow>, sqlx::Error> {
    sqlx::query_as(&format!(
        "{ROLE_SELECT} WHERE r.tenant_id = $1 ORDER BY r.is_system DESC, lower(r.name)"
    ))
    .bind(tenant_id)
    .fetch_all(conn)
    .await
}

pub async fn find(conn: &mut PgConnection, tenant_id: Uuid, id: Uuid) -> Result<Option<RoleRow>, sqlx::Error> {
    sqlx::query_as(&format!("{ROLE_SELECT} WHERE r.tenant_id = $1 AND r.id = $2"))
        .bind(tenant_id)
        .bind(id)
        .fetch_optional(conn)
        .await
}

pub async fn insert_role(
    conn: &mut PgConnection,
    tenant_id: Uuid,
    name: &str,
    description: Option<&str>,
    is_system: bool,
) -> Result<Uuid, sqlx::Error> {
    sqlx::query_scalar(
        "INSERT INTO roles (tenant_id, name, description, is_system) VALUES ($1, $2, $3, $4) RETURNING id",
    )
    .bind(tenant_id)
    .bind(name)
    .bind(description)
    .bind(is_system)
    .fetch_one(conn)
    .await
}

pub async fn update_role(
    conn: &mut PgConnection,
    tenant_id: Uuid,
    id: Uuid,
    name: Option<&str>,
    description: Option<Option<&str>>,
) -> Result<bool, sqlx::Error> {
    let result = sqlx::query(
        "UPDATE roles
            SET name        = COALESCE($3, name),
                description = CASE WHEN $4 THEN $5 ELSE description END
          WHERE tenant_id = $1 AND id = $2",
    )
    .bind(tenant_id)
    .bind(id)
    .bind(name)
    .bind(description.is_some())
    .bind(description.flatten())
    .execute(conn)
    .await?;
    Ok(result.rows_affected() == 1)
}

pub async fn delete_role(conn: &mut PgConnection, tenant_id: Uuid, id: Uuid) -> Result<bool, sqlx::Error> {
    let result = sqlx::query("DELETE FROM roles WHERE tenant_id = $1 AND id = $2")
        .bind(tenant_id)
        .bind(id)
        .execute(conn)
        .await?;
    Ok(result.rows_affected() == 1)
}

pub async fn permissions_for_roles(
    conn: &mut PgConnection,
    tenant_id: Uuid,
    role_ids: &[Uuid],
) -> Result<Vec<RolePermissionRow>, sqlx::Error> {
    sqlx::query_as(
        "SELECT role_id, permission_key FROM role_permissions
          WHERE tenant_id = $1 AND role_id = ANY($2)
          ORDER BY permission_key",
    )
    .bind(tenant_id)
    .bind(role_ids)
    .fetch_all(conn)
    .await
}

pub async fn add_permissions(
    conn: &mut PgConnection,
    tenant_id: Uuid,
    role_id: Uuid,
    keys: &[String],
    granted_by: Uuid,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO role_permissions (tenant_id, role_id, permission_key, granted_by)
         SELECT $1, $2, k, $4 FROM unnest($3::text[]) AS k
         ON CONFLICT DO NOTHING",
    )
    .bind(tenant_id)
    .bind(role_id)
    .bind(keys)
    .bind(granted_by)
    .execute(conn)
    .await?;
    Ok(())
}

pub async fn remove_permissions(
    conn: &mut PgConnection,
    tenant_id: Uuid,
    role_id: Uuid,
    keys: &[String],
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "DELETE FROM role_permissions
          WHERE tenant_id = $1 AND role_id = $2 AND permission_key = ANY($3)",
    )
    .bind(tenant_id)
    .bind(role_id)
    .bind(keys)
    .execute(conn)
    .await?;
    Ok(())
}
