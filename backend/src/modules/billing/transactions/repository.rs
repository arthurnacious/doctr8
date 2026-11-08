use chrono::{DateTime, NaiveDate, Utc};
use sqlx::PgConnection;
use uuid::Uuid;

use super::dto::{Entry, NewTransaction, ReasonTypeResponse, RemittanceAllocation, RemittanceSummary, ValidRemittance};

pub async fn insert_transaction(conn: &mut PgConnection, tenant_id: Uuid, t: &NewTransaction, by: Uuid) -> Result<Uuid, sqlx::Error> {
    sqlx::query_scalar(
        "INSERT INTO transactions
                (tenant_id, account_id, kind, txn_date, amount_cents, payment_method, reference, notes,
                 invoice_id, claim_id, credit_note_id, remittance_id, reason_type_id, reverses_transaction_id, created_by,
                 receipt_no)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16)
         RETURNING id",
    )
    .bind(tenant_id)
    .bind(t.account_id)
    .bind(t.kind.as_str())
    .bind(t.txn_date)
    .bind(t.amount_cents)
    .bind(t.payment_method.map(|m| m.as_str()))
    .bind(&t.reference)
    .bind(&t.notes)
    .bind(t.invoice_id)
    .bind(t.claim_id)
    .bind(t.credit_note_id)
    .bind(t.remittance_id)
    .bind(t.reason_type_id)
    .bind(t.reverses_transaction_id)
    .bind(by)
    .bind(t.receipt_no)
    .fetch_one(conn)
    .await
}

pub async fn insert_entries(conn: &mut PgConnection, tenant_id: Uuid, txn_id: Uuid, account_id: Uuid, entries: &[Entry]) -> Result<(), sqlx::Error> {
    if entries.is_empty() {
        return Ok(());
    }
    let invoices: Vec<Option<Uuid>> = entries.iter().map(|e| e.invoice_id).collect();
    let buckets: Vec<&str> = entries.iter().map(|e| e.bucket.as_str()).collect();
    let amounts: Vec<i64> = entries.iter().map(|e| e.amount_cents).collect();
    sqlx::query(
        "INSERT INTO transaction_entries (tenant_id, transaction_id, account_id, invoice_id, bucket, amount_cents)
         SELECT $1, $2, $3, i, b, a FROM unnest($4::uuid[], $5::text[], $6::bigint[]) AS u(i, b, a)",
    )
    .bind(tenant_id)
    .bind(txn_id)
    .bind(account_id)
    .bind(&invoices)
    .bind(&buckets)
    .bind(&amounts)
    .execute(conn)
    .await?;
    Ok(())
}

/// What the account needs for money rules.
#[derive(sqlx::FromRow)]
pub struct AccountBilling {
    pub status: String,
    pub membership_id: Option<Uuid>,
    pub scheme_id: Option<Uuid>,
}

pub async fn account_billing(conn: &mut PgConnection, tenant_id: Uuid, account_id: Uuid) -> Result<Option<AccountBilling>, sqlx::Error> {
    sqlx::query_as(
        "SELECT a.status, a.membership_id, m.scheme_id
           FROM accounts a
           LEFT JOIN medical_aid_memberships m ON m.tenant_id = a.tenant_id AND m.id = a.membership_id
          WHERE a.tenant_id = $1 AND a.id = $2",
    )
    .bind(tenant_id)
    .bind(account_id)
    .fetch_optional(conn)
    .await
}

/// Per-invoice amounts owed on an account, oldest first.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct InvoiceDue {
    pub invoice_id: Uuid,
    #[allow(dead_code)]
    pub invoice_no: i64,
    #[allow(dead_code)]
    pub invoice_date: NaiveDate,
    pub medical_aid_cents: i64,
    pub patient_cents: i64,
}

const DUE_SELECT: &str = "
    SELECT i.id AS invoice_id, i.invoice_no, i.invoice_date,
           coalesce(sum(e.amount_cents) FILTER (WHERE e.bucket = 'medical_aid'), 0)::bigint AS medical_aid_cents,
           coalesce(sum(e.amount_cents) FILTER (WHERE e.bucket = 'patient'), 0)::bigint AS patient_cents
      FROM invoices i
      LEFT JOIN transaction_entries e ON e.tenant_id = i.tenant_id AND e.invoice_id = i.id";

pub async fn invoice_dues(conn: &mut PgConnection, tenant_id: Uuid, account_id: Uuid) -> Result<Vec<InvoiceDue>, sqlx::Error> {
    sqlx::query_as(&format!(
        "{DUE_SELECT} WHERE i.tenant_id = $1 AND i.account_id = $2
          GROUP BY i.id ORDER BY i.invoice_date, i.invoice_no"
    ))
    .bind(tenant_id)
    .bind(account_id)
    .fetch_all(conn)
    .await
}

