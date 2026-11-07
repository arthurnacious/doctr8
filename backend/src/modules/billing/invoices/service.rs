use std::collections::HashMap;

use sqlx::{PgConnection, PgPool};
use uuid::Uuid;

use super::{
    dto::{
        CentreRef, ClaimResponse, ClaimStatus, ClaimStatusRequest, ClaimsFilter, CreateInvoiceRequest, CreditNoteRequest,
        CreditNoteResponse, InvoiceResponse, InvoiceSummary, LineItem, ProviderRef, SubmitClaimRequest,
        claim_status_after_payment,
    },
    repository::{self as repo, ResolvedLine},
};
use crate::{
    common::{
        auth::TenantUser,
        db,
        error::{AppError, AppResult},
        validation,
        ws::{Event, EventBus, EventKind},
    },
    modules::{
        billing::{
            providers::{self, dto::{ProviderKind, ProviderResponse}},
            today,
            transactions::{
                self,
                dto::{Bucket, Entry, NewTransaction, TxnKind},
            },
        },
        coding,
        people,
    },
};

#[derive(Clone)]
pub struct InvoicesService {
    pool: PgPool,
    bus: EventBus,
}

impl InvoicesService {
    pub fn new(pool: PgPool, bus: EventBus) -> Self {
        Self { pool, bus }
    }

    /// Create and post an invoice. The full amount is initially owed by the
    /// medical aid on a medical aid account, otherwise by the patient.
    pub async fn create(&self, actor: &TenantUser, req: CreateInvoiceRequest) -> AppResult<InvoiceResponse> {
        let inv = req.validate(today())?;
        let t = actor.tenant_id;
        let mut tx = db::begin(&self.pool, actor.scope()).await?;
        db::lock(&mut tx, inv.account_id).await?;

        let account = transactions::repository::account_billing(&mut tx, t, inv.account_id)
            .await?
            .ok_or(AppError::NotFound("account"))?;
        if account.status != "open" {
            return Err(AppError::Conflict("Invoices can't be added to a closed account".into()));
        }
        let patient = people::patients::repository::find(&mut tx, t, inv.patient_id).await?.ok_or(AppError::NotFound("patient"))?;
        if patient.account_id != inv.account_id {
            return Err(AppError::validation("That patient is not on this account"));
        }

        providers::service::usable(&mut tx, t, inv.treating_provider_id, Some(ProviderKind::Practice), "treating provider").await?;
        if let Some(r) = inv.referring_provider_id {
            providers::service::usable(&mut tx, t, r, None, "referring provider").await?;
        }
        let centre = providers::service::usable_centre(&mut tx, t, inv.service_centre_id).await?;

        // Resolve reference codes up front so errors name the line.
        let mut resolved = Vec::with_capacity(inv.lines.len());
        for (i, l) in inv.lines.iter().enumerate() {
            let at = |m: &str| AppError::validation(format!("lines[{i}]: {m}"));
            let (tariff_code_id, default_description) = match &l.item {
                LineItem::Tariff { code_set, code } => {
                    let tc = coding::repository::find_tariff(&mut tx, code_set, code).await?.ok_or_else(|| at("unknown tariff code"))?;
                    (Some(tc.id), tc.description)
                }
                LineItem::Nappi(code) => {
                    let n = coding::repository::find_nappi(&mut tx, code).await?.ok_or_else(|| at("unknown NAPPI code"))?;
                    (None, n.description)
                }
            };
            if !coding::repository::unknown_icd10(&mut tx, &l.icd10_codes).await?.is_empty() {
                return Err(at("unknown or inactive ICD-10 code"));
            }
            if !coding::repository::not_primary_icd10(&mut tx, &l.icd10_codes[..1]).await?.is_empty() {
                return Err(at("the first ICD-10 code can't be used as a primary diagnosis"));
            }
            if !coding::repository::unknown_modifiers(&mut tx, &l.modifiers).await?.is_empty() {
                return Err(at("unknown or inactive modifier"));
            }
            if let Some(a) = l.assistant_provider_id {
                if a == inv.treating_provider_id {
                    return Err(at("the assistant can't be the treating provider"));
                }
                providers::service::usable(&mut tx, t, a, None, "assistant provider").await?;
            }
            let place_of_service = l
                .place_of_service
                .clone()
                .or_else(|| centre.default_place_of_service.clone())
                .ok_or_else(|| at("place_of_service is required (the service centre has no default)"))?;
            resolved.push((tariff_code_id, l.description.clone().unwrap_or(default_description), place_of_service));
        }

        let invoice_no = repo::next_number(&mut tx, t, "invoice").await?;
        let invoice_id = repo::insert_invoice(&mut tx, t, &inv, invoice_no, actor.user_id).await?;
        for (i, (l, (tariff_code_id, description, pos))) in inv.lines.iter().zip(&resolved).enumerate() {
            let nappi = match &l.item {
                LineItem::Nappi(n) => Some(n.as_str()),
                LineItem::Tariff { .. } => None,
            };
            repo::insert_line(
                &mut tx,
                t,
                invoice_id,
                &ResolvedLine {
                    line_no: (i + 1) as i16,
                    service_date: l.service_date,
                    tariff_code_id: *tariff_code_id,
                    nappi_code: nappi,
                    description,
                    quantity: l.quantity,
                    unit_price_cents: l.unit_price_cents,
                    amount_cents: l.amount_cents,
                    place_of_service: pos,
                    assistant_provider_id: l.assistant_provider_id,
                    icd10_codes: &l.icd10_codes,
                    modifiers: &l.modifiers,
                },
            )
            .await?;
        }

        let total = inv.total_cents();
        let bucket = if account.membership_id.is_some() { Bucket::MedicalAid } else { Bucket::Patient };
        let mut charge = NewTransaction::new(inv.account_id, TxnKind::Invoice, inv.invoice_date, total);
        charge.invoice_id = Some(invoice_id);
        charge.reference = Some(format!("INV{invoice_no}"));
        charge.entries = vec![Entry { invoice_id: Some(invoice_id), bucket, amount_cents: total }];
        transactions::service::post(&mut tx, t, actor.user_id, &charge).await?;

        self.bus.publish(&mut tx, &Event::new(t, EventKind::InvoiceCreated, invoice_id).account(inv.account_id).invoice(invoice_id)).await?;
        self.bus.publish(&mut tx, &Event::balance_changed(t, inv.account_id)).await?;

        let out = load(&mut tx, t, invoice_id).await?;
        tx.commit().await?;
        Ok(out)
    }

