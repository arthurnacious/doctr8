use std::collections::HashMap;

use sqlx::{PgConnection, PgPool};
use uuid::Uuid;

use super::{
    dto::{
        AllocateRemittanceRequest, BalanceResponse, Bucket, Entry, EntryResponse, JournalRequest, NewTransaction,
        PaymentRequest, ReasonRef, ReasonTypeInput, ReasonTypeResponse, ReasonTypeUpdate, RefundRequest,
        RemittanceFilter, RemittanceRequest, RemittanceResponse, RemittanceSummary, ReverseRequest, TransactionResponse,
        TransferRequest, TxnKind,
    },
    repository::{self as repo, AccountBilling, TxnRow},
};
use crate::{
    common::{
        auth::TenantUser,
        db,
        error::{AppError, AppResult, on_unique},
        validation,
        ws::{Event, EventBus, EventKind},
    },
    modules::billing::{invoices, today},
};

const REASON_UNIQUES: &[(&str, &str)] = &[("journal_reason_types_code_uq", "A journal reason with this code already exists")];
const REMITTANCE_UNIQUES: &[(&str, &str)] = &[("remittances_reference_uq", "This remittance has already been captured")];

#[derive(Clone)]
pub struct TransactionsService {
    pool: PgPool,
    bus: EventBus,
}

impl TransactionsService {
    pub fn new(pool: PgPool, bus: EventBus) -> Self {
        Self { pool, bus }
    }

    pub async fn list_for_account(&self, actor: &TenantUser, account_id: Uuid) -> AppResult<Vec<TransactionResponse>> {
        let mut tx = db::begin(&self.pool, actor.scope()).await?;
        account(&mut tx, actor.tenant_id, account_id).await?;
        let rows = repo::list_for_account(&mut tx, actor.tenant_id, account_id).await?;
        let out = with_entries(&mut tx, actor.tenant_id, rows).await?;
        tx.commit().await?;
        Ok(out)
    }

    pub async fn get(&self, actor: &TenantUser, id: Uuid) -> AppResult<TransactionResponse> {
        let mut tx = db::begin(&self.pool, actor.scope()).await?;
        let t = load(&mut tx, actor.tenant_id, id).await?;
        tx.commit().await?;
        Ok(t)
    }

    pub async fn balance(&self, actor: &TenantUser, account_id: Uuid) -> AppResult<BalanceResponse> {
        let mut tx = db::begin(&self.pool, actor.scope()).await?;
        account(&mut tx, actor.tenant_id, account_id).await?;
        let b = balance(&mut tx, actor.tenant_id, account_id).await?;
        tx.commit().await?;
        Ok(b)
    }

    /// Patient payment. Allocated as requested, or oldest invoice first;
    /// whatever is left stays on the account as credit.
    pub async fn payment(&self, actor: &TenantUser, req: PaymentRequest) -> AppResult<TransactionResponse> {
        let p = req.validate(today())?;
        let t = actor.tenant_id;
        let mut tx = db::begin(&self.pool, actor.scope()).await?;
        db::lock(&mut tx, p.account_id).await?;
        account(&mut tx, t, p.account_id).await?;

        let dues = repo::invoice_dues(&mut tx, t, p.account_id).await?;
        let (allocated, remainder) = match &p.allocations {
            Some(list) => {
                let by_invoice: HashMap<Uuid, i64> = dues.iter().map(|d| (d.invoice_id, d.patient_cents)).collect();
                let mut out = Vec::new();
                for a in list {
                    let due = *by_invoice.get(&a.invoice_id).ok_or(AppError::NotFound("invoice on this account"))?;
                    if a.amount_cents > due {
                        return Err(AppError::validation("An allocation is more than the patient owes on that invoice"));
                    }
                    out.push((a.invoice_id, a.amount_cents));
                }
                let used: i64 = out.iter().map(|(_, c)| c).sum();
                (out, p.amount_cents - used)
            }
            None => {
                let open: Vec<(Uuid, i64)> = dues.iter().filter(|d| d.patient_cents > 0).map(|d| (d.invoice_id, d.patient_cents)).collect();
                allocate_oldest_first(p.amount_cents, &open)
            }
        };

        let mut entries: Vec<Entry> = allocated
            .into_iter()
            .map(|(invoice_id, cents)| Entry { invoice_id: Some(invoice_id), bucket: Bucket::Patient, amount_cents: -cents })
            .collect();
        if remainder > 0 {
            entries.push(Entry { invoice_id: None, bucket: Bucket::Patient, amount_cents: -remainder });
        }

        let mut n = NewTransaction::new(p.account_id, TxnKind::PatientPayment, p.txn_date, p.amount_cents);
        n.payment_method = Some(p.method);
        n.reference = p.reference;
        n.notes = p.notes;
        n.entries = entries;
        let id = post(&mut tx, t, actor.user_id, &n).await?;

        self.bus.publish(&mut tx, &Event::balance_changed(t, p.account_id)).await?;
        let out = load(&mut tx, t, id).await?;
        tx.commit().await?;
        Ok(out)
    }

