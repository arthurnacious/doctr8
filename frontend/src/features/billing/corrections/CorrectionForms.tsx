/**
 * Corrections and payments. Each posts a new transaction; nothing posted is
 * changed. TanStack Form + zod; errors show once a field is touched or a
 * save is attempted; backend errors are shown in plain language.
 */
import { useEffect, useRef } from 'react'
import { revalidateLogic, useStore } from '@tanstack/react-form'

import { firstError, useAppForm } from '#/components/form/form'
import { Button } from '#/components/ui/Button'
import { Alert } from '#/components/ui/Field'
import { ApiError, errorMessage } from '#/lib/api/errors'
import { useAccountInvoices } from '../api'
import { mapBackendError } from '../draft/model'
import { formatRand } from '../money'
import {
  useCreditNote,
  useJournal,
  useReasonTypes,
  useRecordPayment,
  useTransfer,
} from './api'
import {
  creditNoteDefaults,
  creditNoteRequest,
  creditNoteSchema,
  journalRequest,
  journalSchema,
  paymentRequest,
  paymentSchema,
  remaining,
  transferRequest,
  transferSchema,
} from './model'
import type { ReactNode, RefObject } from 'react'
import type { Transaction } from './api'
import type { JournalValues, PaymentValues, TransferValues } from './model'
import type { Invoice } from '../types'

function plain(e: unknown): string {
  return e instanceof ApiError
    ? mapBackendError(e.message, []).message
    : errorMessage(e)
}

/** Focus the first field when a form opens. */
function useFocusFirst() {
  const ref = useRef<HTMLFormElement>(null)
  useEffect(() => {
    ref.current
      ?.querySelector<HTMLElement>('input:not([type=hidden]), select, textarea')
      ?.focus()
  }, [])
  return ref
}

function Shell({
  label,
  formRef,
  onSubmit,
  error,
  busy,
  submitLabel,
  onCancel,
  children,
}: {
  label: string
  formRef: RefObject<HTMLFormElement | null>
  onSubmit: () => void
  error: unknown
  busy: boolean
  submitLabel: string
  onCancel: () => void
  children: ReactNode
}) {
  return (
    <form
      ref={formRef}
      aria-label={label}
      noValidate
      onSubmit={(e) => {
        e.preventDefault()
        onSubmit()
      }}
      className="flex flex-col gap-4 rounded-lg border border-border-strong bg-panel p-5"
    >
      <h2 className="text-base font-semibold text-text">{label}</h2>
      {children}
      {error ? <Alert>{plain(error)}</Alert> : null}
      <div className="flex flex-wrap gap-3">
        <Button type="submit" busy={busy}>
          {submitLabel}
        </Button>
        <Button variant="secondary" onClick={onCancel}>
          Cancel
        </Button>
      </div>
    </form>
  )
}

async function attempt(run: () => Promise<unknown>) {
  try {
    return await run()
  } catch {
    return undefined // shown from the mutation's error
  }
}

// -----------------------------------------------------------------------------
// Credit note
// -----------------------------------------------------------------------------

