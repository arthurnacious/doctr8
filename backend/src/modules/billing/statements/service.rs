use std::collections::{HashMap, HashSet};

use chrono::{NaiveDate, Utc};
use sqlx::PgPool;
use uuid::Uuid;

use super::{
    dto::{
        Ageing, BANDS, LedgerRow, StatementAccount, StatementAddressee, StatementInvoice, StatementLine, StatementMedicalAid,
        StatementPractice, StatementProvider, StatementQuery, StatementResponse, Totals,
    },
    repository as repo,
};
use crate::{
    common::{
        auth::TenantUser,
        db,
        error::{AppError, AppResult},
    },
    modules::{
        access::tenants,
        billing::{
            invoices,
            providers::{self, dto::display_name},
            today,
            transactions::dto::{Bucket, TxnKind},
        },
        people,
    },
};

#[derive(Clone)]
pub struct StatementsService {
    pool: PgPool,
}

impl StatementsService {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn statement(&self, actor: &TenantUser, account_id: Uuid, q: StatementQuery) -> AppResult<StatementResponse> {
        let as_of = q.as_of.unwrap_or_else(today);
        if as_of > today() {
            return Err(AppError::validation("as_of can't be in the future"));
        }
        let t = actor.tenant_id;
        // One consistent snapshot for the whole statement. SET TRANSACTION
        // must come before any other statement, so apply RLS scope after it.
        let mut tx = self.pool.begin().await?;
        sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ, READ ONLY").execute(&mut *tx).await?;
        db::set_scope(&mut tx, actor.scope()).await?;

        let account = people::accounts::repository::find(&mut tx, t, account_id).await?.ok_or(AppError::NotFound("account"))?;
        let practice = tenants::repository::find(&mut tx, t).await?.ok_or(AppError::NotFound("practice"))?;
        let guarantor = people::persons::repository::find(&mut tx, t, account.guarantor_person_id)
            .await?
            .ok_or(AppError::NotFound("guarantor"))?;
        let medical_aid = match (account.membership_id, account.main_member_person_id) {
            (Some(_), Some(main)) => {
                let m = people::persons::repository::find(&mut tx, t, main).await?.ok_or(AppError::NotFound("main member"))?;
                Some(StatementMedicalAid {
                    scheme_name: account.scheme_name.clone().unwrap_or_default(),
                    plan_name: account.plan_name.clone().unwrap_or_default(),
                    membership_number: account.membership_number.clone().unwrap_or_default(),
                    main_member_person_id: main,
                    main_member_name: format!("{} {}", m.first_name, m.last_name),
                })
            }
            _ => None,
        };
        let patients = repo::patients(&mut tx, t, account_id).await?;
        let codes: HashMap<Uuid, String> = patients.iter().map(|p| (p.patient_id, p.dependant_code.clone())).collect();

        let invoice_rows = invoices::repository::for_account_until(&mut tx, t, account_id, as_of).await?;
        let invoice_ids: Vec<Uuid> = invoice_rows.iter().map(|i| i.id).collect();
        let mut lines_by_invoice: HashMap<Uuid, Vec<_>> = HashMap::new();
        for l in invoices::repository::lines(&mut tx, t, &invoice_ids).await? {
            lines_by_invoice.entry(l.invoice_id).or_default().push(l);
        }
        let all_providers: HashMap<Uuid, StatementProvider> = providers::repository::list(&mut tx, t, None, true)
            .await?
            .into_iter()
            .map(|p| {
                let name = display_name(p.title.as_deref(), p.first_name.as_deref(), &p.last_name);
                (p.id, StatementProvider { id: p.id, name, practice_number: p.practice_number, hpcsa_number: p.hpcsa_number })
            })
            .collect();
        let provider_names: HashMap<Uuid, String> = all_providers.iter().map(|(id, p)| (*id, p.name.clone())).collect();
        let centre_names: HashMap<Uuid, String> =
            providers::repository::list_centres(&mut tx, t, true).await?.into_iter().map(|c| (c.id, c.name)).collect();