pub async fn invoice_due(conn: &mut PgConnection, tenant_id: Uuid, invoice_id: Uuid) -> Result<Option<InvoiceDue>, sqlx::Error> {
    sqlx::query_as(&format!("{DUE_SELECT} WHERE i.tenant_id = $1 AND i.id = $2 GROUP BY i.id"))
        .bind(tenant_id)
        .bind(invoice_id)
        .fetch_optional(conn)
        .await
}

/// (medical aid, patient on invoices, unallocated patient) for the account.
pub async fn account_totals(conn: &mut PgConnection, tenant_id: Uuid, account_id: Uuid) -> Result<(i64, i64, i64), sqlx::Error> {
    sqlx::query_as(
        "SELECT coalesce(sum(amount_cents) FILTER (WHERE bucket = 'medical_aid'), 0)::bigint,
                coalesce(sum(amount_cents) FILTER (WHERE bucket = 'patient' AND invoice_id IS NOT NULL), 0)::bigint,
                coalesce(sum(amount_cents) FILTER (WHERE invoice_id IS NULL), 0)::bigint
           FROM transaction_entries WHERE tenant_id = $1 AND account_id = $2",
    )
    .bind(tenant_id)
    .bind(account_id)
    .fetch_one(conn)
    .await
}

#[derive(sqlx::FromRow)]
pub struct TxnRow {
    pub id: Uuid,
    pub account_id: Uuid,
    pub kind: String,
    pub txn_date: NaiveDate,
    pub posted_at: DateTime<Utc>,
    pub amount_cents: i64,
    pub payment_method: Option<String>,
    pub reference: Option<String>,
    pub notes: Option<String>,
    pub invoice_id: Option<Uuid>,
    pub claim_id: Option<Uuid>,
    pub credit_note_id: Option<Uuid>,
    pub remittance_id: Option<Uuid>,
    pub reason_type_id: Option<Uuid>,
    pub reason_code: Option<String>,
    pub reason_name: Option<String>,
    pub reverses_transaction_id: Option<Uuid>,
    pub reversed_by: Option<Uuid>,
    pub receipt_no: Option<i64>,
}

const TXN_SELECT: &str = "
    SELECT t.id, t.account_id, t.kind, t.txn_date, t.posted_at, t.amount_cents, t.payment_method, t.reference,
           t.notes, t.invoice_id, t.claim_id, t.credit_note_id, t.remittance_id, t.reason_type_id,
           r.code AS reason_code, r.name AS reason_name, t.reverses_transaction_id,
           (SELECT x.id FROM transactions x
             WHERE x.tenant_id = t.tenant_id AND x.reverses_transaction_id = t.id) AS reversed_by,
           t.receipt_no
      FROM transactions t
      LEFT JOIN journal_reason_types r ON r.tenant_id = t.tenant_id AND r.id = t.reason_type_id";

pub async fn find(conn: &mut PgConnection, tenant_id: Uuid, id: Uuid) -> Result<Option<TxnRow>, sqlx::Error> {
    sqlx::query_as(&format!("{TXN_SELECT} WHERE t.tenant_id = $1 AND t.id = $2"))
        .bind(tenant_id)
        .bind(id)
        .fetch_optional(conn)
        .await
}

pub async fn list_for_account(conn: &mut PgConnection, tenant_id: Uuid, account_id: Uuid) -> Result<Vec<TxnRow>, sqlx::Error> {
    sqlx::query_as(&format!(
        "{TXN_SELECT} WHERE t.tenant_id = $1 AND t.account_id = $2 ORDER BY t.txn_date, t.posted_at, t.id"
    ))
    .bind(tenant_id)
    .bind(account_id)
    .fetch_all(conn)
    .await
}

#[derive(sqlx::FromRow)]
pub struct EntryRow {
    pub transaction_id: Uuid,
    pub invoice_id: Option<Uuid>,
    pub bucket: String,
    pub amount_cents: i64,
}

pub async fn entries_for(conn: &mut PgConnection, tenant_id: Uuid, txn_ids: &[Uuid]) -> Result<Vec<EntryRow>, sqlx::Error> {
    sqlx::query_as(
        "SELECT transaction_id, invoice_id, bucket, amount_cents FROM transaction_entries
          WHERE tenant_id = $1 AND transaction_id = ANY($2)
          ORDER BY transaction_id, bucket, invoice_id",
    )
    .bind(tenant_id)
    .bind(txn_ids)
    .fetch_all(conn)
    .await
}

// --- journal reason types --------------------------------------------------------

pub async fn list_reasons(conn: &mut PgConnection, tenant_id: Uuid, include_inactive: bool) -> Result<Vec<ReasonTypeResponse>, sqlx::Error> {
    sqlx::query_as(
        "SELECT id, code, name, is_active FROM journal_reason_types
          WHERE tenant_id = $1 AND ($2 OR is_active) ORDER BY lower(name)",
    )
    .bind(tenant_id)
    .bind(include_inactive)
    .fetch_all(conn)
    .await
}