export function CreditNoteForm({
  invoice,
  onDone,
}: {
  invoice: Invoice
  onDone: (message: string | null) => void
}) {
  const credit = useCreditNote(invoice.id)
  const ref = useFocusFirst()
  const byId = new Map(invoice.lines.map((l) => [l.id, l]))
  const form = useAppForm({
    defaultValues: creditNoteDefaults(invoice.lines),
    validationLogic: revalidateLogic(),
    validators: { onDynamic: creditNoteSchema(invoice.lines) },
    onSubmit: async ({ value }) => {
      const r = await attempt(() =>
        credit.mutateAsync(creditNoteRequest(value, invoice.lines)),
      )
      if (r) onDone('Credit note issued.')
    },
  })
  const busy = useStore(form.store, (s) => s.isSubmitting)
  const submitted = useStore(form.store, (s) => s.submissionAttempts > 0)

  return (
    <Shell
      label="Issue a credit note"
      formRef={ref}
      onSubmit={() => void form.handleSubmit()}
      error={credit.error}
      busy={busy}
      submitLabel="Issue credit note"
      onCancel={() => onDone(null)}
    >
      <p className="text-sm text-muted">
        Choose the lines to reverse. A credit note is a new document: the
        invoice itself stays as it was posted.
      </p>
      <form.Field name="lines" mode="array">
        {(linesField) => (
          <fieldset className="flex flex-col gap-2">
            <legend className="sr-only">Lines to credit</legend>
            {linesField.state.value.map((l, i) => {
              const line = byId.get(l.invoice_line_id)
              if (!line) return null
              return (
                <div
                  key={l.invoice_line_id}
                  className="grid items-start gap-3 rounded-lg border border-border bg-card p-3 sm:grid-cols-[1fr_9rem]"
                >
                  <form.Field name={`lines[${i}].selected`}>
                    {(f) => (
                      <label className="flex min-h-11 cursor-pointer items-start gap-3 text-sm text-text">
                        <input
                          type="checkbox"
                          checked={f.state.value}
                          onChange={(e) => f.handleChange(e.target.checked)}
                          className="mt-1 size-5 shrink-0 accent-[var(--color-accent)]"
                        />
                        <span>
                          <span className="font-mono">
                            {line.tariff_code ?? line.nappi_code}
                          </span>{' '}
                          {line.description}
                          <span className="block text-muted">
                            {formatRand(remaining(line))} left to credit
                          </span>
                        </span>
                      </label>
                    )}
                  </form.Field>
                  <form.Field name={`lines[${i}].amount`}>
                    {(f) => {
                      const err = submitted
                        ? firstError(f.state.meta.errors)
                        : undefined
                      return (
                        <div>
                          <label
                            htmlFor={`cn-${l.invoice_line_id}`}
                            className="mb-1 block text-xs font-medium text-muted"
                          >
                            Amount (R)
                          </label>
                          <input
                            id={`cn-${l.invoice_line_id}`}
                            inputMode="decimal"
                            disabled={!l.selected}
                            value={f.state.value}
                            onChange={(e) => f.handleChange(e.target.value)}
                            aria-invalid={err ? true : undefined}
                            aria-label={`Amount to credit on ${line.description}`}
                            className={`block min-h-11 w-full rounded-lg border bg-page px-3 text-right tabular-nums text-text disabled:opacity-50 ${err ? 'border-danger' : 'border-border'}`}
                          />
                          {err && (
                            <p className="mt-1 text-xs text-danger">{err}</p>
                          )}
                        </div>
                      )
                    }}
                  </form.Field>
                </div>
              )
            })}
          </fieldset>
        )}
      </form.Field>
      <form.Subscribe selector={(s) => s.fieldMeta.lines?.errors ?? []}>
        {(errs) =>
          submitted && firstError(errs) ? (
            <p className="text-sm text-danger">{firstError(errs)}</p>
          ) : null
        }
      </form.Subscribe>
      <form.AppField name="reason">
        {(f) => <f.TextInput label="Reason" autoComplete="off" />}
      </form.AppField>
    </Shell>
  )
}

// -----------------------------------------------------------------------------
// Patient payment
// -----------------------------------------------------------------------------