        let rows = repo::ledger(&mut tx, t, account_id, as_of)
            .await?
            .into_iter()
            .map(|r| {
                Ok(LedgerRow {
                    transaction_id: r.transaction_id,
                    kind: TxnKind::from_db(&r.kind)?,
                    txn_date: r.txn_date,
                    reference: r.reference,
                    notes: r.notes,
                    reason_name: r.reason_name,
                    payment_method: r.payment_method,
                    reverses: r.reverses_transaction_id,
                    reversed_on: r.reversed_on,
                    invoice_id: r.invoice_id,
                    bucket: r.bucket.as_deref().map(Bucket::from_db).transpose()?,
                    amount_cents: r.amount_cents.unwrap_or(0),
                    age_from: r.age_from,
                })
            })
            .collect::<anyhow::Result<Vec<_>>>()?;
        tx.commit().await?;

        let ageing = ageing(as_of, &rows);
        let totals = totals(&rows);
        let mut grouped = group_lines(&rows, as_of, q.include_reversed);
        let dues = dues_by_invoice(&rows);

        let referenced = referenced_providers(&invoice_rows, &lines_by_invoice);
        let mut providers: Vec<StatementProvider> =
            referenced.iter().filter_map(|id| all_providers.get(id).cloned()).collect();
        providers.sort_by(|a, b| a.name.cmp(&b.name));

        let invoices = invoice_rows
            .into_iter()
            .filter_map(|i| {
                let (ma, pt) = dues.get(&i.id).copied().unwrap_or((0, 0));
                if q.outstanding_only && ma == 0 && pt == 0 {
                    return None;
                }
                Some(StatementInvoice {
                    lines: lines_by_invoice.remove(&i.id).unwrap_or_default(),
                    transactions: grouped.remove(&Some(i.id)).unwrap_or_default(),
                    dependant_code: codes.get(&i.patient_id).cloned(),
                    treating_provider_id: i.treating_provider_id,
                    referring_provider_id: i.referring_provider_id,
                    treating_provider: provider_names.get(&i.treating_provider_id).cloned().unwrap_or_default(),
                    referring_provider: i.referring_provider_id.and_then(|r| provider_names.get(&r).cloned()),
                    service_centre: centre_names.get(&i.service_centre_id).cloned().unwrap_or_default(),
                    id: i.id,
                    invoice_no: i.invoice_no,
                    invoice_date: i.invoice_date,
                    patient_id: i.patient_id,
                    authorisation_number: i.authorisation_number,
                    total_cents: i.total_cents,
                    medical_aid_due_cents: ma,
                    patient_due_cents: pt,
                })
            })
            .collect();

        Ok(StatementResponse {
            as_of,
            generated_at: Utc::now(),
            practice: StatementPractice { name: practice.name, pcns_number: practice.pcns_number, details: practice.details },
            account: StatementAccount {
                id: account.id,
                account_no: account.account_no,
                status: account.status,
                file_mode: account.file_mode,
            },
            addressee: StatementAddressee {
                person_id: guarantor.id,
                title: guarantor.title,
                first_name: guarantor.first_name,
                last_name: guarantor.last_name,
                address_line1: guarantor.address_line1,
                address_line2: guarantor.address_line2,
                suburb: guarantor.suburb,
                city: guarantor.city,
                postal_code: guarantor.postal_code,
            },
            medical_aid,
            patients,
            providers,
            invoices,
            unallocated: grouped.remove(&None).unwrap_or_default(),
            totals,
            ageing,
        })
    }
}

/// Treating, referring and assistant doctors named on these invoices.
fn referenced_providers(
    invoices: &[invoices::repository::InvoiceRow],
    lines: &HashMap<Uuid, Vec<invoices::dto::LineResponse>>,
) -> HashSet<Uuid> {
    let mut ids = HashSet::new();
    for i in invoices {
        ids.insert(i.treating_provider_id);
        ids.extend(i.referring_provider_id);
    }
    ids.extend(lines.values().flatten().filter_map(|l| l.assistant_provider_id));
    ids
}

// -----------------------------------------------------------------------------
// Pure statement maths
// -----------------------------------------------------------------------------

/// Ageing band index for money dated `from` on a statement as of `as_of`.
pub fn band(as_of: NaiveDate, from: NaiveDate) -> usize {
    match (as_of - from).num_days() {
        ..30 => 0,
        30..60 => 1,
        60..90 => 2,
        90..120 => 3,
        120..150 => 4,
        150..180 => 5,
        _ => 6,
    }
}

/// Outstanding amounts by band, split medical aid / patient. Every entry
/// counts, so a reversed journal and its reversal cancel in the same band.
pub fn ageing(as_of: NaiveDate, rows: &[LedgerRow]) -> Ageing {
    let mut a = Ageing { bands: BANDS, ..Default::default() };
    for r in rows {
        let Some(bucket) = r.bucket else { continue };
        let b = band(as_of, r.age_from);
        match bucket {
            Bucket::MedicalAid => a.medical_aid[b] += r.amount_cents,
            Bucket::Patient => a.patient[b] += r.amount_cents,
        }
        a.total[b] += r.amount_cents;
    }
    a
}

