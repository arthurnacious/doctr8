use sqlx::PgConnection;
use uuid::Uuid;

use chrono::NaiveDate;

use super::dto::{PatientResponse, PatientSearch};

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
        "SELECT pt.id, pt.person_id, pt.account_id, a.account_no, pt.dependant_code, pt.status, pt.created_at, pt.updated_at
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

#[derive(sqlx::FromRow)]
pub struct PatientListRow {
    pub patient_id: Uuid,
    pub person_id: Uuid,
    pub account_id: Uuid,
    pub account_no: i64,
    pub dependant_code: String,
    pub status: String,
    pub title: Option<String>,
    pub first_name: String,
    pub last_name: String,
    pub date_of_birth: Option<NaiveDate>,
    pub id_type: Option<String>,
    pub id_number: Option<String>,
    pub scheme_name: Option<String>,
    pub plan_name: Option<String>,
    pub membership_number: Option<String>,
    pub main_member_person_id: Option<Uuid>,
    pub main_member_first_name: Option<String>,
    pub main_member_last_name: Option<String>,
}

const LIST_FROM: &str = "
      FROM patients pt
      JOIN persons p  ON p.tenant_id = pt.tenant_id AND p.id = pt.person_id
      JOIN accounts a ON a.tenant_id = pt.tenant_id AND a.id = pt.account_id
      LEFT JOIN medical_aid_memberships m ON m.tenant_id = a.tenant_id AND m.id = a.membership_id
      LEFT JOIN medical_schemes s ON s.id = m.scheme_id
      LEFT JOIN medical_scheme_plans pl ON pl.id = m.plan_id
      LEFT JOIN persons mm ON mm.tenant_id = m.tenant_id AND mm.id = m.main_member_person_id
     WHERE pt.tenant_id = $1
       AND ($2::text IS NULL OR pt.status = $2)
       AND ($3
         OR ($4::text   IS NOT NULL AND p.id_number = $4)
         OR ($5::bigint IS NOT NULL AND a.account_no = $5)
         OR ($6::text   IS NOT NULL AND m.membership_number = $6)
         OR ($7::text IS NOT NULL AND $8::text IS NULL AND (
               lower(p.last_name) LIKE $7 || '%'
            OR lower(p.first_name) LIKE $7 || '%'
            OR lower(coalesce(p.preferred_name, '')) LIKE $7 || '%'))
         OR ($7::text IS NOT NULL AND $8::text IS NOT NULL AND (
               (lower(p.last_name) LIKE $7 || '%'
                AND (lower(p.first_name) LIKE $8 || '%' OR lower(coalesce(p.preferred_name, '')) LIKE $8 || '%'))
            OR (lower(p.first_name) LIKE $7 || '%' AND lower(p.last_name) LIKE $8 || '%'))))";

macro_rules! bind_search {
    ($q:expr, $tenant:expr, $s:expr) => {{
        let t = $s.terms.as_ref();
        $q.bind($tenant)
            .bind($s.status.as_db())
            .bind(t.is_none())
            .bind(t.and_then(|t| t.id_number.clone()))
            .bind(t.and_then(|t| t.account_no))
            .bind(t.and_then(|t| t.membership_number.clone()))
            .bind(t.and_then(|t| t.name_a.clone()))
            .bind(t.and_then(|t| t.name_b.clone()))
    }};
}

pub async fn search_count(conn: &mut PgConnection, tenant_id: Uuid, s: &PatientSearch) -> Result<i64, sqlx::Error> {
    let sql = format!("SELECT count(*) {LIST_FROM}");
    bind_search!(sqlx::query_scalar(&sql), tenant_id, s).fetch_one(conn).await
}

pub async fn search_page(conn: &mut PgConnection, tenant_id: Uuid, s: &PatientSearch) -> Result<Vec<PatientListRow>, sqlx::Error> {
    let sql = format!(
        "SELECT pt.id AS patient_id, pt.person_id, pt.account_id, a.account_no, pt.dependant_code, pt.status,
                p.title, p.first_name, p.last_name, p.date_of_birth, p.id_type, p.id_number,
                s.name AS scheme_name, pl.name AS plan_name, m.membership_number,
                mm.id AS main_member_person_id, mm.first_name AS main_member_first_name,
                mm.last_name AS main_member_last_name
         {LIST_FROM}
         ORDER BY lower(p.last_name), lower(p.first_name), a.account_no, pt.dependant_code
         LIMIT $9 OFFSET $10"
    );
    bind_search!(sqlx::query_as(&sql), tenant_id, s)
        .bind(s.page_size)
        .bind(s.offset())
        .fetch_all(conn)
        .await
}

pub async fn set_status(conn: &mut PgConnection, tenant_id: Uuid, id: Uuid, status: &str) -> Result<bool, sqlx::Error> {
    let r = sqlx::query(
        "UPDATE patients
            SET status = $3,
                inactive_at = CASE WHEN $3 = 'inactive' THEN COALESCE(inactive_at, now()) ELSE NULL END
          WHERE tenant_id = $1 AND id = $2",
    )
    .bind(tenant_id)
    .bind(id)
    .bind(status)
    .execute(conn)
    .await?;
    Ok(r.rows_affected() == 1)
}