export function PaymentForm({
  accountId,
  invoice,
  today,
  onDone,
}: {
  accountId: string
  /** From an invoice: offer to put the payment on it first. */
  invoice?: Invoice
  today: string
  onDone: (receipt: Transaction | null) => void
}) {
  const pay = useRecordPayment()
  const ref = useFocusFirst()
  const defaults: PaymentValues = {
    method: 'cash',
    amount:
      invoice && invoice.patient_due_cents > 0
        ? (invoice.patient_due_cents / 100).toFixed(2)
        : '',
    reference: '',
    txn_date: today,
    allocate: invoice ? 'invoice' : 'auto',
  }
  const form = useAppForm({
    defaultValues: defaults,
    validationLogic: revalidateLogic(),
    validators: { onDynamic: paymentSchema(today) },
    onSubmit: async ({ value }) => {
      const r = (await attempt(() =>
        pay.mutateAsync(paymentRequest(value, accountId, invoice)),
      )) as Transaction | undefined
      if (r) onDone(r)
    },
  })
  const busy = useStore(form.store, (s) => s.isSubmitting)

  return (
    <Shell
      label="Record a payment"
      formRef={ref}
      onSubmit={() => void form.handleSubmit()}
      error={pay.error}
      busy={busy}
      submitLabel="Record payment"
      onCancel={() => onDone(null)}
    >
      <div className="grid gap-4 sm:grid-cols-2">
        <form.AppField name="method">
          {(f) => (
            <f.SelectInput
              label="Method"
              options={[
                { value: 'cash', label: 'Cash' },
                { value: 'card', label: 'Card' },
                { value: 'eft', label: 'EFT' },
              ]}
            />
          )}
        </form.AppField>
        <form.AppField name="amount">
          {(f) => (
            <f.TextInput
              label="Amount (R)"
              inputMode="decimal"
              autoComplete="off"
            />
          )}
        </form.AppField>
        <form.AppField name="txn_date">
          {(f) => <f.TextInput label="Date received" type="date" max={today} />}
        </form.AppField>
        <form.AppField name="reference">
          {(f) => (
            <f.TextInput
              label="Reference (optional)"
              hint="E.g. the EFT or card slip reference."
              autoComplete="off"
            />
          )}
        </form.AppField>
      </div>
      {invoice && (
        <form.AppField name="allocate">
          {(f) => (
            <f.RadioGroupInput
              legend="Allocate to"
              options={[
                {
                  value: 'invoice',
                  label: `Invoice ${invoice.invoice_no} first`,
                  hint: `The patient owes ${formatRand(invoice.patient_due_cents)} on it; any rest goes to older invoices or stays as credit.`,
                },
                { value: 'auto', label: 'Oldest invoices first' },
              ]}
            />
          )}
        </form.AppField>
      )}
      <p className="text-sm text-muted">
        A receipt number is issued when the payment is recorded.
      </p>
    </Shell>
  )
}

/** What was received and where it went. */
export function ReceiptSummary({
  receipt,
  accountId,
}: {
  receipt: Transaction
  accountId: string
}) {
  const invoices = useAccountInvoices(accountId)
  const number = (id: string) =>
    invoices.data?.find((i) => i.id === id)?.invoice_no
  const allocated = receipt.entries.filter((e) => e.invoice_id)
  const credit = receipt.entries
    .filter((e) => !e.invoice_id)
    .reduce((n, e) => n - e.amount_cents, 0)
  return (
    <section
      role="status"
      aria-label="Payment recorded"
      className="flex flex-col gap-2 rounded-lg border border-border-strong bg-card p-4 text-sm"
    >
      <p className="font-semibold text-text">
        Receipt {receipt.receipt_no} · {formatRand(receipt.amount_cents)}{' '}
        <span className="font-normal text-muted">
          {(receipt.payment_method ?? '').toUpperCase()}
          {receipt.reference ? ` · ${receipt.reference}` : ''}
        </span>
      </p>
      <ul aria-label="Allocated to" className="flex flex-col gap-1">
        {allocated.map((e) => (
          <li
            key={e.invoice_id}
            className="flex justify-between gap-4 text-text"
          >
            <span>Invoice {number(e.invoice_id!) ?? '…'}</span>
            <span className="tabular-nums">{formatRand(-e.amount_cents)}</span>
          </li>
        ))}
        {credit > 0 && (
          <li className="flex justify-between gap-4 text-text">
            <span>Kept on the account as credit</span>
            <span className="tabular-nums">{formatRand(credit)}</span>
          </li>
        )}
      </ul>
    </section>
  )
}

// -----------------------------------------------------------------------------
// Liability move
// -----------------------------------------------------------------------------

