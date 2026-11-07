use chrono::NaiveDate;
use sqlx::PgConnection;
use uuid::Uuid;

use super::dto::StatementPatient;

#[derive(sqlx::FromRow)]
pub struct LedgerDbRow {
    pub transaction_id: Uuid,
    pub kind: String,
    pub txn_date: NaiveDate,
    pub reference: Option<String>,
    pub notes: Option<String>,
    pub reason_name: Option<String>,
    pub payment_method: Option<String>,
    pub reverses_transaction_id: Option<Uuid>,
    pub reversed_on: Option<NaiveDate>,
    pub invoice_id: Option<Uuid>,
    pub bucket: Option<String>,
    pub amount_cents: Option<i64>,
    pub age_from: NaiveDate,
}

/// Every ledger entry on the account dated on or before `as_of`, plus
/// claim-submitted records (which have no entries).
pub async fn ledger(conn: &mut PgConnection, tenant_id: Uuid, account_id: Uuid, as_of: NaiveDate) -> Result<Vec<LedgerDbRow>, sqlx::Error> {
    sqlx::query_as(
        "SELECT t.id AS transaction_id, t.kind, t.txn_date, t.reference, t.notes, r.name AS reason_name,
                t.payment_method, t.reverses_transaction_id,
                (SELECT x.txn_date FROM transactions x
                  WHERE x.tenant_id = t.tenant_id AND x.reverses_transaction_id = t.id) AS reversed_on,
                coalesce(e.invoice_id, t.invoice_id) AS invoice_id, e.bucket, e.amount_cents,
                coalesce(i.invoice_date, o.txn_date, t.txn_date) AS age_from
           FROM transactions t
           LEFT JOIN transaction_entries e ON e.tenant_id = t.tenant_id AND e.transaction_id = t.id
           LEFT JOIN invoices i ON i.tenant_id = t.tenant_id AND i.id = coalesce(e.invoice_id, t.invoice_id)
           LEFT JOIN transactions o ON o.tenant_id = t.tenant_id AND o.id = t.reverses_transaction_id
           LEFT JOIN journal_reason_types r ON r.tenant_id = t.tenant_id AND r.id = t.reason_type_id
          WHERE t.tenant_id = $1 AND t.account_id = $2 AND t.txn_date <= $3
            AND (e.id IS NOT NULL OR t.kind = 'claim_submitted')
          ORDER BY t.txn_date, t.posted_at, t.id",
    )
    .bind(tenant_id)
    .bind(account_id)
    .bind(as_of)
    .fetch_all(conn)
    .await
}

pub async fn patients(conn: &mut PgConnection, tenant_id: Uuid, account_id: Uuid) -> Result<Vec<StatementPatient>, sqlx::Error> {
    sqlx::query_as(
        "SELECT pt.id AS patient_id, pt.person_id, pt.dependant_code, p.title, p.first_name, p.last_name, p.date_of_birth
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
