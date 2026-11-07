use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::Decimal;
use sqlx::PgConnection;
use uuid::Uuid;

use super::dto::{ClaimResponse, CreditNoteLineResponse, InvoiceSearch, LineResponse, NewInvoice};

/// Next per-practice document number ('invoice' | 'credit_note').
pub async fn next_number(conn: &mut PgConnection, tenant_id: Uuid, kind: &str) -> Result<i64, sqlx::Error> {
    sqlx::query_scalar(
        "INSERT INTO billing_counters (tenant_id, kind, last_value) VALUES ($1, $2, 1)
         ON CONFLICT (tenant_id, kind) DO UPDATE SET last_value = billing_counters.last_value + 1
         RETURNING last_value",
    )
    .bind(tenant_id)
    .bind(kind)
    .fetch_one(conn)
    .await
}

pub async fn invoice_account(conn: &mut PgConnection, tenant_id: Uuid, invoice_id: Uuid) -> Result<Option<Uuid>, sqlx::Error> {
    sqlx::query_scalar("SELECT account_id FROM invoices WHERE tenant_id = $1 AND id = $2")
        .bind(tenant_id)
        .bind(invoice_id)
        .fetch_optional(conn)
        .await
}

pub async fn insert_invoice(conn: &mut PgConnection, tenant_id: Uuid, inv: &NewInvoice, invoice_no: i64, by: Uuid) -> Result<Uuid, sqlx::Error> {
    sqlx::query_scalar(
        "INSERT INTO invoices (tenant_id, account_id, patient_id, invoice_no, invoice_date, treating_provider_id,
                               referring_provider_id, service_centre_id, authorisation_number, total_cents, created_by)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11) RETURNING id",
    )
    .bind(tenant_id)
    .bind(inv.account_id)
    .bind(inv.patient_id)
    .bind(invoice_no)
    .bind(inv.invoice_date)
    .bind(inv.treating_provider_id)
    .bind(inv.referring_provider_id)
    .bind(inv.service_centre_id)
    .bind(&inv.authorisation_number)
    .bind(inv.total_cents())
    .bind(by)
    .fetch_one(conn)
    .await
}

/// A line with its reference codes resolved.
pub struct ResolvedLine<'a> {
    pub line_no: i16,
    pub service_date: NaiveDate,
    pub tariff_code_id: Option<Uuid>,
    pub nappi_code: Option<&'a str>,
    pub description: &'a str,
    pub quantity: Decimal,
    pub unit_price_cents: i64,
    pub amount_cents: i64,
    pub place_of_service: &'a str,
    pub assistant_provider_id: Option<Uuid>,
    pub icd10_codes: &'a [String],
    pub modifiers: &'a [String],
}

pub async fn insert_line(conn: &mut PgConnection, tenant_id: Uuid, invoice_id: Uuid, l: &ResolvedLine<'_>) -> Result<(), sqlx::Error> {
    let line_id: Uuid = sqlx::query_scalar(
        "INSERT INTO invoice_lines (tenant_id, invoice_id, line_no, service_date, tariff_code_id, nappi_code, description,
                                    quantity, unit_price_cents, amount_cents, place_of_service, assistant_provider_id)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12) RETURNING id",
    )
    .bind(tenant_id)
    .bind(invoice_id)
    .bind(l.line_no)
    .bind(l.service_date)
    .bind(l.tariff_code_id)
    .bind(l.nappi_code)
    .bind(l.description)
    .bind(l.quantity)
    .bind(l.unit_price_cents)
    .bind(l.amount_cents)
    .bind(l.place_of_service)
    .bind(l.assistant_provider_id)
    .fetch_one(&mut *conn)
    .await?;

    sqlx::query(
        "INSERT INTO invoice_line_icd10 (tenant_id, line_id, position, icd10_code)
         SELECT $1, $2, ord, code FROM unnest($3::text[]) WITH ORDINALITY AS u(code, ord)",
    )
    .bind(tenant_id)
    .bind(line_id)
    .bind(l.icd10_codes)
    .execute(&mut *conn)
    .await?;

    if !l.modifiers.is_empty() {
        sqlx::query(
            "INSERT INTO invoice_line_modifiers (tenant_id, line_id, position, modifier_code)
             SELECT $1, $2, ord, code FROM unnest($3::text[]) WITH ORDINALITY AS u(code, ord)",
        )
        .bind(tenant_id)
        .bind(line_id)
        .bind(l.modifiers)
        .execute(&mut *conn)
        .await?;
    }
    Ok(())
}