pub fn totals(rows: &[LedgerRow]) -> Totals {
    let mut t = Totals::default();
    for r in rows {
        match r.bucket {
            Some(Bucket::MedicalAid) => t.medical_aid_due_cents += r.amount_cents,
            Some(Bucket::Patient) => t.patient_due_cents += r.amount_cents,
            None => {}
        }
    }
    t.total_due_cents = t.medical_aid_due_cents + t.patient_due_cents;
    t
}

/// (medical aid, patient) owed per invoice.
pub fn dues_by_invoice(rows: &[LedgerRow]) -> HashMap<Uuid, (i64, i64)> {
    let mut out: HashMap<Uuid, (i64, i64)> = HashMap::new();
    for r in rows {
        let (Some(invoice), Some(bucket)) = (r.invoice_id, r.bucket) else { continue };
        let d = out.entry(invoice).or_default();
        match bucket {
            Bucket::MedicalAid => d.0 += r.amount_cents,
            Bucket::Patient => d.1 += r.amount_cents,
        }
    }
    out
}

/// Statement lines grouped by invoice (`None` = unallocated), in posting
/// order. Unless `include_reversed`, transactions reversed by the statement
/// date are hidden together with their reversals.
pub fn group_lines(rows: &[LedgerRow], as_of: NaiveDate, include_reversed: bool) -> HashMap<Option<Uuid>, Vec<StatementLine>> {
    let reversed: HashSet<Uuid> = rows
        .iter()
        .filter(|r| r.reversed_on.is_some_and(|d| d <= as_of))
        .map(|r| r.transaction_id)
        .collect();

    let mut out: HashMap<Option<Uuid>, Vec<StatementLine>> = HashMap::new();
    for r in rows {
        let hidden = reversed.contains(&r.transaction_id) || r.reverses.is_some_and(|o| reversed.contains(&o));
        if hidden && !include_reversed {
            continue;
        }
        let lines = out.entry(r.invoice_id).or_default();
        let line = match lines.iter_mut().find(|l| l.transaction_id == r.transaction_id) {
            Some(l) => l,
            None => {
                lines.push(StatementLine {
                    transaction_id: r.transaction_id,
                    date: r.txn_date,
                    kind: r.kind,
                    description: describe(r),
                    medical_aid_cents: 0,
                    patient_cents: 0,
                    reversed: reversed.contains(&r.transaction_id),
                    reverses_transaction_id: r.reverses,
                });
                lines.last_mut().expect("just pushed")
            }
        };
        match r.bucket {
            Some(Bucket::MedicalAid) => line.medical_aid_cents += r.amount_cents,
            Some(Bucket::Patient) => line.patient_cents += r.amount_cents,
            None => {}
        }
    }
    out
}

/// Statement wording. Uses references and reason names, never notes
/// (free text may hold anything).
pub fn describe(r: &LedgerRow) -> String {
    let base = match (r.kind, r.reason_name.as_deref(), r.payment_method.as_deref()) {
        (TxnKind::Journal, Some(reason), _) => format!("Journal: {reason}"),
        (TxnKind::PatientPayment, _, Some(m)) => format!("Payment received ({})", m.to_uppercase()),
        (TxnKind::Refund, _, Some(m)) => format!("Refund ({})", m.to_uppercase()),
        (k, _, _) => k.label().to_owned(),
    };
    match r.reference.as_deref() {
        Some(reference) if !reference.is_empty() => format!("{base} {reference}"),
        _ => base,
    }
}

#[cfg(test)]
// Amounts are written as rand_cents (100_00 = R100.00).
#[allow(clippy::inconsistent_digit_grouping)]
mod tests {
    use super::*;
    use crate::common::test_support::date;

    const AS_OF: (i32, u32, u32) = (2026, 11, 7);

    fn as_of() -> NaiveDate {
        date(AS_OF.0, AS_OF.1, AS_OF.2)
    }