export function TransferForm({
  invoice,
  onDone,
}: {
  invoice: Invoice
  onDone: (message: string | null) => void
}) {
  const move = useTransfer()
  const ref = useFocusFirst()
  const defaults: TransferValues = {
    from: invoice.medical_aid_due_cents > 0 ? 'medical_aid' : 'patient',
    amount: '',
    notes: '',
  }
  const form = useAppForm({
    defaultValues: defaults,
    validationLogic: revalidateLogic(),
    validators: {
      onDynamic: transferSchema({
        medical_aid: invoice.medical_aid_due_cents,
        patient: invoice.patient_due_cents,
      }),
    },
    onSubmit: async ({ value }) => {
      const r = await attempt(() =>
        move.mutateAsync(transferRequest(value, invoice.id)),
      )
      if (r) onDone('Liability moved.')
    },
  })
  const busy = useStore(form.store, (s) => s.isSubmitting)
  return (
    <Shell
      label="Move liability"
      formRef={ref}
      onSubmit={() => void form.handleSubmit()}
      error={move.error}
      busy={busy}
      submitLabel="Move liability"
      onCancel={() => onDone(null)}
    >
      <form.AppField name="from">
        {(f) => (
          <f.RadioGroupInput
            legend="Move"
            options={[
              {
                value: 'medical_aid',
                label: 'From the medical aid to the patient',
                hint: `E.g. a co-payment the scheme won't pay. The medical aid owes ${formatRand(invoice.medical_aid_due_cents)}.`,
              },
              {
                value: 'patient',
                label: 'From the patient to the medical aid',
                hint: `The patient owes ${formatRand(invoice.patient_due_cents)}.`,
              },
            ]}
          />
        )}
      </form.AppField>
      <div className="grid gap-4 sm:grid-cols-[10rem_1fr]">
        <form.AppField name="amount">
          {(f) => (
            <f.TextInput
              label="Amount (R)"
              inputMode="decimal"
              autoComplete="off"
            />
          )}
        </form.AppField>
        <form.AppField name="notes">
          {(f) => <f.TextInput label="Reason" autoComplete="off" />}
        </form.AppField>
      </div>
    </Shell>
  )
}

// -----------------------------------------------------------------------------
// Journal
// -----------------------------------------------------------------------------

export function JournalForm({
  invoice,
  onDone,
}: {
  invoice: Invoice
  onDone: (message: string | null) => void
}) {
  const journal = useJournal()
  const reasons = useReasonTypes()
  const ref = useFocusFirst()
  const defaults: JournalValues = {
    reason_type_id: '',
    bucket:
      invoice.patient_due_cents !== 0 || !invoice.medical_aid
        ? 'patient'
        : 'medical_aid',
    direction: 'reduce',
    amount: '',
    notes: '',
  }
  const form = useAppForm({
    defaultValues: defaults,
    validationLogic: revalidateLogic(),
    validators: { onDynamic: journalSchema },
    onSubmit: async ({ value }) => {
      const r = await attempt(() =>
        journal.mutateAsync(
          journalRequest(value, invoice.account_id, invoice.id),
        ),
      )
      if (r) onDone('Journal posted.')
    },
  })
  const busy = useStore(form.store, (s) => s.isSubmitting)
  return (
    <Shell
      label="Post a journal"
      formRef={ref}
      onSubmit={() => void form.handleSubmit()}
      error={journal.error ?? reasons.error}
      busy={busy}
      submitLabel="Post journal"
      onCancel={() => onDone(null)}
    >
      <div className="grid gap-4 sm:grid-cols-2">
        <form.AppField name="reason_type_id">
          {(f) => (
            <f.SelectInput
              label="Reason"
              placeholder={reasons.data ? 'Choose…' : 'Loading…'}
              options={(reasons.data ?? [])
                .filter((r) => r.is_active)
                .map((r) => ({ value: r.id, label: r.name }))}
            />
          )}
        </form.AppField>
        <form.AppField name="bucket">
          {(f) => (
            <f.SelectInput
              label="Whose balance"
              options={[
                { value: 'patient', label: 'Patient' },
                ...(invoice.medical_aid
                  ? [{ value: 'medical_aid', label: 'Medical aid' }]
                  : []),
              ]}
            />
          )}
        </form.AppField>
        <form.AppField name="direction">
          {(f) => (
            <f.SelectInput
              label="Effect"
              options={[
                {
                  value: 'reduce',
                  label: 'Reduce what is owed (write-off, discount)',
                },
                { value: 'increase', label: 'Increase what is owed' },
              ]}
            />
          )}
        </form.AppField>
        <form.AppField name="amount">
          {(f) => (
            <f.TextInput
              label="Amount (R)"
              inputMode="decimal"
              autoComplete="off"
            />
          )}
        </form.AppField>
      </div>
      <form.AppField name="notes">
        {(f) => (
          <f.TextInput
            label="Note"
            hint="Kept with the journal for staff; never printed on statements."
            autoComplete="off"
          />
        )}
      </form.AppField>
    </Shell>
  )
}
