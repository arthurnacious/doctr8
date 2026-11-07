use sqlx::PgConnection;
use uuid::Uuid;

use super::dto::{NewPerson, PersonFile, PersonResponse, PersonSummary, SearchTerms};

const COLUMNS: &str = "id, title, first_name, last_name, preferred_name, date_of_birth, sex,
    id_type, id_number, id_country, email::text AS email, mobile, home_phone,
    address_line1, address_line2, suburb, city, postal_code, created_at, updated_at";

pub async fn insert(
    conn: &mut PgConnection,
    tenant_id: Uuid,
    p: &NewPerson,
    created_by: Uuid,
) -> Result<PersonResponse, sqlx::Error> {
    sqlx::query_as(&format!(
        "INSERT INTO persons (tenant_id, title, first_name, last_name, preferred_name, date_of_birth, sex,
                              id_type, id_number, id_country, email, mobile, home_phone,
                              address_line1, address_line2, suburb, city, postal_code, created_by)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11::citext, $12, $13, $14, $15, $16, $17, $18, $19)
         RETURNING {COLUMNS}"
    ))
    .bind(tenant_id)
    .bind(&p.title)
    .bind(&p.first_name)
    .bind(&p.last_name)
    .bind(&p.preferred_name)
    .bind(p.date_of_birth)
    .bind(p.sex)
    .bind(p.id_type)
    .bind(&p.id_number)
    .bind(&p.id_country)
    .bind(&p.email)
    .bind(&p.mobile)
    .bind(&p.home_phone)
    .bind(&p.address_line1)
    .bind(&p.address_line2)
    .bind(&p.suburb)
    .bind(&p.city)
    .bind(&p.postal_code)
    .bind(created_by)
    .fetch_one(conn)
    .await
}

pub async fn replace(
    conn: &mut PgConnection,
    tenant_id: Uuid,
    id: Uuid,
    p: &NewPerson,
) -> Result<Option<PersonResponse>, sqlx::Error> {
    sqlx::query_as(&format!(
        "UPDATE persons
            SET title = $3, first_name = $4, last_name = $5, preferred_name = $6, date_of_birth = $7,
                sex = $8, id_type = $9, id_number = $10, id_country = $11, email = $12::citext,
                mobile = $13, home_phone = $14, address_line1 = $15, address_line2 = $16,
                suburb = $17, city = $18, postal_code = $19
          WHERE tenant_id = $1 AND id = $2
      RETURNING {COLUMNS}"
    ))
    .bind(tenant_id)
    .bind(id)
    .bind(&p.title)
    .bind(&p.first_name)
    .bind(&p.last_name)
    .bind(&p.preferred_name)
    .bind(p.date_of_birth)
    .bind(p.sex)
    .bind(p.id_type)
    .bind(&p.id_number)
    .bind(&p.id_country)
    .bind(&p.email)
    .bind(&p.mobile)
    .bind(&p.home_phone)
    .bind(&p.address_line1)
    .bind(&p.address_line2)
    .bind(&p.suburb)
    .bind(&p.city)
    .bind(&p.postal_code)
    .fetch_optional(conn)
    .await
}

pub async fn find(conn: &mut PgConnection, tenant_id: Uuid, id: Uuid) -> Result<Option<PersonResponse>, sqlx::Error> {
    sqlx::query_as(&format!("SELECT {COLUMNS} FROM persons WHERE tenant_id = $1 AND id = $2"))
        .bind(tenant_id)
        .bind(id)
        .fetch_optional(conn)
        .await
}

pub async fn exists(conn: &mut PgConnection, tenant_id: Uuid, id: Uuid) -> Result<bool, sqlx::Error> {
    sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM persons WHERE tenant_id = $1 AND id = $2)")
        .bind(tenant_id)
        .bind(id)
        .fetch_one(conn)
        .await
}

pub async fn search(
    conn: &mut PgConnection,
    tenant_id: Uuid,
    t: &SearchTerms,
) -> Result<Vec<PersonSummary>, sqlx::Error> {
    sqlx::query_as(
        r"SELECT p.id, p.title, p.first_name, p.last_name, p.preferred_name, p.date_of_birth
            FROM persons p
           WHERE p.tenant_id = $1
             AND (
                  ($2::text IS NOT NULL AND p.id_number = $2)
               OR ($3::bigint IS NOT NULL AND EXISTS (
                     SELECT 1 FROM patients pt
                       JOIN accounts a ON a.tenant_id = pt.tenant_id AND a.id = pt.account_id
                      WHERE pt.tenant_id = p.tenant_id AND pt.person_id = p.id AND a.account_no = $3))
               OR ($7::text IS NOT NULL AND EXISTS (
                     SELECT 1 FROM patients pt
                       JOIN accounts a ON a.tenant_id = pt.tenant_id AND a.id = pt.account_id
                       JOIN medical_aid_memberships m ON m.tenant_id = a.tenant_id AND m.id = a.membership_id
                      WHERE pt.tenant_id = p.tenant_id AND pt.person_id = p.id AND m.membership_number = $7))
               OR ($4::text IS NOT NULL AND $5::text IS NULL AND (
                     lower(p.last_name) LIKE $4 || '%'
                  OR lower(p.first_name) LIKE $4 || '%'
                  OR lower(coalesce(p.preferred_name, '')) LIKE $4 || '%'))
               OR ($4::text IS NOT NULL AND $5::text IS NOT NULL AND (
                     (lower(p.last_name) LIKE $4 || '%'
                      AND (lower(p.first_name) LIKE $5 || '%' OR lower(coalesce(p.preferred_name, '')) LIKE $5 || '%'))
                  OR (lower(p.first_name) LIKE $4 || '%' AND lower(p.last_name) LIKE $5 || '%')))
             )
           ORDER BY lower(p.last_name), lower(p.first_name), p.id
           LIMIT $6",
    )
    .bind(tenant_id)
    .bind(&t.id_number)
    .bind(t.account_no)
    .bind(&t.name_a)
    .bind(&t.name_b)
    .bind(t.limit)
    .bind(&t.membership_number)
    .fetch_all(conn)
    .await
}

pub async fn files(conn: &mut PgConnection, tenant_id: Uuid, person_id: Uuid) -> Result<Vec<PersonFile>, sqlx::Error> {
    sqlx::query_as(
        "SELECT pt.id AS patient_id, a.id AS account_id, a.account_no, pt.dependant_code,
                a.status AS account_status, a.file_mode,
                s.name AS scheme_name, pl.name AS plan_name, m.membership_number,
                a.created_at AS opened_at
           FROM patients pt
           JOIN accounts a ON a.tenant_id = pt.tenant_id AND a.id = pt.account_id
           LEFT JOIN medical_aid_memberships m ON m.tenant_id = a.tenant_id AND m.id = a.membership_id
           LEFT JOIN medical_schemes s ON s.id = m.scheme_id
           LEFT JOIN medical_scheme_plans pl ON pl.id = m.plan_id
          WHERE pt.tenant_id = $1 AND pt.person_id = $2
          ORDER BY a.created_at DESC",
    )
    .bind(tenant_id)
    .bind(person_id)
    .fetch_all(conn)
    .await
}