    /// Refund patient credit: unallocated credit first, then credit sitting
    /// on invoices, oldest first.
    pub async fn refund(&self, actor: &TenantUser, req: RefundRequest) -> AppResult<TransactionResponse> {
        let r = req.validate(today())?;
        let t = actor.tenant_id;
        let mut tx = db::begin(&self.pool, actor.scope()).await?;
        db::lock(&mut tx, r.account_id).await?;
        account(&mut tx, t, r.account_id).await?;

        let (_, _, unallocated) = repo::account_totals(&mut tx, t, r.account_id).await?;
        let dues = repo::invoice_dues(&mut tx, t, r.account_id).await?;
        let mut credits: Vec<(Option<Uuid>, i64)> = Vec::new();
        if unallocated < 0 {
            credits.push((None, -unallocated));
        }
        credits.extend(dues.iter().filter(|d| d.patient_cents < 0).map(|d| (Some(d.invoice_id), -d.patient_cents)));

        let (allocated, short) = allocate_oldest_first(r.amount_cents, &credits);
        if short > 0 {
            return Err(AppError::validation("The refund is more than the patient's credit on this account"));
        }

        let mut n = NewTransaction::new(r.account_id, TxnKind::Refund, r.txn_date, r.amount_cents);
        n.payment_method = Some(r.method);
        n.reference = r.reference;
        n.notes = r.notes;
        n.entries = allocated
            .into_iter()
            .map(|(invoice_id, cents)| Entry { invoice_id, bucket: Bucket::Patient, amount_cents: cents })
            .collect();
        let id = post(&mut tx, t, actor.user_id, &n).await?;

        self.bus.publish(&mut tx, &Event::balance_changed(t, r.account_id)).await?;
        let out = load(&mut tx, t, id).await?;
        tx.commit().await?;
        Ok(out)
    }

    pub async fn journal(&self, actor: &TenantUser, req: JournalRequest) -> AppResult<TransactionResponse> {
        let j = req.validate(today())?;
        let t = actor.tenant_id;
        let mut tx = db::begin(&self.pool, actor.scope()).await?;
        db::lock(&mut tx, j.account_id).await?;
        account(&mut tx, t, j.account_id).await?;

        let reason = repo::find_reason(&mut tx, t, j.reason_type_id).await?.ok_or(AppError::NotFound("journal reason"))?;
        if !reason.is_active {
            return Err(AppError::validation("That journal reason is no longer in use"));
        }
        let on_account: std::collections::HashSet<Uuid> =
            repo::invoice_dues(&mut tx, t, j.account_id).await?.into_iter().map(|d| d.invoice_id).collect();
        if j.entries.iter().any(|e| e.invoice_id.is_some_and(|i| !on_account.contains(&i))) {
            return Err(AppError::NotFound("invoice on this account"));
        }

        let total: i64 = j.entries.iter().map(|e| e.amount_cents.abs()).sum();
        let mut n = NewTransaction::new(j.account_id, TxnKind::Journal, j.txn_date, total);
        n.reason_type_id = Some(reason.id);
        n.notes = Some(j.notes);
        n.entries = j.entries;
        let id = post(&mut tx, t, actor.user_id, &n).await?;

        self.bus.publish(&mut tx, &Event::balance_changed(t, j.account_id)).await?;
        let out = load(&mut tx, t, id).await?;
        tx.commit().await?;
        Ok(out)
    }

