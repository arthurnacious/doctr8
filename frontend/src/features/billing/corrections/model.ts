/**
 * Corrections and payments on posted invoices. Nothing posted is edited:
 * each of these is a new transaction. Values, zod checks and the request
 * each form sends.
 */
import { z } from 'zod'

import { parseRand } from '../draft/model'
import type { InvoiceLine } from '../types'

const amount = (label = 'Enter an amount, e.g. 150.00') =>
  z.string().refine((s) => (parseRand(s) ?? 0) > 0, label)

const reason = (what: string) =>
  z
    .string()
    .trim()
    .min(1, `Give a reason for the ${what}`)
    .max(500, 'Keep it under 500 characters')

export const remaining = (
  l: Pick<InvoiceLine, 'amount_cents' | 'credited_cents'>,
) => l.amount_cents - l.credited_cents

// -----------------------------------------------------------------------------
// Credit note: reverse whole lines (or part of one)
// -----------------------------------------------------------------------------

export interface CreditNoteValues {
  reason: string
  lines: Array<{ invoice_line_id: string; selected: boolean; amount: string }>
}

export function creditNoteDefaults(
  lines: ReadonlyArray<InvoiceLine>,
): CreditNoteValues {
  return {
    reason: '',
    lines: lines
      .filter((l) => remaining(l) > 0)
      .map((l) => ({
        invoice_line_id: l.id,
        selected: false,
        amount: (remaining(l) / 100).toFixed(2),
      })),
  }
}

export function creditNoteSchema(lines: ReadonlyArray<InvoiceLine>) {
  const left = new Map(lines.map((l) => [l.id, remaining(l)]))
  return z
    .object({ reason: reason('credit note'), lines: z.array(z.any()) })
    .superRefine((v, ctx) => {
      const chosen = (v.lines as CreditNoteValues['lines'])
        .map((l, i) => [l, i] as const)
        .filter(([l]) => l.selected)
      if (chosen.length === 0)
        ctx.addIssue({
          code: 'custom',
          path: ['lines'],
          message: 'Choose at least one line to credit',
        })
      for (const [l, i] of chosen) {
        const c = parseRand(l.amount)
        const max = left.get(l.invoice_line_id) ?? 0
        if (c === null || c <= 0)
          ctx.addIssue({
            code: 'custom',
            path: ['lines', i, 'amount'],
            message: 'Enter the amount to credit',
          })
        else if (c > max)
          ctx.addIssue({
            code: 'custom',
            path: ['lines', i, 'amount'],
            message: `At most ${(max / 100).toFixed(2)} is left on this line`,
          })
      }
    })
}

/** Whole lines send no amount (the backend credits what's left). */
export function creditNoteRequest(
  v: CreditNoteValues,
  lines: ReadonlyArray<InvoiceLine>,
) {
  const left = new Map(lines.map((l) => [l.id, remaining(l)]))
  return {
    reason: v.reason.trim(),
    lines: v.lines
      .filter((l) => l.selected)
      .map((l) => {
        const c = parseRand(l.amount)
        return c === left.get(l.invoice_line_id)
          ? { invoice_line_id: l.invoice_line_id }
          : { invoice_line_id: l.invoice_line_id, amount_cents: c ?? 0 }
      }),
  }
}

// -----------------------------------------------------------------------------
// Patient payment
// -----------------------------------------------------------------------------

export type PaymentMethod = 'cash' | 'card' | 'eft'

export interface PaymentValues {
  method: PaymentMethod
  amount: string
  reference: string
  txn_date: string
  /** "auto": oldest invoices first; "invoice": this invoice first. */
  allocate: 'auto' | 'invoice'
}

export function paymentSchema(today: string) {
  return z.object({
    method: z.enum(['cash', 'card', 'eft']),
    amount: amount(),
    reference: z
      .string()
      .max(60, 'Up to 60 characters')
      .refine((s) => !/[\r\n]/.test(s), 'One line only'),
    txn_date: z
      .string()
      .regex(/^\d{4}-\d{2}-\d{2}$/, 'Enter the payment date')
      .refine((d) => d <= today, "A payment can't be dated in the future"),
    allocate: z.enum(['auto', 'invoice']),
  })
}

/**
 * Paying "this invoice": allocate up to what the patient owes on it; the
 * backend keeps any rest on the account as credit.
 */
export function paymentRequest(
  v: PaymentValues,
  accountId: string,
  invoice?: { id: string; patient_due_cents: number },
) {
  const cents = parseRand(v.amount) ?? 0
  const toInvoice =
    v.allocate === 'invoice' && invoice && invoice.patient_due_cents > 0
      ? Math.min(cents, invoice.patient_due_cents)
      : 0
  return {
    account_id: accountId,
    method: v.method,
    amount_cents: cents,
    reference: v.reference.trim() || undefined,
    txn_date: v.txn_date,
    allocations:
      toInvoice > 0
        ? [{ invoice_id: invoice!.id, amount_cents: toInvoice }]
        : undefined,
  }
}

// -----------------------------------------------------------------------------
// Moving liability between medical aid and patient
// -----------------------------------------------------------------------------

export interface TransferValues {
  from: 'medical_aid' | 'patient'
  amount: string
  notes: string
}

export function transferSchema(due: { medical_aid: number; patient: number }) {
  return z
    .object({
      from: z.enum(['medical_aid', 'patient']),
      amount: amount(),
      notes: reason('move'),
    })
    .superRefine((v, ctx) => {
      const c = parseRand(v.amount)
      const max = v.from === 'medical_aid' ? due.medical_aid : due.patient
      if (c !== null && c > max)
        ctx.addIssue({
          code: 'custom',
          path: ['amount'],
          message: `${v.from === 'medical_aid' ? 'The medical aid' : 'The patient'} owes only ${(Math.max(max, 0) / 100).toFixed(2)} on this invoice`,
        })
    })
}

export const transferRequest = (v: TransferValues, invoiceId: string) => ({
  invoice_id: invoiceId,
  from: v.from,
  amount_cents: parseRand(v.amount) ?? 0,
  notes: v.notes.trim(),
})

// -----------------------------------------------------------------------------
// Journal with a reason type
// -----------------------------------------------------------------------------

export interface JournalValues {
  reason_type_id: string
  bucket: 'medical_aid' | 'patient'
  /** "reduce": write off / discount; "increase": add a charge back. */
  direction: 'reduce' | 'increase'
  amount: string
  notes: string
}

export const journalSchema = z.object({
  reason_type_id: z.string().min(1, 'Choose the reason'),
  bucket: z.enum(['medical_aid', 'patient']),
  direction: z.enum(['reduce', 'increase']),
  amount: amount(),
  notes: reason('journal'),
})

export function journalRequest(
  v: JournalValues,
  accountId: string,
  invoiceId: string,
) {
  const c = parseRand(v.amount) ?? 0
  return {
    account_id: accountId,
    reason_type_id: v.reason_type_id,
    notes: v.notes.trim(),
    entries: [
      {
        invoice_id: invoiceId,
        bucket: v.bucket,
        amount_cents: v.direction === 'reduce' ? -c : c,
      },
    ],
  }
}
