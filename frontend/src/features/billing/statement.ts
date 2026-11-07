/**
 * Statement presentation logic: visits, settled invoices, the patient filter
 * and hiding reversed journals. Pure, so it's unit-tested.
 */
import type {
  Statement,
  StatementInvoice,
  StatementLine,
  StatementPatient,
  StatementProvider,
} from './types'

export interface Visit {
  key: string
  date: string
  patient: StatementPatient | undefined
  invoices: Array<StatementInvoice>
}

/**
 * Invoices grouped into visits (same patient, same day), oldest first so the
 * newest visit is last, as on a paper statement.
 */
export function visitsOf(
  invoices: ReadonlyArray<StatementInvoice>,
  patients: ReadonlyArray<StatementPatient>,
): Array<Visit> {
  const byId = new Map(patients.map((p) => [p.patient_id, p]))
  const visits = new Map<string, Visit>()
  for (const inv of invoices) {
    const key = `${inv.invoice_date}|${inv.patient_id}`
    let v = visits.get(key)
    if (!v) {
      v = {
        key,
        date: inv.invoice_date,
        patient: byId.get(inv.patient_id),
        invoices: [],
      }
      visits.set(key, v)
    }
    v.invoices.push(inv)
  }
  const code = (v: Visit) => v.patient?.dependant_code ?? '99'
  return [...visits.values()]
    .map((v) => ({
      ...v,
      invoices: v.invoices.sort((a, b) => a.invoice_no - b.invoice_no),
    }))
    .sort(
      (a, b) => a.date.localeCompare(b.date) || code(a).localeCompare(code(b)),
    )
}

export function outstanding(inv: StatementInvoice): number {
  return inv.medical_aid_due_cents + inv.patient_due_cents
}

/** Nothing owed by anyone. */
export function isSettled(inv: StatementInvoice): boolean {
  return inv.medical_aid_due_cents === 0 && inv.patient_due_cents === 0
}

/**
 * Journals that were later reversed, and their reversals: they cancel out,
 * so by default they're left off the statement. Reversed payments and
 * other entries stay, marked as reversed.
 */
export function reversedJournalIds(s: Statement): Set<string> {
  const ids = new Set<string>()
  const all = [...s.unallocated, ...s.invoices.flatMap((i) => i.transactions)]
  for (const l of all)
    if (l.kind === 'journal' && l.reversed) ids.add(l.transaction_id)
  return ids
}

export function visibleLines(
  lines: ReadonlyArray<StatementLine>,
  hidden: ReadonlySet<string>,
): Array<StatementLine> {
  return lines.filter(
    (l) =>
      !hidden.has(l.transaction_id) &&
      !(l.reverses_transaction_id && hidden.has(l.reverses_transaction_id)),
  )
}

/** "Dr A Nkosi · Pr 1234567 · HPCSA MP0123456" */
export function providerLabel(
  p: StatementProvider | undefined,
  fallback: string,
) {
  if (!p) return fallback
  return [
    p.name,
    p.practice_number && `Pr ${p.practice_number}`,
    p.hpcsa_number && `HPCSA ${p.hpcsa_number}`,
  ]
    .filter(Boolean)
    .join(' · ')
}

export const BAND_LABELS: Record<string, string> = {
  current: 'Current',
  '30': '30 days',
  '60': '60 days',
  '90': '90 days',
  '120': '120 days',
  '150': '150 days',
  '180+': '180+ days',
}

/** "1" not "1.00"; "2.5" stays. */
export function formatQuantity(q: string): string {
  const n = Number(q)
  return Number.isFinite(n) ? String(n) : q
}

/** Today's date in South Africa as yyyy-mm-dd (statements are SAST). */
export function todayInSA(now: Date = new Date()): string {
  return new Intl.DateTimeFormat('en-CA', {
    timeZone: 'Africa/Johannesburg',
    year: 'numeric',
    month: '2-digit',
    day: '2-digit',
  }).format(now)
}

/** Why a "statement as of" date can't be used, or null if it can. */
export function asOfError(value: string, today: string): string | null {
  if (!/^\d{4}-\d{2}-\d{2}$/.test(value)) return 'Enter a valid date'
  if (value > today) return "A statement can't be dated in the future"
  if (value < '2000-01-01') return 'Check the year'
  return null
}