    /// Move liability between medical aid and patient on one invoice, e.g. a
    /// co-payment the scheme won't pay. No money changes hands.
    pub async fn transfer(&self, actor: &TenantUser, req: TransferRequest) -> AppResult<TransactionResponse> {
        let r = req.validate(today())?;
        let t = actor.tenant_id;
        let mut tx = db::begin(&self.pool, actor.scope()).await?;
        let account_id = invoices::repository::invoice_account(&mut tx, t, r.invoice_id)
            .await?
            .ok_or(AppError::NotFound("invoice"))?;
        db::lock(&mut tx, account_id).await?;
        let acc = account(&mut tx, t, account_id).await?;

        let due = repo::invoice_due(&mut tx, t, r.invoice_id).await?.ok_or(AppError::NotFound("invoice"))?;
        let available = match r.from {
            Bucket::MedicalAid => due.medical_aid_cents,
            Bucket::Patient => due.patient_cents,
        };
        if r.amount_cents > available {
            return Err(AppError::validation("You can only transfer what is currently owed by that party"));
        }
        if r.from.other() == Bucket::MedicalAid && acc.membership_id.is_none() {
            return Err(AppError::validation("This is a private account: there is no medical aid to transfer to"));
        }

        let mut n = NewTransaction::new(account_id, TxnKind::LiabilityTransfer, r.txn_date, r.amount_cents);
        n.invoice_id = Some(r.invoice_id);
        n.notes = Some(r.notes);
        n.entries = vec![
            Entry { invoice_id: Some(r.invoice_id), bucket: r.from, amount_cents: -r.amount_cents },
            Entry { invoice_id: Some(r.invoice_id), bucket: r.from.other(), amount_cents: r.amount_cents },
        ];
        let id = post(&mut tx, t, actor.user_id, &n).await?;

        self.bus.publish(&mut tx, &Event::balance_changed(t, account_id)).await?;
        let out = load(&mut tx, t, id).await?;
        tx.commit().await?;
        Ok(out)
    }

    /// Post a new transaction that exactly negates `id`. The original stays.
    pub async fn reverse(&self, actor: &TenantUser, id: Uuid, req: ReverseRequest) -> AppResult<TransactionResponse> {
        let notes = validation::text("notes", &req.notes, 500)?;
        let t = actor.tenant_id;
        let mut tx = db::begin(&self.pool, actor.scope()).await?;
        let original = repo::find(&mut tx, t, id).await?.ok_or(AppError::NotFound("transaction"))?;
        db::lock(&mut tx, original.account_id).await?;
        let original = repo::find(&mut tx, t, id).await?.ok_or(AppError::NotFound("transaction"))?;

        let kind = TxnKind::from_db(&original.kind)?;
        let txn_date = req.txn_date.unwrap_or_else(today);
        check_reversal(kind, original.reversed_by.is_some(), original.txn_date, txn_date, today())?;

        let mut n = NewTransaction::new(original.account_id, TxnKind::Reversal, txn_date, original.amount_cents);
        n.reverses_transaction_id = Some(original.id);
        n.remittance_id = original.remittance_id;
        n.invoice_id = original.invoice_id;
        n.notes = Some(notes);
        n.entries = negate(&repo::entries_for(&mut tx, t, &[original.id]).await?)?;
        let rid = post(&mut tx, t, actor.user_id, &n).await?;

        self.bus.publish(&mut tx, &Event::balance_changed(t, original.account_id)).await?;
        if let Some(remittance_id) = original.remittance_id {
            let e = Event::new(t, EventKind::RemittanceAllocated, remittance_id).account(original.account_id);
            self.bus.publish(&mut tx, &e).await?;
        }
        let out = load(&mut tx, t, rid).await?;
        tx.commit().await?;
        Ok(out)
    }

