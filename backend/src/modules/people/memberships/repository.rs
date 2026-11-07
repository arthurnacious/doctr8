use sqlx::PgConnection;
use uuid::Uuid;

use super::dto::{MembershipChanges, MembershipResponse, NewMembership};

#[derive(sqlx::FromRow)]
pub struct SchemeRow {
    pub id: Uuid,
    pub name: String,
    pub code: Option<String>,
}

#[derive(sqlx::FromRow)]
pub struct PlanRow {
    pub id: Uuid,
    pub scheme_id: Uuid,
    pub name: String,
    pub code: Option<String>,
}

pub async fn active_schemes(conn: &mut PgConnection) -> Result<Vec<SchemeRow>, sqlx::Error> {
    sqlx::query_as("SELECT id, name, code FROM medical_schemes WHERE is_active ORDER BY lower(name)")
        .fetch_all(conn)
        .await
}

pub async fn active_plans(conn: &mut PgConnection) -> Result<Vec<PlanRow>, sqlx::Error> {
    sqlx::query_as(
        "SELECT id, scheme_id, name, code FROM medical_scheme_plans WHERE is_active ORDER BY lower(name)",
    )
    .fetch_all(conn)
    .await
}

/// True if the plan is active and belongs to the (active) scheme.
pub async fn plan_is_offered(conn: &mut PgConnection, scheme_id: Uuid, plan_id: Uuid) -> Result<bool, sqlx::Error> {
    sqlx::query_scalar(
        "SELECT EXISTS (
            SELECT 1 FROM medical_scheme_plans p
              JOIN medical_schemes s ON s.id = p.scheme_id
             WHERE p.id = $2 AND p.scheme_id = $1 AND p.is_active AND s.is_active)",
    )
    .bind(scheme_id)
    .bind(plan_id)
    .fetch_one(conn)
    .await
}

pub async fn insert(
    conn: &mut PgConnection,
    tenant_id: Uuid,
    m: &NewMembership,
    created_by: Uuid,
) -> Result<Uuid, sqlx::Error> {
    sqlx::query_scalar(
        "INSERT INTO medical_aid_memberships
                (tenant_id, scheme_id, plan_id, membership_number, main_member_person_id, created_by)
         VALUES ($1, $2, $3, $4, $5, $6)
         RETURNING id",
    )
    .bind(tenant_id)
    .bind(m.scheme_id)
    .bind(m.plan_id)
    .bind(&m.membership_number)
    .bind(m.main_member_person_id)
    .bind(created_by)
    .fetch_one(conn)
    .await
}

pub async fn find(conn: &mut PgConnection, tenant_id: Uuid, id: Uuid) -> Result<Option<MembershipResponse>, sqlx::Error> {
    sqlx::query_as(
        "SELECT m.id, m.scheme_id, s.name AS scheme_name, m.plan_id, p.name AS plan_name,
                m.membership_number, m.main_member_person_id, m.created_at, m.updated_at
           FROM medical_aid_memberships m
           JOIN medical_schemes s       ON s.id = m.scheme_id
           JOIN medical_scheme_plans p  ON p.id = m.plan_id
          WHERE m.tenant_id = $1 AND m.id = $2",
    )
    .bind(tenant_id)
    .bind(id)
    .fetch_optional(conn)
    .await
}

pub async fn update(
    conn: &mut PgConnection,
    tenant_id: Uuid,
    id: Uuid,
    c: &MembershipChanges,
) -> Result<bool, sqlx::Error> {
    let result = sqlx::query(
        "UPDATE medical_aid_memberships
            SET plan_id               = COALESCE($3, plan_id),
                membership_number     = COALESCE($4, membership_number),
                main_member_person_id = COALESCE($5, main_member_person_id)
          WHERE tenant_id = $1 AND id = $2",
    )
    .bind(tenant_id)
    .bind(id)
    .bind(c.plan_id)
    .bind(&c.membership_number)
    .bind(c.main_member_person_id)
    .execute(conn)
    .await?;
    Ok(result.rows_affected() == 1)
}