    pub async fn get(&self, actor: &TenantUser, id: Uuid) -> AppResult<InvoiceResponse> {
        let mut tx = db::begin(&self.pool, actor.scope()).await?;
        let out = load(&mut tx, actor.tenant_id, id).await?;
        tx.commit().await?;
        Ok(out)
    }

    pub async fn list_for_account(&self, actor: &TenantUser, account_id: Uuid) -> AppResult<Vec<InvoiceSummary>> {
        let mut tx = db::begin(&self.pool, actor.scope()).await?;
        if transactions::repository::account_billing(&mut tx, actor.tenant_id, account_id).await?.is_none() {
            return Err(AppError::NotFound("account"));
        }
        let rows = repo::summaries_for_account(&mut tx, actor.tenant_id, account_id).await?;
        tx.commit().await?;
        Ok(rows)
    }

    /// Credit individual lines (fully by default, or partly). Reduces what
    /// the medical aid owes first, then the patient; anything already paid
    /// becomes a credit for whoever was originally charged.
    pub async fn credit(&self, actor: &TenantUser, invoice_id: Uuid, req: CreditNoteRequest) -> AppResult<InvoiceResponse> {
        let (credit_date, reason, wanted) = req.validate(today())?;
        let t = actor.tenant_id;
        let mut tx = db::begin(&self.pool, actor.scope()).await?;
        let account_id = repo::invoice_account(&mut tx, t, invoice_id).await?.ok_or(AppError::NotFound("invoice"))?;
        db::lock(&mut tx, account_id).await?;
        let invoice = repo::find(&mut tx, t, invoice_id).await?.ok_or(AppError::NotFound("invoice"))?;
        if credit_date < invoice.invoice_date {
            return Err(AppError::validation("credit_date can't be before the invoice date"));
        }

        let lines: HashMap<Uuid, (i64, i64)> = repo::lines(&mut tx, t, &[invoice_id])
            .await?
            .into_iter()
            .map(|l| (l.id, (l.amount_cents, l.credited_cents)))
            .collect();
        let mut credit_lines = Vec::with_capacity(wanted.len());
        for w in &wanted {
            let (amount, credited) = *lines.get(&w.invoice_line_id).ok_or(AppError::NotFound("line on this invoice"))?;
            let remaining = amount - credited;
            let cents = w.amount_cents.unwrap_or(remaining);
            if cents <= 0 || cents > remaining {
                return Err(AppError::validation("A line can't be credited for more than what's left on it"));
            }
            credit_lines.push((w.invoice_line_id, cents));
        }
        let total: i64 = credit_lines.iter().map(|(_, c)| c).sum();

        let no = repo::next_number(&mut tx, t, "credit_note").await?;
        let credit_note_id = repo::insert_credit_note(&mut tx, t, invoice_id, no, credit_date, &reason, &credit_lines, actor.user_id).await?;

        let due = transactions::repository::invoice_due(&mut tx, t, invoice_id).await?.ok_or(AppError::NotFound("invoice"))?;
        let charged = repo::charged_bucket(&mut tx, t, invoice_id).await?.map(|b| Bucket::from_db(&b)).transpose()?.unwrap_or(Bucket::Patient);
        let mut posting = NewTransaction::new(account_id, TxnKind::CreditNote, credit_date, total);
        posting.invoice_id = Some(invoice_id);
        posting.credit_note_id = Some(credit_note_id);
        posting.reference = Some(format!("CN{no}"));
        posting.notes = Some(reason);
        posting.entries = split_credit(total, due.medical_aid_cents, due.patient_cents, charged)
            .into_iter()
            .map(|(bucket, cents)| Entry { invoice_id: Some(invoice_id), bucket, amount_cents: -cents })
            .collect();
        transactions::service::post(&mut tx, t, actor.user_id, &posting).await?;

        self.bus.publish(&mut tx, &Event::new(t, EventKind::InvoiceCredited, invoice_id).account(account_id).invoice(invoice_id)).await?;
        self.bus.publish(&mut tx, &Event::balance_changed(t, account_id)).await?;
        let out = load(&mut tx, t, invoice_id).await?;
        tx.commit().await?;
        Ok(out)
    }