    // --- journal reason types ----------------------------------------------------

    pub async fn list_reasons(&self, actor: &TenantUser, include_inactive: bool) -> AppResult<Vec<ReasonTypeResponse>> {
        let mut tx = db::begin(&self.pool, actor.scope()).await?;
        let rows = repo::list_reasons(&mut tx, actor.tenant_id, include_inactive).await?;
        tx.commit().await?;
        Ok(rows)
    }

    pub async fn create_reason(&self, actor: &TenantUser, input: ReasonTypeInput) -> AppResult<ReasonTypeResponse> {
        let (code, name) = input.validate()?;
        let mut tx = db::begin(&self.pool, actor.scope()).await?;
        let r = repo::insert_reason(&mut tx, actor.tenant_id, &code, &name).await.map_err(on_unique(REASON_UNIQUES))?;
        tx.commit().await?;
        Ok(r)
    }

    pub async fn update_reason(&self, actor: &TenantUser, id: Uuid, input: ReasonTypeUpdate) -> AppResult<ReasonTypeResponse> {
        let name = input.name.map(|n| validation::text("name", &n, 100)).transpose()?;
        let mut tx = db::begin(&self.pool, actor.scope()).await?;
        let r = repo::update_reason(&mut tx, actor.tenant_id, id, name.as_deref(), input.is_active)
            .await?
            .ok_or(AppError::NotFound("journal reason"))?;
        tx.commit().await?;
        Ok(r)
    }

    // --- remittances -------------------------------------------------------------

    pub async fn create_remittance(&self, actor: &TenantUser, req: RemittanceRequest) -> AppResult<RemittanceResponse> {
        let r = req.validate(today())?;
        let t = actor.tenant_id;
        let mut tx = db::begin(&self.pool, actor.scope()).await?;
        if !repo::scheme_exists(&mut tx, r.scheme_id).await? {
            return Err(AppError::NotFound("medical scheme"));
        }
        let id = repo::insert_remittance(&mut tx, t, &r, actor.user_id).await.map_err(on_unique(REMITTANCE_UNIQUES))?;
        self.bus.publish(&mut tx, &Event::new(t, EventKind::RemittanceReceived, id)).await?;
        let out = load_remittance(&mut tx, t, id).await?;
        tx.commit().await?;
        Ok(out)
    }

    pub async fn list_remittances(&self, actor: &TenantUser, f: RemittanceFilter) -> AppResult<Vec<RemittanceSummary>> {
        let mut tx = db::begin(&self.pool, actor.scope()).await?;
        let rows = repo::list_remittances(&mut tx, actor.tenant_id, f.unallocated_only, f.limit.unwrap_or(50).clamp(1, 200)).await?;
        tx.commit().await?;
        Ok(rows)
    }

    pub async fn get_remittance(&self, actor: &TenantUser, id: Uuid) -> AppResult<RemittanceResponse> {
        let mut tx = db::begin(&self.pool, actor.scope()).await?;
        let out = load_remittance(&mut tx, actor.tenant_id, id).await?;
        tx.commit().await?;
        Ok(out)
    }