pub async fn find_reason(conn: &mut PgConnection, tenant_id: Uuid, id: Uuid) -> Result<Option<ReasonTypeResponse>, sqlx::Error> {
    sqlx::query_as("SELECT id, code, name, is_active FROM journal_reason_types WHERE tenant_id = $1 AND id = $2")
        .bind(tenant_id)
        .bind(id)
        .fetch_optional(conn)
        .await
}

pub async fn insert_reason(conn: &mut PgConnection, tenant_id: Uuid, code: &str, name: &str) -> Result<ReasonTypeResponse, sqlx::Error> {
    sqlx::query_as(
        "INSERT INTO journal_reason_types (tenant_id, code, name) VALUES ($1, $2, $3)
         RETURNING id, code, name, is_active",
    )
    .bind(tenant_id)
    .bind(code)
    .bind(name)
    .fetch_one(conn)
    .await
}

pub async fn update_reason(conn: &mut PgConnection, tenant_id: Uuid, id: Uuid, name: Option<&str>, is_active: Option<bool>) -> Result<Option<ReasonTypeResponse>, sqlx::Error> {
    sqlx::query_as(
        "UPDATE journal_reason_types SET name = COALESCE($3, name), is_active = COALESCE($4, is_active)
          WHERE tenant_id = $1 AND id = $2
      RETURNING id, code, name, is_active",
    )
    .bind(tenant_id)
    .bind(id)
    .bind(name)
    .bind(is_active)
    .fetch_optional(conn)
    .await
}

// --- remittances -----------------------------------------------------------------

const REMITTANCE_SELECT: &str = "
    SELECT r.id, r.scheme_id, s.name AS scheme_name, r.reference, r.received_date, r.total_cents,
           a.allocated AS allocated_cents, r.total_cents - a.allocated AS unallocated_cents,
           r.notes, r.created_at
      FROM remittances r
      JOIN medical_schemes s ON s.id = r.scheme_id
      CROSS JOIN LATERAL (
        SELECT coalesce(-sum(e.amount_cents), 0)::bigint AS allocated
          FROM transactions t
          JOIN transaction_entries e ON e.tenant_id = t.tenant_id AND e.transaction_id = t.id
         WHERE t.tenant_id = r.tenant_id AND t.remittance_id = r.id) a";

pub async fn insert_remittance(conn: &mut PgConnection, tenant_id: Uuid, r: &ValidRemittance, by: Uuid) -> Result<Uuid, sqlx::Error> {
    sqlx::query_scalar(
        "INSERT INTO remittances (tenant_id, scheme_id, reference, received_date, total_cents, notes, created_by)
         VALUES ($1, $2, $3, $4, $5, $6, $7) RETURNING id",
    )
    .bind(tenant_id)
    .bind(r.scheme_id)
    .bind(&r.reference)
    .bind(r.received_date)
    .bind(r.total_cents)
    .bind(&r.notes)
    .bind(by)
    .fetch_one(conn)
    .await
}

pub async fn find_remittance(conn: &mut PgConnection, tenant_id: Uuid, id: Uuid) -> Result<Option<RemittanceSummary>, sqlx::Error> {
    sqlx::query_as(&format!("{REMITTANCE_SELECT} WHERE r.tenant_id = $1 AND r.id = $2"))
        .bind(tenant_id)
        .bind(id)
        .fetch_optional(conn)
        .await
}

pub async fn list_remittances(conn: &mut PgConnection, tenant_id: Uuid, unallocated_only: bool, limit: i64) -> Result<Vec<RemittanceSummary>, sqlx::Error> {
    sqlx::query_as(&format!(
        "SELECT * FROM ({REMITTANCE_SELECT} WHERE r.tenant_id = $1) x
          WHERE NOT $2 OR unallocated_cents > 0
          ORDER BY received_date DESC, created_at DESC LIMIT $3"
    ))
    .bind(tenant_id)
    .bind(unallocated_only)
    .bind(limit)
    .fetch_all(conn)
    .await
}

pub async fn remittance_allocations(conn: &mut PgConnection, tenant_id: Uuid, id: Uuid) -> Result<Vec<RemittanceAllocation>, sqlx::Error> {
    sqlx::query_as(
        "SELECT t.id AS transaction_id, t.kind, t.account_id, a.account_no, t.txn_date,
                coalesce(-sum(e.amount_cents), 0)::bigint AS amount_cents
           FROM transactions t
           JOIN accounts a ON a.tenant_id = t.tenant_id AND a.id = t.account_id
           JOIN transaction_entries e ON e.tenant_id = t.tenant_id AND e.transaction_id = t.id
          WHERE t.tenant_id = $1 AND t.remittance_id = $2
          GROUP BY t.id, a.account_no
          ORDER BY t.posted_at",
    )
    .bind(tenant_id)
    .bind(id)
    .fetch_all(conn)
    .await
}

pub async fn scheme_exists(conn: &mut PgConnection, scheme_id: Uuid) -> Result<bool, sqlx::Error> {
    sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM medical_schemes WHERE id = $1 AND is_active)")
        .bind(scheme_id)
        .fetch_one(conn)
        .await
}