#[derive(sqlx::FromRow)]
pub struct InvoiceRow {
    pub id: Uuid,
    pub invoice_no: i64,
    pub account_id: Uuid,
    pub patient_id: Uuid,
    pub invoice_date: NaiveDate,
    pub treating_provider_id: Uuid,
    pub referring_provider_id: Option<Uuid>,
    pub service_centre_id: Uuid,
    pub authorisation_number: Option<String>,
    pub total_cents: i64,
    pub created_at: DateTime<Utc>,
}

const INVOICE_COLUMNS: &str = "id, invoice_no, account_id, patient_id, invoice_date, treating_provider_id,
    referring_provider_id, service_centre_id, authorisation_number, total_cents, created_at";

pub async fn find(conn: &mut PgConnection, tenant_id: Uuid, id: Uuid) -> Result<Option<InvoiceRow>, sqlx::Error> {
    sqlx::query_as(&format!("SELECT {INVOICE_COLUMNS} FROM invoices WHERE tenant_id = $1 AND id = $2"))
        .bind(tenant_id)
        .bind(id)
        .fetch_optional(conn)
        .await
}

pub async fn for_account_until(conn: &mut PgConnection, tenant_id: Uuid, account_id: Uuid, until: NaiveDate) -> Result<Vec<InvoiceRow>, sqlx::Error> {
    sqlx::query_as(&format!(
        "SELECT {INVOICE_COLUMNS} FROM invoices
          WHERE tenant_id = $1 AND account_id = $2 AND invoice_date <= $3
          ORDER BY invoice_date, invoice_no"
    ))
    .bind(tenant_id)
    .bind(account_id)
    .bind(until)
    .fetch_all(conn)
    .await
}

pub async fn lines(conn: &mut PgConnection, tenant_id: Uuid, invoice_ids: &[Uuid]) -> Result<Vec<LineResponse>, sqlx::Error> {
    sqlx::query_as(
        "SELECT l.invoice_id, l.id, l.line_no, l.service_date, tc.code AS tariff_code, l.nappi_code, l.description,
                l.quantity, l.unit_price_cents, l.amount_cents, l.place_of_service, l.assistant_provider_id,
                coalesce((SELECT array_agg(d.icd10_code ORDER BY d.position) FROM invoice_line_icd10 d
                           WHERE d.tenant_id = l.tenant_id AND d.line_id = l.id), '{}') AS icd10_codes,
                coalesce((SELECT array_agg(m.modifier_code ORDER BY m.position) FROM invoice_line_modifiers m
                           WHERE m.tenant_id = l.tenant_id AND m.line_id = l.id), '{}') AS modifiers,
                coalesce((SELECT sum(c.amount_cents) FROM credit_note_lines c
                           WHERE c.tenant_id = l.tenant_id AND c.invoice_line_id = l.id), 0)::bigint AS credited_cents
           FROM invoice_lines l
           LEFT JOIN tariff_codes tc ON tc.id = l.tariff_code_id
          WHERE l.tenant_id = $1 AND l.invoice_id = ANY($2)
          ORDER BY l.invoice_id, l.line_no",
    )
    .bind(tenant_id)
    .bind(invoice_ids)
    .fetch_all(conn)
    .await
}

#[derive(sqlx::FromRow)]
pub struct CreditNoteRow {
    pub id: Uuid,
    #[allow(dead_code)]
    pub invoice_id: Uuid,
    pub credit_note_no: i64,
    pub credit_date: NaiveDate,
    pub reason: String,
    pub total_cents: i64,
}

pub async fn credit_notes(conn: &mut PgConnection, tenant_id: Uuid, invoice_ids: &[Uuid]) -> Result<Vec<CreditNoteRow>, sqlx::Error> {
    sqlx::query_as(
        "SELECT id, invoice_id, credit_note_no, credit_date, reason, total_cents FROM credit_notes
          WHERE tenant_id = $1 AND invoice_id = ANY($2) ORDER BY credit_note_no",
    )
    .bind(tenant_id)
    .bind(invoice_ids)
    .fetch_all(conn)
    .await
}

pub async fn credit_note_lines(conn: &mut PgConnection, tenant_id: Uuid, credit_note_ids: &[Uuid]) -> Result<Vec<CreditNoteLineResponse>, sqlx::Error> {
    sqlx::query_as(
        "SELECT credit_note_id, invoice_line_id, amount_cents FROM credit_note_lines
          WHERE tenant_id = $1 AND credit_note_id = ANY($2)",
    )
    .bind(tenant_id)
    .bind(credit_note_ids)
    .fetch_all(conn)
    .await
}