    /// Allocate part of a remittance to invoices on one account. Claims on
    /// those invoices move to accepted / paid.
    pub async fn allocate_remittance(&self, actor: &TenantUser, id: Uuid, req: AllocateRemittanceRequest) -> AppResult<RemittanceResponse> {
        let (account_id, txn_date, lines) = req.validate(today())?;
        let t = actor.tenant_id;
        let mut tx = db::begin(&self.pool, actor.scope()).await?;
        db::lock(&mut tx, id).await?;
        db::lock(&mut tx, account_id).await?;

        let remittance = repo::find_remittance(&mut tx, t, id).await?.ok_or(AppError::NotFound("remittance"))?;
        let acc = account(&mut tx, t, account_id).await?;
        if acc.scheme_id != Some(remittance.scheme_id) {
            return Err(AppError::Conflict("This remittance is from a different medical scheme than the account's".into()));
        }
        if txn_date < remittance.received_date {
            return Err(AppError::validation("An allocation can't be dated before the remittance was received"));
        }
        let total: i64 = lines.iter().map(|l| l.amount_cents).sum();
        if total > remittance.unallocated_cents {
            return Err(AppError::validation("That's more than is left unallocated on the remittance"));
        }
        let dues: HashMap<Uuid, i64> =
            repo::invoice_dues(&mut tx, t, account_id).await?.into_iter().map(|d| (d.invoice_id, d.medical_aid_cents)).collect();
        for l in &lines {
            let due = *dues.get(&l.invoice_id).ok_or(AppError::NotFound("invoice on this account"))?;
            if l.amount_cents > due {
                return Err(AppError::validation("A line pays more than the medical aid owes on that invoice"));
            }
        }

        let mut n = NewTransaction::new(account_id, TxnKind::Remittance, txn_date, total);
        n.remittance_id = Some(id);
        n.reference = Some(remittance.reference.clone());
        n.entries = lines
            .iter()
            .map(|l| Entry { invoice_id: Some(l.invoice_id), bucket: Bucket::MedicalAid, amount_cents: -l.amount_cents })
            .collect();
        post(&mut tx, t, actor.user_id, &n).await?;

        for l in &lines {
            let after = dues[&l.invoice_id] - l.amount_cents;
            if let Some(event) = invoices::service::claim_paid(&mut tx, t, l.invoice_id, after).await? {
                self.bus.publish(&mut tx, &event.account(account_id)).await?;
            }
        }
        self.bus.publish(&mut tx, &Event::new(t, EventKind::RemittanceAllocated, id).account(account_id)).await?;
        self.bus.publish(&mut tx, &Event::balance_changed(t, account_id)).await?;

        let out = load_remittance(&mut tx, t, id).await?;
        tx.commit().await?;
        Ok(out)
    }
}

// -----------------------------------------------------------------------------
// Ledger helpers shared with invoices and statements
// -----------------------------------------------------------------------------

/// Insert a transaction and its entries. The caller holds the account lock
/// and owns the transaction; the database re-validates the shape at COMMIT.
pub async fn post(conn: &mut PgConnection, tenant_id: Uuid, by: Uuid, t: &NewTransaction) -> AppResult<Uuid> {
    let id = repo::insert_transaction(conn, tenant_id, t, by).await?;
    repo::insert_entries(conn, tenant_id, id, t.account_id, &t.entries).await?;
    Ok(id)
}

pub async fn balance(conn: &mut PgConnection, tenant_id: Uuid, account_id: Uuid) -> AppResult<BalanceResponse> {
    let (ma, pt_invoices, unallocated) = repo::account_totals(conn, tenant_id, account_id).await?;
    Ok(BalanceResponse {
        account_id,
        medical_aid_cents: ma,
        patient_cents: pt_invoices + unallocated,
        total_cents: ma + pt_invoices + unallocated,
        unallocated_cents: unallocated,
    })
}

async fn account(conn: &mut PgConnection, tenant_id: Uuid, account_id: Uuid) -> AppResult<AccountBilling> {
    repo::account_billing(conn, tenant_id, account_id).await?.ok_or(AppError::NotFound("account"))
}

pub async fn load(conn: &mut PgConnection, tenant_id: Uuid, id: Uuid) -> AppResult<TransactionResponse> {
    let row = repo::find(conn, tenant_id, id).await?.ok_or(AppError::NotFound("transaction"))?;
    Ok(with_entries(conn, tenant_id, vec![row]).await?.remove(0))
}