    /// Record that a claim was submitted (actual switching comes later).
    pub async fn submit_claim(&self, actor: &TenantUser, invoice_id: Uuid, req: SubmitClaimRequest) -> AppResult<ClaimResponse> {
        let t = actor.tenant_id;
        let mut tx = db::begin(&self.pool, actor.scope()).await?;
        let account_id = repo::invoice_account(&mut tx, t, invoice_id).await?.ok_or(AppError::NotFound("invoice"))?;
        db::lock(&mut tx, account_id).await?;
        let invoice = repo::find(&mut tx, t, invoice_id).await?.ok_or(AppError::NotFound("invoice"))?;

        let account = transactions::repository::account_billing(&mut tx, t, account_id).await?.ok_or(AppError::NotFound("account"))?;
        if account.membership_id.is_none() {
            return Err(AppError::Conflict("This is a private account: there is no medical aid to claim from".into()));
        }
        if repo::open_claim(&mut tx, t, invoice_id).await?.is_some() {
            return Err(AppError::Conflict("This invoice already has an open claim".into()));
        }
        let due = transactions::repository::invoice_due(&mut tx, t, invoice_id).await?.ok_or(AppError::NotFound("invoice"))?;
        let claimed = req.claimed_cents.unwrap_or(due.medical_aid_cents);
        if due.medical_aid_cents <= 0 {
            return Err(AppError::validation("Nothing is owed by the medical aid on this invoice"));
        }
        if claimed <= 0 || claimed > invoice.total_cents {
            return Err(AppError::validation("claimed_cents must be more than zero and no more than the invoice total"));
        }
        let date = req.submitted_date.unwrap_or_else(today);
        if date < invoice.invoice_date || date > today() {
            return Err(AppError::validation("submitted_date must be between the invoice date and today"));
        }

        let claim_id = repo::insert_claim(&mut tx, t, invoice_id, claimed, date, actor.user_id).await?;
        let mut record = NewTransaction::new(account_id, TxnKind::ClaimSubmitted, date, claimed);
        record.invoice_id = Some(invoice_id);
        record.claim_id = Some(claim_id);
        record.reference = Some(format!("INV{}", invoice.invoice_no));
        transactions::service::post(&mut tx, t, actor.user_id, &record).await?;

        self.bus.publish(&mut tx, &claim_event(t, claim_id, invoice_id, account_id, ClaimStatus::Submitted)).await?;
        let out = repo::find_claim(&mut tx, t, claim_id).await?.ok_or(AppError::NotFound("claim"))?;
        tx.commit().await?;
        Ok(out)
    }

