use chrono::{DateTime, Utc};
use sqlx::PgConnection;
use uuid::Uuid;

use super::dto::AccountPatient;

#[derive(sqlx::FromRow)]
pub struct AccountRow {
    pub id: Uuid,
    pub account_no: i64,
    pub file_mode: String,
    pub status: String,
    pub guarantor_person_id: Uuid,
    pub guarantor_first_name: String,
    pub guarantor_last_name: String,
    pub membership_id: Option<Uuid>,
    pub scheme_name: Option<String>,
    pub plan_name: Option<String>,
    pub membership_number: Option<String>,
    pub main_member_person_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl AccountRow {
    /// Medical aid main member, or the guarantor on a private account.
    pub fn principal(&self) -> Uuid {
        self.main_member_person_id.unwrap_or(self.guarantor_person_id)
    }
}

pub async fn find(conn: &mut PgConnection, tenant_id: Uuid, id: Uuid) -> Result<Option<AccountRow>, sqlx::Error> {
    sqlx::query_as(
        "SELECT a.id, a.account_no, a.file_mode, a.status, a.guarantor_person_id,
                g.first_name AS guarantor_first_name, g.last_name AS guarantor_last_name,
                a.membership_id, s.name AS scheme_name, pl.name AS plan_name,
                m.membership_number, m.main_member_person_id,
                a.created_at, a.updated_at
           FROM accounts a
           JOIN persons g ON g.tenant_id = a.tenant_id AND g.id = a.guarantor_person_id
           LEFT JOIN medical_aid_memberships m ON m.tenant_id = a.tenant_id AND m.id = a.membership_id
           LEFT JOIN medical_schemes s ON s.id = m.scheme_id
           LEFT JOIN medical_scheme_plans pl ON pl.id = m.plan_id
          WHERE a.tenant_id = $1 AND a.id = $2",
    )
    .bind(tenant_id)
    .bind(id)
    .fetch_optional(conn)
    .await
}

/// Row-lock the account so concurrent changes to its patients serialise.
pub async fn lock(conn: &mut PgConnection, tenant_id: Uuid, id: Uuid) -> Result<bool, sqlx::Error> {
    Ok(sqlx::query("SELECT 1 FROM accounts WHERE tenant_id = $1 AND id = $2 FOR UPDATE")
        .bind(tenant_id)
        .bind(id)
        .fetch_optional(conn)
        .await?
        .is_some())
}

pub async fn patients(conn: &mut PgConnection, tenant_id: Uuid, account_id: Uuid) -> Result<Vec<AccountPatient>, sqlx::Error> {
    sqlx::query_as(
        "SELECT pt.id AS patient_id, pt.person_id, p.first_name, p.last_name, pt.dependant_code, pt.status
           FROM patients pt
           JOIN persons p ON p.tenant_id = pt.tenant_id AND p.id = pt.person_id
          WHERE pt.tenant_id = $1 AND pt.account_id = $2
          ORDER BY pt.dependant_code",
    )
    .bind(tenant_id)
    .bind(account_id)
    .fetch_all(conn)
    .await
}

/// Allocate the next account number for the practice.
pub async fn next_account_no(conn: &mut PgConnection, tenant_id: Uuid) -> Result<i64, sqlx::Error> {
    sqlx::query_scalar(
        "INSERT INTO account_counters (tenant_id, last_account_no) VALUES ($1, 1)
         ON CONFLICT (tenant_id) DO UPDATE SET last_account_no = account_counters.last_account_no + 1
         RETURNING last_account_no",
    )
    .bind(tenant_id)
    .fetch_one(conn)
    .await
}

/// Returns (id, file_mode). The file mode is stamped by a trigger from the
/// practice's current setting — whatever we'd pass would be ignored.
pub async fn insert(
    conn: &mut PgConnection,
    tenant_id: Uuid,
    account_no: i64,
    guarantor_person_id: Uuid,
    membership_id: Option<Uuid>,
    created_by: Uuid,
) -> Result<(Uuid, String), sqlx::Error> {
    sqlx::query_as(
        "INSERT INTO accounts (tenant_id, account_no, file_mode, guarantor_person_id, membership_id, created_by)
         VALUES ($1, $2, 'patient_per_file', $3, $4, $5)
         RETURNING id, file_mode",
    )
    .bind(tenant_id)
    .bind(account_no)
    .bind(guarantor_person_id)
    .bind(membership_id)
    .bind(created_by)
    .fetch_one(conn)
    .await
}

pub async fn update(
    conn: &mut PgConnection,
    tenant_id: Uuid,
    id: Uuid,
    guarantor_person_id: Option<Uuid>,
    membership_id: Option<Option<Uuid>>,
    status: Option<&str>,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE accounts
            SET guarantor_person_id = COALESCE($3, guarantor_person_id),
                membership_id       = CASE WHEN $4 THEN $5 ELSE membership_id END,
                status              = COALESCE($6, status),
                closed_at           = CASE WHEN $6::text IS NULL THEN closed_at
                                           WHEN $6 = 'closed' THEN COALESCE(closed_at, now())
                                           ELSE NULL END
          WHERE tenant_id = $1 AND id = $2",
    )
    .bind(tenant_id)
    .bind(id)
    .bind(guarantor_person_id)
    .bind(membership_id.is_some())
    .bind(membership_id.flatten())
    .bind(status)
    .execute(conn)
    .await?;
    Ok(())
}

/// Accounts a person appears on: as a patient, the guarantor, or the main
/// member of the account's medical aid.
pub async fn involving_person(conn: &mut PgConnection, tenant_id: Uuid, person_id: Uuid) -> Result<Vec<Uuid>, sqlx::Error> {
    sqlx::query_scalar(
        "SELECT DISTINCT a.id
           FROM accounts a
           LEFT JOIN medical_aid_memberships m ON m.tenant_id = a.tenant_id AND m.id = a.membership_id
          WHERE a.tenant_id = $1
            AND (a.guarantor_person_id = $2
              OR m.main_member_person_id = $2
              OR EXISTS (SELECT 1 FROM patients pt
                          WHERE pt.tenant_id = a.tenant_id AND pt.account_id = a.id AND pt.person_id = $2))",
    )
    .bind(tenant_id)
    .bind(person_id)
    .fetch_all(conn)
    .await
}

pub async fn using_membership(conn: &mut PgConnection, tenant_id: Uuid, membership_id: Uuid) -> Result<Vec<Uuid>, sqlx::Error> {
    sqlx::query_scalar("SELECT id FROM accounts WHERE tenant_id = $1 AND membership_id = $2")
        .bind(tenant_id)
        .bind(membership_id)
        .fetch_all(conn)
        .await
}