async fn with_entries(conn: &mut PgConnection, tenant_id: Uuid, rows: Vec<TxnRow>) -> AppResult<Vec<TransactionResponse>> {
    let ids: Vec<Uuid> = rows.iter().map(|r| r.id).collect();
    let mut by_txn: HashMap<Uuid, Vec<EntryResponse>> = HashMap::new();
    for e in repo::entries_for(conn, tenant_id, &ids).await? {
        by_txn.entry(e.transaction_id).or_default().push(EntryResponse {
            invoice_id: e.invoice_id,
            bucket: Bucket::from_db(&e.bucket)?,
            amount_cents: e.amount_cents,
        });
    }
    rows.into_iter()
        .map(|r| {
            Ok(TransactionResponse {
                entries: by_txn.remove(&r.id).unwrap_or_default(),
                kind: TxnKind::from_db(&r.kind)?,
                reason: match (r.reason_type_id, r.reason_code, r.reason_name) {
                    (Some(id), Some(code), Some(name)) => Some(ReasonRef { id, code, name }),
                    _ => None,
                },
                id: r.id,
                account_id: r.account_id,
                txn_date: r.txn_date,
                posted_at: r.posted_at,
                amount_cents: r.amount_cents,
                payment_method: r.payment_method,
                reference: r.reference,
                notes: r.notes,
                invoice_id: r.invoice_id,
                claim_id: r.claim_id,
                credit_note_id: r.credit_note_id,
                remittance_id: r.remittance_id,
                reverses_transaction_id: r.reverses_transaction_id,
                reversed_by_transaction_id: r.reversed_by,
            })
        })
        .collect()
}

async fn load_remittance(conn: &mut PgConnection, tenant_id: Uuid, id: Uuid) -> AppResult<RemittanceResponse> {
    let remittance = repo::find_remittance(conn, tenant_id, id).await?.ok_or(AppError::NotFound("remittance"))?;
    let allocations = repo::remittance_allocations(conn, tenant_id, id).await?;
    Ok(RemittanceResponse { remittance, allocations })
}

fn negate(entries: &[repo::EntryRow]) -> AppResult<Vec<Entry>> {
    entries
        .iter()
        .map(|e| Ok(Entry { invoice_id: e.invoice_id, bucket: Bucket::from_db(&e.bucket)?, amount_cents: -e.amount_cents }))
        .collect()
}

// -----------------------------------------------------------------------------
// Pure rules
// -----------------------------------------------------------------------------

/// Spread `amount` over `items` (key, how much it can take) in order.
/// Returns what each item got (non-zero only) and what's left over.
pub fn allocate_oldest_first<K: Copy>(amount: i64, items: &[(K, i64)]) -> (Vec<(K, i64)>, i64) {
    let mut left = amount.max(0);
    let mut out = Vec::new();
    for (key, capacity) in items {
        if left == 0 {
            break;
        }
        let take = left.min((*capacity).max(0));
        if take > 0 {
            out.push((*key, take));
            left -= take;
        }
    }
    (out, left)
}