    pub async fn set_claim_status(&self, actor: &TenantUser, claim_id: Uuid, req: ClaimStatusRequest) -> AppResult<ClaimResponse> {
        let reason = validation::optional_text("reason", req.reason.as_deref(), 500)?;
        let t = actor.tenant_id;
        let mut tx = db::begin(&self.pool, actor.scope()).await?;
        let claim = repo::find_claim(&mut tx, t, claim_id).await?.ok_or(AppError::NotFound("claim"))?;
        db::lock(&mut tx, claim.account_id).await?;
        let current = ClaimStatus::from_db(&claim.status)?;
        if !current.can_become(req.status) {
            return Err(AppError::Conflict(format!("A claim can't go from {} to {}", current.as_str(), req.status.as_str())));
        }
        repo::set_claim_status(&mut tx, t, claim_id, req.status.as_str(), reason.as_deref()).await?;
        self.bus.publish(&mut tx, &claim_event(t, claim_id, claim.invoice_id, claim.account_id, req.status)).await?;
        let out = repo::find_claim(&mut tx, t, claim_id).await?.ok_or(AppError::NotFound("claim"))?;
        tx.commit().await?;
        Ok(out)
    }

    pub async fn list_claims(&self, actor: &TenantUser, f: ClaimsFilter) -> AppResult<Vec<ClaimResponse>> {
        let mut tx = db::begin(&self.pool, actor.scope()).await?;
        let rows = repo::list_claims(&mut tx, actor.tenant_id, f.status.map(ClaimStatus::as_str), f.limit.unwrap_or(50).clamp(1, 200)).await?;
        tx.commit().await?;
        Ok(rows)
    }
}

fn claim_event(tenant_id: Uuid, claim_id: Uuid, invoice_id: Uuid, account_id: Uuid, status: ClaimStatus) -> Event {
    Event::new(tenant_id, EventKind::ClaimStatusChanged, claim_id)
        .account(account_id)
        .invoice(invoice_id)
        .status(status.as_str())
}

/// After the scheme paid on `invoice_id`, move its open claim along.
/// Returns the event to publish, if the status changed.
pub async fn claim_paid(conn: &mut PgConnection, tenant_id: Uuid, invoice_id: Uuid, medical_aid_due_after: i64) -> AppResult<Option<Event>> {
    let Some(claim) = repo::open_claim(conn, tenant_id, invoice_id).await? else { return Ok(None) };
    let current = ClaimStatus::from_db(&claim.status)?;
    let Some(next) = claim_status_after_payment(current, medical_aid_due_after) else { return Ok(None) };
    repo::set_claim_status(conn, tenant_id, claim.id, next.as_str(), None).await?;
    Ok(Some(claim_event(tenant_id, claim.id, invoice_id, claim.account_id, next)))
}

/// Split a credit of `amount` across buckets: what the medical aid still
/// owes first, then the patient, then (already paid) back to whoever was
/// originally charged.
pub fn split_credit(amount: i64, medical_aid_due: i64, patient_due: i64, charged: Bucket) -> Vec<(Bucket, i64)> {
    let mut left = amount.max(0);
    let mut ma = left.min(medical_aid_due.max(0));
    left -= ma;
    let mut pt = left.min(patient_due.max(0));
    left -= pt;
    match charged {
        Bucket::MedicalAid => ma += left,
        Bucket::Patient => pt += left,
    }
    [(Bucket::MedicalAid, ma), (Bucket::Patient, pt)].into_iter().filter(|(_, c)| *c > 0).collect()
}

fn provider_ref(p: &ProviderResponse) -> ProviderRef {
    ProviderRef {
        id: p.id,
        name: p.display_name(),
        practice_number: p.practice_number.clone(),
        hpcsa_number: p.hpcsa_number.clone(),
    }
}