#[allow(clippy::too_many_arguments)]
pub async fn insert_credit_note(
    conn: &mut PgConnection,
    tenant_id: Uuid,
    invoice_id: Uuid,
    no: i64,
    date: NaiveDate,
    reason: &str,
    lines: &[(Uuid, i64)],
    by: Uuid,
) -> Result<Uuid, sqlx::Error> {
    let total: i64 = lines.iter().map(|(_, a)| a).sum();
    let id: Uuid = sqlx::query_scalar(
        "INSERT INTO credit_notes (tenant_id, invoice_id, credit_note_no, credit_date, reason, total_cents, created_by)
         VALUES ($1, $2, $3, $4, $5, $6, $7) RETURNING id",
    )
    .bind(tenant_id)
    .bind(invoice_id)
    .bind(no)
    .bind(date)
    .bind(reason)
    .bind(total)
    .bind(by)
    .fetch_one(&mut *conn)
    .await?;
    let line_ids: Vec<Uuid> = lines.iter().map(|(l, _)| *l).collect();
    let amounts: Vec<i64> = lines.iter().map(|(_, a)| *a).collect();
    sqlx::query(
        "INSERT INTO credit_note_lines (tenant_id, credit_note_id, invoice_id, invoice_line_id, amount_cents)
         SELECT $1, $2, $3, l, a FROM unnest($4::uuid[], $5::bigint[]) AS u(l, a)",
    )
    .bind(tenant_id)
    .bind(id)
    .bind(invoice_id)
    .bind(&line_ids)
    .bind(&amounts)
    .execute(&mut *conn)
    .await?;
    Ok(id)
}

/// Which bucket the invoice was originally charged to.
pub async fn charged_bucket(conn: &mut PgConnection, tenant_id: Uuid, invoice_id: Uuid) -> Result<Option<String>, sqlx::Error> {
    sqlx::query_scalar(
        "SELECT e.bucket FROM transactions t
           JOIN transaction_entries e ON e.tenant_id = t.tenant_id AND e.transaction_id = t.id
          WHERE t.tenant_id = $1 AND t.invoice_id = $2 AND t.kind = 'invoice'
          LIMIT 1",
    )
    .bind(tenant_id)
    .bind(invoice_id)
    .fetch_optional(conn)
    .await
}

#[derive(sqlx::FromRow)]
pub struct ListRow {
    pub id: Uuid,
    pub invoice_no: i64,
    pub invoice_date: NaiveDate,
    pub account_id: Uuid,
    pub account_no: i64,
    pub patient_id: Uuid,
    pub dependant_code: String,
    pub first_name: String,
    pub last_name: String,
    pub treating_provider_id: Uuid,
    pub provider_title: Option<String>,
    pub provider_first_name: Option<String>,
    pub provider_last_name: String,
    pub total_cents: i64,
    pub credited_cents: i64,
    pub medical_aid_due_cents: i64,
    pub patient_due_cents: i64,
    pub claim_status: Option<String>,
    pub total_count: i64,
}

/// Invoices matching the filters, newest first, with what's still owed.
pub async fn search(conn: &mut PgConnection, tenant_id: Uuid, s: &InvoiceSearch) -> Result<Vec<ListRow>, sqlx::Error> {
    sqlx::query_as(
        "WITH found AS (
           SELECT i.id,
                  coalesce(sum(e.amount_cents) FILTER (WHERE e.bucket = 'medical_aid'), 0)::bigint AS medical_aid_due_cents,
                  coalesce(sum(e.amount_cents) FILTER (WHERE e.bucket = 'patient'), 0)::bigint AS patient_due_cents
             FROM invoices i
             LEFT JOIN transaction_entries e ON e.tenant_id = i.tenant_id AND e.invoice_id = i.id
            WHERE i.tenant_id = $1
              AND ($2::date IS NULL OR i.invoice_date >= $2)
              AND ($3::date IS NULL OR i.invoice_date <= $3)
              AND ($4::uuid IS NULL OR i.treating_provider_id = $4)
              AND ($5::uuid IS NULL OR i.patient_id = $5)
              AND ($6::uuid IS NULL OR i.account_id = $6)
            GROUP BY i.id
         )
         SELECT i.id, i.invoice_no, i.invoice_date, i.account_id, a.account_no, i.patient_id, pt.dependant_code,
                pe.first_name, pe.last_name, i.treating_provider_id, pr.title AS provider_title,
                pr.first_name AS provider_first_name, pr.last_name AS provider_last_name, i.total_cents,
                coalesce((SELECT sum(cn.total_cents) FROM credit_notes cn
                           WHERE cn.tenant_id = i.tenant_id AND cn.invoice_id = i.id), 0)::bigint AS credited_cents,
                f.medical_aid_due_cents, f.patient_due_cents, c.status AS claim_status,
                count(*) OVER () AS total_count
           FROM found f
           JOIN invoices i  ON i.tenant_id = $1 AND i.id = f.id
           JOIN accounts a  ON a.tenant_id = i.tenant_id AND a.id = i.account_id
           JOIN patients pt ON pt.tenant_id = i.tenant_id AND pt.id = i.patient_id
           JOIN persons pe  ON pe.tenant_id = pt.tenant_id AND pe.id = pt.person_id
           JOIN providers pr ON pr.tenant_id = i.tenant_id AND pr.id = i.treating_provider_id
           LEFT JOIN LATERAL (
             SELECT cl.status FROM claims cl WHERE cl.tenant_id = i.tenant_id AND cl.invoice_id = i.id
              ORDER BY cl.created_at DESC LIMIT 1
           ) c ON true
          WHERE ($7::text IS NULL
                 OR ($7 = 'outstanding' AND (f.medical_aid_due_cents <> 0 OR f.patient_due_cents <> 0))
                 OR ($7 = 'settled' AND f.medical_aid_due_cents = 0 AND f.patient_due_cents = 0))
            AND ($8::text IS NULL OR ($8 = 'none' AND c.status IS NULL) OR c.status = $8)
          ORDER BY i.invoice_date DESC, i.invoice_no DESC
          LIMIT $9 OFFSET $10",
    )
    .bind(tenant_id)
    .bind(s.from)
    .bind(s.to)
    .bind(s.provider_id)
    .bind(s.patient_id)
    .bind(s.account_id)
    .bind(s.status.map(|d| match d {
        super::dto::DueFilter::Outstanding => "outstanding",
        super::dto::DueFilter::Settled => "settled",
    }))
    .bind(s.claim_status.map(|c| c.as_str()))
    .bind(s.page_size)
    .bind(s.offset())
    .fetch_all(conn)
    .await
}