/// May a transaction of `kind` be reversed now, dated `on`?
pub fn check_reversal(
    kind: TxnKind,
    already_reversed: bool,
    original_date: chrono::NaiveDate,
    on: chrono::NaiveDate,
    today: chrono::NaiveDate,
) -> AppResult<()> {
    if !kind.reversible() {
        return Err(AppError::validation(match kind {
            TxnKind::Invoice => "Invoices are corrected with a credit note, not a reversal",
            TxnKind::CreditNote => "Credit notes can't be reversed",
            TxnKind::Reversal => "A reversal can't itself be reversed",
            _ => "This transaction can't be reversed",
        }));
    }
    if already_reversed {
        return Err(AppError::Conflict("This transaction has already been reversed".into()));
    }
    if on < original_date {
        return Err(AppError::validation("A reversal can't be dated before the transaction it reverses"));
    }
    if on > today {
        return Err(AppError::validation("txn_date can't be in the future"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::test_support::date;

    #[test]
    fn allocation_fills_oldest_first() {
        let (got, left) = allocate_oldest_first(150, &[('a', 100), ('b', 100), ('c', 100)]);
        assert_eq!(got, vec![('a', 100), ('b', 50)]);
        assert_eq!(left, 0);
    }

    #[test]
    fn allocation_leaves_remainder_as_credit() {
        let (got, left) = allocate_oldest_first(250, &[('a', 100), ('b', 100)]);
        assert_eq!(got, vec![('a', 100), ('b', 100)]);
        assert_eq!(left, 50);
    }

    #[test]
    fn allocation_skips_empty_and_negative_capacity() {
        let (got, left) = allocate_oldest_first(80, &[('a', 0), ('b', -40), ('c', 50), ('d', 50)]);
        assert_eq!(got, vec![('c', 50), ('d', 30)]);
        assert_eq!(left, 0);
    }

    #[test]
    fn allocation_edge_amounts() {
        assert_eq!(allocate_oldest_first(0, &[('a', 10)]), (vec![], 0));
        assert_eq!(allocate_oldest_first(-5, &[('a', 10)]), (vec![], 0), "never allocates negative amounts");
        assert_eq!(allocate_oldest_first(10, &[] as &[(char, i64)]), (vec![], 10));
    }

    #[test]
    fn allocation_conserves_money() {
        let items = [(1, 333), (2, 1), (3, 0), (4, 999), (5, 12)];
        for amount in [0, 1, 333, 334, 1000, 1345, 5000] {
            let (got, left) = allocate_oldest_first(amount, &items);
            assert_eq!(got.iter().map(|(_, c)| c).sum::<i64>() + left, amount);
            for (k, c) in &got {
                let cap = items.iter().find(|(i, _)| i == k).unwrap().1;
                assert!(*c > 0 && *c <= cap);
            }
        }
    }

    #[test]
    fn reversal_rules() {
        let (d1, d2, today) = (date(2026, 10, 1), date(2026, 10, 5), date(2026, 11, 7));
        assert!(check_reversal(TxnKind::Journal, false, d1, d2, today).is_ok());
        assert!(check_reversal(TxnKind::Journal, false, d1, d1, today).is_ok(), "same day is fine");
        assert!(matches!(check_reversal(TxnKind::Journal, true, d1, d2, today), Err(AppError::Conflict(_))));
        assert!(check_reversal(TxnKind::Journal, false, d2, d1, today).is_err(), "dated before original");
        assert!(check_reversal(TxnKind::Journal, false, d1, date(2026, 11, 8), today).is_err(), "future");
        for k in [TxnKind::Invoice, TxnKind::CreditNote, TxnKind::ClaimSubmitted, TxnKind::Reversal] {
            assert!(check_reversal(k, false, d1, d2, today).is_err(), "{k:?}");
        }
        let msg = check_reversal(TxnKind::Invoice, false, d1, d2, today).unwrap_err().to_string();
        assert!(msg.contains("credit note"));
    }

    #[test]
    fn negation_flips_every_entry() {
        let rows = vec![
            repo::EntryRow { transaction_id: Uuid::nil(), invoice_id: Some(Uuid::from_u128(1)), bucket: "medical_aid".into(), amount_cents: -700 },
            repo::EntryRow { transaction_id: Uuid::nil(), invoice_id: None, bucket: "patient".into(), amount_cents: 50 },
        ];
        let neg = negate(&rows).unwrap();
        assert_eq!(neg[0], Entry { invoice_id: Some(Uuid::from_u128(1)), bucket: Bucket::MedicalAid, amount_cents: 700 });
        assert_eq!(neg[1], Entry { invoice_id: None, bucket: Bucket::Patient, amount_cents: -50 });
    }
}