pub async fn load(conn: &mut PgConnection, tenant_id: Uuid, id: Uuid) -> AppResult<InvoiceResponse> {
    let inv = repo::find(conn, tenant_id, id).await?.ok_or(AppError::NotFound("invoice"))?;
    let treating = providers::service::find(conn, tenant_id, inv.treating_provider_id).await?;
    let referring = match inv.referring_provider_id {
        Some(r) => Some(providers::service::find(conn, tenant_id, r).await?),
        None => None,
    };
    let centre = providers::repository::find_centre(conn, tenant_id, inv.service_centre_id)
        .await?
        .ok_or(AppError::NotFound("service centre"))?;
    let lines = repo::lines(conn, tenant_id, &[id]).await?;
    let notes = repo::credit_notes(conn, tenant_id, &[id]).await?;
    let note_ids: Vec<Uuid> = notes.iter().map(|n| n.id).collect();
    let mut note_lines: HashMap<Uuid, Vec<_>> = HashMap::new();
    for l in repo::credit_note_lines(conn, tenant_id, &note_ids).await? {
        note_lines.entry(l.credit_note_id).or_default().push(l);
    }
    let due = transactions::repository::invoice_due(conn, tenant_id, id).await?.ok_or(AppError::NotFound("invoice"))?;
    let claims = repo::claims_for(conn, tenant_id, id).await?;

    Ok(InvoiceResponse {
        id: inv.id,
        invoice_no: inv.invoice_no,
        account_id: inv.account_id,
        patient_id: inv.patient_id,
        invoice_date: inv.invoice_date,
        treating_provider: provider_ref(&treating),
        referring_provider: referring.as_ref().map(provider_ref),
        service_centre: CentreRef { id: centre.id, kind: centre.kind, name: centre.name },
        authorisation_number: inv.authorisation_number,
        total_cents: inv.total_cents,
        credited_cents: notes.iter().map(|n| n.total_cents).sum(),
        medical_aid_due_cents: due.medical_aid_cents,
        patient_due_cents: due.patient_cents,
        lines,
        credit_notes: notes
            .into_iter()
            .map(|n| CreditNoteResponse {
                lines: note_lines.remove(&n.id).unwrap_or_default(),
                id: n.id,
                credit_note_no: n.credit_note_no,
                credit_date: n.credit_date,
                reason: n.reason,
                total_cents: n.total_cents,
            })
            .collect(),
        claims,
        created_at: inv.created_at,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn credit_reduces_medical_aid_first() {
        assert_eq!(split_credit(300, 1000, 0, Bucket::MedicalAid), vec![(Bucket::MedicalAid, 300)]);
    }

    #[test]
    fn credit_spills_into_patient_portion() {
        // R10 still owed by the scheme, R5 co-payment owed by the patient.
        assert_eq!(split_credit(1200, 1000, 500, Bucket::MedicalAid), vec![(Bucket::MedicalAid, 1000), (Bucket::Patient, 200)]);
    }

    #[test]
    fn credit_on_a_paid_invoice_becomes_credit_for_the_payer() {
        // Fully paid by the patient: the whole credit is owed back to the patient.
        assert_eq!(split_credit(500, 0, 0, Bucket::Patient), vec![(Bucket::Patient, 500)]);
        // Scheme paid in full: the scheme is owed the credit.
        assert_eq!(split_credit(500, 0, 0, Bucket::MedicalAid), vec![(Bucket::MedicalAid, 500)]);
        // Partly outstanding: outstanding first, the rest to the charged bucket.
        assert_eq!(split_credit(500, 200, 0, Bucket::MedicalAid), vec![(Bucket::MedicalAid, 500)]);
        assert_eq!(split_credit(500, 0, 200, Bucket::MedicalAid), vec![(Bucket::MedicalAid, 300), (Bucket::Patient, 200)]);
    }

    #[test]
    fn existing_credit_balances_are_not_eaten_into() {
        assert_eq!(split_credit(100, -50, -20, Bucket::Patient), vec![(Bucket::Patient, 100)]);
    }

    #[test]
    fn split_always_conserves_the_amount() {
        for (amount, ma, pt) in [(1, 0, 0), (999, 500, 500), (1000, 500, 500), (1001, 500, 500), (7, -3, 4)] {
            for charged in [Bucket::MedicalAid, Bucket::Patient] {
                let s = split_credit(amount, ma, pt, charged);
                assert_eq!(s.iter().map(|(_, c)| c).sum::<i64>(), amount);
                assert!(s.iter().all(|(_, c)| *c > 0));
            }
        }
        assert!(split_credit(0, 10, 10, Bucket::Patient).is_empty());
    }

    #[test]
    fn claim_events_carry_ids_and_status_only() {
        let e = claim_event(Uuid::nil(), Uuid::from_u128(1), Uuid::from_u128(2), Uuid::from_u128(3), ClaimStatus::Rejected);
        assert_eq!((e.kind, e.id, e.invoice_id, e.account_id), (EventKind::ClaimStatusChanged, Uuid::from_u128(1), Some(Uuid::from_u128(2)), Some(Uuid::from_u128(3))));
        assert_eq!(e.status.as_deref(), Some("rejected"));
    }
}
