use sqlx::PgConnection;
use uuid::Uuid;

use super::dto::PatientResponse;

pub async fn insert(
    conn: &mut PgConnection,
    tenant_id: Uuid,
    person_id: Uuid,
    account_id: Uuid,
    dependant_code: &str,
    created_by: Uuid,
) -> Result<Uuid, sqlx::Error> {
    sqlx::query_scalar(
        "INSERT INTO patients (tenant_id, person_id, account_id, dependant_code, created_by)
         VALUES ($1, $2, $3, $4, $5)
         RETURNING id",
    )
    .bind(tenant_id)
    .bind(person_id)
    .bind(account_id)
    .bind(dependant_code)
    .bind(created_by)
    .fetch_one(conn)
    .await
}

pub async fn find(conn: &mut PgConnection, tenant_id: Uuid, id: Uuid) -> Result<Option<PatientResponse>, sqlx::Error> {
    sqlx::query_as(
        "SELECT pt.id, pt.person_id, pt.account_id, a.account_no, pt.dependant_code, pt.created_at, pt.updated_at
           FROM patients pt
           JOIN accounts a ON a.tenant_id = pt.tenant_id AND a.id = pt.account_id
          WHERE pt.tenant_id = $1 AND pt.id = $2",
    )
    .bind(tenant_id)
    .bind(id)
    .fetch_optional(conn)
    .await
}

pub async fn update_code(conn: &mut PgConnection, tenant_id: Uuid, id: Uuid, code: &str) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE patients SET dependant_code = $3 WHERE tenant_id = $1 AND id = $2")
        .bind(tenant_id)
        .bind(id)
        .bind(code)
        .execute(conn)
        .await?;
    Ok(())
}