// --- claims ----------------------------------------------------------------------

const CLAIM_SELECT: &str = "
    SELECT c.id, c.invoice_id, i.invoice_no, i.account_id, c.status, c.claimed_cents, c.submitted_date,
           c.status_reason, c.updated_at
      FROM claims c
      JOIN invoices i ON i.tenant_id = c.tenant_id AND i.id = c.invoice_id";

pub async fn insert_claim(conn: &mut PgConnection, tenant_id: Uuid, invoice_id: Uuid, claimed: i64, date: NaiveDate, by: Uuid) -> Result<Uuid, sqlx::Error> {
    sqlx::query_scalar(
        "INSERT INTO claims (tenant_id, invoice_id, claimed_cents, submitted_date, created_by)
         VALUES ($1, $2, $3, $4, $5) RETURNING id",
    )
    .bind(tenant_id)
    .bind(invoice_id)
    .bind(claimed)
    .bind(date)
    .bind(by)
    .fetch_one(conn)
    .await
}

pub async fn find_claim(conn: &mut PgConnection, tenant_id: Uuid, id: Uuid) -> Result<Option<ClaimResponse>, sqlx::Error> {
    sqlx::query_as(&format!("{CLAIM_SELECT} WHERE c.tenant_id = $1 AND c.id = $2"))
        .bind(tenant_id)
        .bind(id)
        .fetch_optional(conn)
        .await
}

pub async fn open_claim(conn: &mut PgConnection, tenant_id: Uuid, invoice_id: Uuid) -> Result<Option<ClaimResponse>, sqlx::Error> {
    sqlx::query_as(&format!(
        "{CLAIM_SELECT} WHERE c.tenant_id = $1 AND c.invoice_id = $2 AND c.status IN ('submitted', 'accepted')"
    ))
    .bind(tenant_id)
    .bind(invoice_id)
    .fetch_optional(conn)
    .await
}

pub async fn claims_for(conn: &mut PgConnection, tenant_id: Uuid, invoice_id: Uuid) -> Result<Vec<ClaimResponse>, sqlx::Error> {
    sqlx::query_as(&format!("{CLAIM_SELECT} WHERE c.tenant_id = $1 AND c.invoice_id = $2 ORDER BY c.created_at"))
        .bind(tenant_id)
        .bind(invoice_id)
        .fetch_all(conn)
        .await
}

pub async fn list_claims(conn: &mut PgConnection, tenant_id: Uuid, status: Option<&str>, limit: i64) -> Result<Vec<ClaimResponse>, sqlx::Error> {
    sqlx::query_as(&format!(
        "{CLAIM_SELECT} WHERE c.tenant_id = $1 AND ($2::text IS NULL OR c.status = $2)
          ORDER BY c.updated_at DESC LIMIT $3"
    ))
    .bind(tenant_id)
    .bind(status)
    .bind(limit)
    .fetch_all(conn)
    .await
}

pub async fn set_claim_status(conn: &mut PgConnection, tenant_id: Uuid, id: Uuid, status: &str, reason: Option<&str>) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE claims SET status = $3, status_reason = $4 WHERE tenant_id = $1 AND id = $2")
        .bind(tenant_id)
        .bind(id)
        .bind(status)
        .bind(reason)
        .execute(conn)
        .await?;
    Ok(())
}