    fn row(txn: u128, kind: TxnKind, on: NaiveDate, invoice: Option<u128>, bucket: Bucket, cents: i64, age_from: NaiveDate) -> LedgerRow {
        LedgerRow {
            transaction_id: Uuid::from_u128(txn),
            kind,
            txn_date: on,
            reference: None,
            notes: None,
            reason_name: None,
            payment_method: None,
            reverses: None,
            reversed_on: None,
            invoice_id: invoice.map(Uuid::from_u128),
            bucket: Some(bucket),
            amount_cents: cents,
            age_from,
        }
    }

    #[test]
    fn band_edges() {
        let d = as_of();
        let ago = |n: i64| d - chrono::Duration::days(n);
        for (days, expected) in [(0, 0), (29, 0), (30, 1), (59, 1), (60, 2), (89, 2), (90, 3), (119, 3), (120, 4), (149, 4), (150, 5), (179, 5), (180, 6), (900, 6)] {
            assert_eq!(band(d, ago(days)), expected, "{days} days");
        }
        assert_eq!(band(d, d + chrono::Duration::days(3)), 0, "future-dated money is current");
    }

    #[test]
    fn ageing_splits_by_bucket_and_band() {
        let inv_old = date(2026, 6, 1); // 159 days → 150 band
        let inv_new = date(2026, 10, 20); // 18 days → current
        let rows = vec![
            row(1, TxnKind::Invoice, inv_old, Some(10), Bucket::MedicalAid, 100_00, inv_old),
            row(2, TxnKind::LiabilityTransfer, date(2026, 8, 1), Some(10), Bucket::MedicalAid, -30_00, inv_old),
            row(2, TxnKind::LiabilityTransfer, date(2026, 8, 1), Some(10), Bucket::Patient, 30_00, inv_old),
            row(3, TxnKind::Invoice, inv_new, Some(11), Bucket::Patient, 50_00, inv_new),
        ];
        let a = ageing(as_of(), &rows);
        assert_eq!(a.medical_aid, [0, 0, 0, 0, 0, 70_00, 0]);
        assert_eq!(a.patient, [50_00, 0, 0, 0, 0, 30_00, 0]);
        assert_eq!(a.total, [50_00, 0, 0, 0, 0, 100_00, 0]);
        assert_eq!(a.bands, BANDS);
    }

    #[test]
    fn reversed_journal_nets_out_of_ageing_even_across_bands() {
        // Unallocated journal posted 100 days ago, reversed 10 days ago.
        // The reversal ages from the original's date, so both sit in the
        // 90 band and cancel instead of leaving +X in 90 and -X in current.
        let journal_on = as_of() - chrono::Duration::days(100);
        let reversal_on = as_of() - chrono::Duration::days(10);
        let mut journal = row(1, TxnKind::Journal, journal_on, None, Bucket::Patient, 25_00, journal_on);
        journal.reversed_on = Some(reversal_on);
        let mut reversal = row(2, TxnKind::Reversal, reversal_on, None, Bucket::Patient, -25_00, journal_on);
        reversal.reverses = Some(journal.transaction_id);

        let a = ageing(as_of(), &[journal.clone(), reversal.clone()]);
        assert_eq!(a.patient, [0; 7]);
        assert_eq!(a.total, [0; 7]);
        assert_eq!(totals(&[journal, reversal]), Totals::default());
    }

    #[test]
    fn journal_reversed_after_the_statement_date_still_counts() {
        // The ledger query only returns rows up to as_of, so the reversal is
        // absent; the journal stands, and isn't hidden.
        let on = as_of() - chrono::Duration::days(40);
        let mut journal = row(1, TxnKind::Journal, on, Some(5), Bucket::Patient, -10_00, date(2026, 9, 1));
        journal.reversed_on = Some(as_of() + chrono::Duration::days(5));
        let a = ageing(as_of(), std::slice::from_ref(&journal));
        assert_eq!(a.patient[band(as_of(), date(2026, 9, 1))], -10_00);
        let g = group_lines(&[journal], as_of(), false);
        assert_eq!(g[&Some(Uuid::from_u128(5))].len(), 1);
        assert!(!g[&Some(Uuid::from_u128(5))][0].reversed);
    }

    #[test]
    fn reversed_pairs_are_hidden_by_default_and_shown_on_request() {
        let d = date(2026, 10, 1);
        let charge = row(1, TxnKind::Invoice, d, Some(9), Bucket::Patient, 100_00, d);
        let mut payment = row(2, TxnKind::PatientPayment, d, Some(9), Bucket::Patient, -100_00, d);
        payment.reversed_on = Some(date(2026, 10, 3));
        let mut reversal = row(3, TxnKind::Reversal, date(2026, 10, 3), Some(9), Bucket::Patient, 100_00, d);
        reversal.reverses = Some(payment.transaction_id);
        let rows = [charge, payment, reversal];

        let hidden = group_lines(&rows, as_of(), false);
        assert_eq!(hidden[&Some(Uuid::from_u128(9))].iter().map(|l| l.kind).collect::<Vec<_>>(), vec![TxnKind::Invoice]);

        let shown = group_lines(&rows, as_of(), true);
        let lines = &shown[&Some(Uuid::from_u128(9))];
        assert_eq!(lines.len(), 3);
        assert!(lines[1].reversed);
        assert_eq!(lines[2].reverses_transaction_id, Some(Uuid::from_u128(2)));
        // Either way the invoice still owes the full amount.
        assert_eq!(dues_by_invoice(&rows)[&Uuid::from_u128(9)], (0, 100_00));
    }

    #[test]
    fn one_line_per_transaction_per_invoice() {
        let d = date(2026, 10, 1);
        // A transfer has two entries on one invoice: one statement line.
        let rows = vec![
            row(7, TxnKind::LiabilityTransfer, d, Some(1), Bucket::MedicalAid, -40_00, d),
            row(7, TxnKind::LiabilityTransfer, d, Some(1), Bucket::Patient, 40_00, d),
            // A payment split over two invoices plus unallocated: three lines.
            row(8, TxnKind::PatientPayment, d, Some(1), Bucket::Patient, -10_00, d),
            row(8, TxnKind::PatientPayment, d, Some(2), Bucket::Patient, -20_00, d),
            row(8, TxnKind::PatientPayment, d, None, Bucket::Patient, -5_00, d),
        ];
        let g = group_lines(&rows, as_of(), false);
        let inv1 = &g[&Some(Uuid::from_u128(1))];
        assert_eq!(inv1.len(), 2);
        assert_eq!((inv1[0].medical_aid_cents, inv1[0].patient_cents), (-40_00, 40_00));
        assert_eq!(inv1[1].patient_cents, -10_00);
        assert_eq!(g[&Some(Uuid::from_u128(2))][0].patient_cents, -20_00);
        assert_eq!(g[&None][0].patient_cents, -5_00);
    }

    #[test]
    fn claim_records_appear_without_moving_money() {
        let d = date(2026, 10, 2);
        let mut claim = row(4, TxnKind::ClaimSubmitted, d, Some(1), Bucket::MedicalAid, 0, d);
        claim.bucket = None;
        let g = group_lines(std::slice::from_ref(&claim), as_of(), false);
        assert_eq!(g[&Some(Uuid::from_u128(1))][0].kind, TxnKind::ClaimSubmitted);
        assert_eq!(ageing(as_of(), &[claim.clone()]).total, [0; 7]);
        assert_eq!(totals(&[claim]), Totals::default());
    }

    #[test]
    fn totals_add_up() {
        let d = date(2026, 10, 1);
        let rows = vec![
            row(1, TxnKind::Invoice, d, Some(1), Bucket::MedicalAid, 500_00, d),
            row(2, TxnKind::Invoice, d, Some(2), Bucket::Patient, 200_00, d),
            row(3, TxnKind::PatientPayment, d, None, Bucket::Patient, -50_00, d),
        ];
        assert_eq!(totals(&rows), Totals { medical_aid_due_cents: 500_00, patient_due_cents: 150_00, total_due_cents: 650_00 });
        let a = ageing(as_of(), &rows);
        assert_eq!(a.total.iter().sum::<i64>(), 650_00, "ageing always sums to the total due");
    }

    #[test]
    fn descriptions_never_include_notes() {
        let d = date(2026, 10, 1);
        let mut j = row(1, TxnKind::Journal, d, None, Bucket::Patient, -1, d);
        j.reason_name = Some("Small balance write-off".into());
        j.notes = Some("Mrs Nkosi called, said ...".into());
        assert_eq!(describe(&j), "Journal: Small balance write-off");
        let mut p = row(2, TxnKind::PatientPayment, d, None, Bucket::Patient, -1, d);
        p.payment_method = Some("eft".into());
        p.reference = Some("REF123".into());
        assert_eq!(describe(&p), "Payment received (EFT) REF123");
        let r = row(3, TxnKind::Remittance, d, Some(1), Bucket::MedicalAid, -1, d);
        assert_eq!(describe(&r), "Medical aid payment");
    }
}
