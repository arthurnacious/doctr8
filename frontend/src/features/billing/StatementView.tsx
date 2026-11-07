/**
 * The account statement: header, totals, invoices by visit (each with its
 * lines and transactions), unallocated money and ageing. Amounts are
 * right-aligned in tabular figures so columns line up.
 */
import { useState } from 'react'
import { ChevronDown, ChevronRight } from 'lucide-react'

import { Button } from '#/components/ui/Button'
import { FILE_MODE_LABELS, formatDate, fullName } from '../patients/format'
import { formatRand, formatRandOrBlank } from './money'
import {
  BAND_LABELS,
  formatQuantity,
  isSettled,
  outstanding,
  providerLabel,
  visibleLines,
  visitsOf,
} from './statement'
import type { ReactNode } from 'react'
import type {
  InvoiceLine,
  Statement,
  StatementInvoice,
  StatementLine,
  StatementProvider,
} from './types'
import type { Visit } from './statement'

const num = 'text-right tabular-nums whitespace-nowrap'

export interface StatementViewProps {
  statement: Statement
  /** Transaction ids left off (reversed journals and their reversals). */
  hidden: ReadonlySet<string>
  /** Only this patient's invoices; undefined for everyone on the file. */
  patientId?: string
}

export function StatementView({
  statement: s,
  hidden,
  patientId,
}: StatementViewProps) {
  const providers = new Map(s.providers.map((p) => [p.id, p]))
  const invoices = patientId
    ? s.invoices.filter((i) => i.patient_id === patientId)
    : s.invoices
  const visits = visitsOf(invoices, s.patients)
  const filtered = s.patients.find((p) => p.patient_id === patientId)
  // Unallocated money belongs to the account, not to one patient.
  const unallocated = patientId ? [] : visibleLines(s.unallocated, hidden)
  const empty = s.invoices.length === 0 && s.unallocated.length === 0
  const expansion = useExpansion()

  return (
    <article className="statement flex flex-col gap-6" aria-label="Statement">
      <StatementHeader s={s} />

      {empty ? (
        <p className="rounded-lg border border-border bg-panel px-5 py-10 text-center text-sm text-muted">
          No invoices on this account
          {s.as_of ? ` up to ${formatDate(s.as_of)}` : ''}.
        </p>
      ) : (
        <>
          <Totals s={s} />

          <section
            aria-labelledby="visits-heading"
            className="flex flex-col gap-4"
          >
            <div className="no-print flex flex-wrap items-center justify-between gap-3">
              <h2
                id="visits-heading"
                className="text-base font-semibold text-text"
              >
                Invoices by visit
              </h2>
              <div className="flex flex-wrap gap-2">
                <Button
                  variant="ghost"
                  onClick={() => expansion.setAll(invoices, true)}
                >
                  Expand all
                </Button>
                <Button
                  variant="ghost"
                  onClick={() => expansion.setAll(invoices, false)}
                >
                  Collapse all
                </Button>
              </div>
            </div>
            {filtered && (
              <p role="status" className="text-sm text-muted">
                Showing {fullName(filtered)}&rsquo;s invoices only:{' '}
                <span className="tabular-nums text-text">
                  {formatRand(invoices.reduce((n, i) => n + outstanding(i), 0))}
                </span>{' '}
                outstanding. The totals and ageing are for the whole account.
              </p>
            )}
            {visits.length === 0 && (
              <p className="rounded-lg border border-border bg-panel px-5 py-8 text-center text-sm text-muted">
                No invoices for this patient on this statement.
              </p>
            )}
            {visits.map((v) => (
              <VisitSection
                key={v.key}
                visit={v}
                providers={providers}
                hidden={hidden}
                expansion={expansion}
              />
            ))}
          </section>

          {unallocated.length > 0 && (
            <section
              aria-labelledby="unallocated-heading"
              className="statement-block rounded-lg border border-border bg-panel p-5"
            >
              <h2
                id="unallocated-heading"
                className="mb-1 text-base font-semibold text-text"
              >
                Not allocated to an invoice
              </h2>
              <p className="mb-3 text-sm text-muted">
                For example an overpayment, or a payment still to be matched to
                an invoice.
              </p>
              <TransactionsTable lines={unallocated} caption="Unallocated" />
            </section>
          )}

          <AgeingTable s={s} />
        </>
      )}
    </article>
  )
}

// -----------------------------------------------------------------------------
// Header and totals
// -----------------------------------------------------------------------------

function StatementHeader({ s }: { s: Statement }) {
  const p = s.practice
  const a = s.addressee
  const address = (x: {
    address_line1: string | null
    address_line2: string | null
    suburb: string | null
    city: string | null
    postal_code: string | null
  }) =>
    [
      x.address_line1,
      x.address_line2,
      x.suburb,
      [x.city, x.postal_code].filter(Boolean).join(' '),
    ]
      .filter(Boolean)
      .map((l) => (
        <span key={l} className="block">
          {l}
        </span>
      ))

  return (
    <header className="statement-block grid gap-5 rounded-lg border border-border bg-panel p-5 md:grid-cols-2">
      <div>
        <p className="statement-label text-xs font-semibold uppercase tracking-wide text-subtle">
          Practice
        </p>
        <p className="mt-1 text-lg font-semibold text-text">{p.name}</p>
        <p className="text-sm text-muted">
          {p.pcns_number && (
            <span className="block">Practice no {p.pcns_number}</span>
          )}
          {address(p)}
          {p.phone && <span className="block">Tel {p.phone}</span>}
          {p.email && <span className="block">{p.email}</span>}
        </p>
      </div>

      <dl className="grid grid-cols-[auto_1fr] content-start gap-x-4 gap-y-1 text-sm">
        <dt className="text-muted">Statement date</dt>
        <dd className="font-semibold text-text">{formatDate(s.as_of)}</dd>
        <dt className="text-muted">Account</dt>
        <dd className="font-semibold tabular-nums text-text">
          {s.account.account_no}
        </dd>
        <dt className="text-muted">File mode</dt>
        <dd className="text-text">{FILE_MODE_LABELS[s.account.file_mode]}</dd>
        {s.account.status === 'closed' && (
          <>
            <dt className="text-muted">Status</dt>
            <dd className="text-text">Closed</dd>
          </>
        )}
      </dl>

      <div>
        <p className="statement-label text-xs font-semibold uppercase tracking-wide text-subtle">
          Account holder
        </p>
        <p className="mt-1 text-sm text-text">
          <span className="block font-semibold">{fullName(a)}</span>
          <span className="text-muted">{address(a)}</span>
        </p>
      </div>

      <div>
        <p className="statement-label text-xs font-semibold uppercase tracking-wide text-subtle">
          Medical aid
        </p>
        {s.medical_aid ? (
          <dl className="mt-1 grid grid-cols-[auto_1fr] gap-x-4 gap-y-1 text-sm">
            <dt className="text-muted">Main member</dt>
            <dd className="font-semibold text-text">
              {s.medical_aid.main_member_name}
            </dd>
            <dt className="text-muted">Scheme</dt>
            <dd className="text-text">{s.medical_aid.scheme_name}</dd>
            <dt className="text-muted">Plan</dt>
            <dd className="text-text">{s.medical_aid.plan_name}</dd>
            <dt className="text-muted">Membership no</dt>
            <dd className="font-mono text-text">
              {s.medical_aid.membership_number}
            </dd>
          </dl>
        ) : (
          <p className="mt-1 text-sm text-text">Private: no medical aid</p>
        )}
      </div>

      <div className="md:col-span-2">
        <p className="statement-label text-xs font-semibold uppercase tracking-wide text-subtle">
          On this file
        </p>
        <ul className="mt-1 flex flex-wrap gap-x-6 gap-y-1 text-sm">
          {s.patients.map((pt) => (
            <li key={pt.patient_id} className="text-text">
              <span className="mr-2 font-semibold tabular-nums text-muted">
                {pt.dependant_code}
              </span>
              {fullName(pt)}
              {pt.date_of_birth && (
                <span className="text-muted">
                  {' '}
                  · born {formatDate(pt.date_of_birth)}
                </span>
              )}
            </li>
          ))}
        </ul>
      </div>
    </header>
  )
}

function Totals({ s }: { s: Statement }) {
  const t = s.totals
  return (
    <section
      aria-label="Totals"
      className="statement-totals grid gap-3 sm:grid-cols-3"
    >
      <Total label="Total due" cents={t.total_due_cents} strong />
      <Total
        label="Medical aid"
        cents={t.medical_aid_due_cents}
        hint="Owed by the medical aid"
      />
      <Total
        label="Patient liable"
        cents={t.patient_due_cents}
        hint="Owed by the account holder"
      />
    </section>
  )
}

function Total({
  label,
  cents,
  hint,
  strong = false,
}: {
  label: string
  cents: number
  hint?: string
  strong?: boolean
}) {
  return (
    <div className="statement-block rounded-lg border border-border bg-panel p-4">
      <p className="text-sm text-muted">{label}</p>
      <p
        className={`mt-1 text-2xl font-semibold tabular-nums ${strong ? 'text-accent statement-key' : 'text-text'}`}
      >
        {formatRand(cents)}
      </p>
      {hint && <p className="mt-1 text-xs text-subtle">{hint}</p>}
    </div>
  )
}

// -----------------------------------------------------------------------------
// Visits and invoices
// -----------------------------------------------------------------------------

/** Which invoices are open. Settled invoices start collapsed. */
function useExpansion() {
  const [open, setOpen] = useState<Record<string, boolean>>({})
  return {
    isOpen: (inv: StatementInvoice) => open[inv.id] ?? !isSettled(inv),
    toggle: (inv: StatementInvoice) =>
      setOpen((o) => ({ ...o, [inv.id]: !(o[inv.id] ?? !isSettled(inv)) })),
    setAll: (invs: ReadonlyArray<StatementInvoice>, value: boolean) =>
      setOpen(Object.fromEntries(invs.map((i) => [i.id, value]))),
  }
}
type Expansion = ReturnType<typeof useExpansion>

function VisitSection({
  visit,
  providers,
  hidden,
  expansion,
}: {
  visit: Visit
  providers: Map<string, StatementProvider>
  hidden: ReadonlySet<string>
  expansion: Expansion
}) {
  const p = visit.patient
  const headingId = `visit-${visit.key.replace(/[^a-z0-9]/gi, '')}`
  return (
    <section
      aria-labelledby={headingId}
      className="statement-visit rounded-lg border border-border bg-panel"
    >
      <h3
        id={headingId}
        className="border-b border-border px-5 py-3 text-sm font-semibold text-text"
      >
        {formatDate(visit.date)}
        <span className="font-normal text-muted">
          {' · '}
          {p ? (
            <>
              {fullName(p)} ({p.dependant_code})
              {p.date_of_birth ? ` · born ${formatDate(p.date_of_birth)}` : ''}
            </>
          ) : (
            'Patient no longer on this file'
          )}
        </span>
      </h3>
      <div className="flex flex-col divide-y divide-border">
        {visit.invoices.map((inv) => (
          <InvoiceBlock
            key={inv.id}
            inv={inv}
            providers={providers}
            hidden={hidden}
            open={expansion.isOpen(inv)}
            onToggle={() => expansion.toggle(inv)}
          />
        ))}
      </div>
    </section>
  )
}

function InvoiceBlock({
  inv,
  providers,
  hidden,
  open,
  onToggle,
}: {
  inv: StatementInvoice
  providers: Map<string, StatementProvider>
  hidden: ReadonlySet<string>
  open: boolean
  onToggle: () => void
}) {
  const lines = visibleLines(inv.transactions, hidden)
  const detailsId = `invoice-${inv.id}-details`
  const Chevron = open ? ChevronDown : ChevronRight
  const title = (
    <button
      type="button"
      aria-expanded={open}
      aria-controls={detailsId}
      onClick={onToggle}
      className="-ml-2 inline-flex min-h-11 items-center gap-1.5 rounded-lg px-2 text-left font-semibold text-text hover:bg-raised"
    >
      <Chevron
        className="no-print size-4 shrink-0 text-muted"
        aria-hidden="true"
      />
      Invoice <span className="tabular-nums">{inv.invoice_no}</span>
      <span className="font-normal text-muted">
        {' '}
        · {formatDate(inv.invoice_date)}
      </span>
      {isSettled(inv) && (
        <span className="ml-2 rounded border border-border-strong px-1.5 py-0.5 text-xs font-medium text-muted">
          Settled
        </span>
      )}
    </button>
  )
  return (
    <div className="statement-invoice flex flex-col gap-4 p-5">
      <InvoiceHeader inv={inv} providers={providers} title={title} />
      {/* Collapsed on screen only: print always shows the detail. */}
      <div
        id={detailsId}
        hidden={!open}
        className="statement-invoice-details flex flex-col gap-4"
      >
        <InvoiceLines lines={inv.lines} providers={providers} />
        {lines.length > 0 && (
          <TransactionsTable
            lines={lines}
            caption={`Transactions on invoice ${inv.invoice_no}`}
          />
        )}
      </div>
    </div>
  )
}

function InvoiceHeader({
  inv,
  providers,
  title,
}: {
  inv: StatementInvoice
  providers: Map<string, StatementProvider>
  title: ReactNode
}) {
  return (
    <div className="flex flex-wrap items-start justify-between gap-4">
      <div className="min-w-0">
        {title}
        <dl className="mt-1 grid grid-cols-[auto_1fr] gap-x-3 gap-y-0.5 text-sm">
          <Meta label="Treating doctor">
            {providerLabel(
              providers.get(inv.treating_provider_id),
              inv.treating_provider,
            )}
          </Meta>
          <Meta label="Service centre">{inv.service_centre}</Meta>
          {inv.referring_provider_id && (
            <Meta label="Referred by">
              {providerLabel(
                providers.get(inv.referring_provider_id),
                inv.referring_provider ?? '',
              )}
            </Meta>
          )}
          {inv.authorisation_number && (
            <Meta label="Authorisation">
              <span className="font-mono">{inv.authorisation_number}</span>
            </Meta>
          )}
        </dl>
      </div>
      <dl
        aria-label={`Outstanding on invoice ${inv.invoice_no}`}
        className="grid grid-cols-[auto_auto] gap-x-4 gap-y-0.5 text-sm"
      >
        <dt className="text-muted">Medical aid</dt>
        <dd className={`${num} text-text`}>
          {formatRand(inv.medical_aid_due_cents)}
        </dd>
        <dt className="text-muted">Patient</dt>
        <dd className={`${num} text-text`}>
          {formatRand(inv.patient_due_cents)}
        </dd>
        <dt className="font-semibold text-text">Outstanding</dt>
        <dd className={`${num} font-semibold text-text`}>
          {formatRand(outstanding(inv))}
        </dd>
      </dl>
    </div>
  )
}

function Meta({ label, children }: { label: string; children: ReactNode }) {
  return (
    <>
      <dt className="text-muted">{label}</dt>
      <dd className="min-w-0 text-text">{children}</dd>
    </>
  )
}

const th = 'px-2 py-1.5 text-left text-xs font-semibold text-subtle'
const td = 'px-2 py-1.5 align-top'

function InvoiceLines({
  lines,
  providers,
}: {
  lines: Array<InvoiceLine>
  providers: Map<string, StatementProvider>
}) {
  return (
    <div className="statement-table overflow-x-auto">
      <table className="w-full min-w-[44rem] text-sm">
        <caption className="sr-only">Invoice lines</caption>
        <thead className="border-b border-border">
          <tr>
            <th scope="col" className={th}>
              Code
            </th>
            <th scope="col" className={th}>
              Description
            </th>
            <th scope="col" className={th}>
              ICD-10
            </th>
            <th scope="col" className={th} title="Place of service">
              Place
            </th>
            <th scope="col" className={`${th} text-right`}>
              Qty
            </th>
            <th scope="col" className={th}>
              NAPPI / modifiers
            </th>
            <th scope="col" className={`${th} text-right`}>
              Amount
            </th>
          </tr>
        </thead>
        <tbody>
          {lines.map((l) => {
            const assistant = l.assistant_provider_id
              ? providers.get(l.assistant_provider_id)
              : undefined
            return (
              <tr key={l.id} className="border-b border-border last:border-0">
                <td className={`${td} font-mono text-text`}>
                  {l.tariff_code ?? '—'}
                </td>
                <td className={`${td} text-text`}>
                  {l.description}
                  {assistant && (
                    <span className="block text-xs text-muted">
                      Assistant: {providerLabel(assistant, '')}
                    </span>
                  )}
                </td>
                <td className={`${td} font-mono text-muted`}>
                  {l.icd10_codes.join(', ')}
                </td>
                <td className={`${td} tabular-nums text-muted`}>
                  {l.place_of_service}
                </td>
                <td className={`${td} ${num} text-text`}>
                  {formatQuantity(l.quantity)}
                </td>
                <td className={`${td} font-mono text-muted`}>
                  {[l.nappi_code, ...l.modifiers].filter(Boolean).join(', ')}
                </td>
                <td className={`${td} ${num} text-text`}>
                  {formatRand(l.amount_cents)}
                  {l.credited_cents > 0 && (
                    <span className="block text-xs text-muted">
                      credited {formatRand(l.credited_cents)}
                    </span>
                  )}
                </td>
              </tr>
            )
          })}
        </tbody>
      </table>
    </div>
  )
}

function TransactionsTable({
  lines,
  caption,
}: {
  lines: Array<StatementLine>
  caption: string
}) {
  return (
    <div className="statement-table overflow-x-auto">
      <table className="w-full text-sm">
        <caption className="sr-only">{caption}</caption>
        <thead className="border-b border-border">
          <tr>
            <th scope="col" className={th}>
              Date
            </th>
            <th scope="col" className={th}>
              Transaction
            </th>
            <th scope="col" className={`${th} text-right`}>
              Medical aid
            </th>
            <th scope="col" className={`${th} text-right`}>
              Patient
            </th>
          </tr>
        </thead>
        <tbody>
          {lines.map((l) => (
            <tr
              key={l.transaction_id}
              className="border-b border-border last:border-0"
            >
              <td className={`${td} whitespace-nowrap tabular-nums text-muted`}>
                {formatDate(l.date)}
              </td>
              <td className={`${td} text-text`}>
                {l.description}
                {l.reversed && (
                  <span className="ml-2 text-xs text-muted">(reversed)</span>
                )}
              </td>
              <td className={`${td} ${num} text-text`}>
                {formatRandOrBlank(l.medical_aid_cents)}
              </td>
              <td className={`${td} ${num} text-text`}>
                {formatRandOrBlank(l.patient_cents)}
              </td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  )
}

// -----------------------------------------------------------------------------
// Ageing
// -----------------------------------------------------------------------------

function AgeingTable({ s }: { s: Statement }) {
  const a = s.ageing
  const rows: Array<[string, Array<number>, boolean]> = [
    ['Medical aid', a.medical_aid, false],
    ['Patient', a.patient, false],
    ['Total', a.total, true],
  ]
  return (
    <section
      aria-labelledby="ageing-heading"
      className="statement-block rounded-lg border border-border bg-panel p-5"
    >
      <h2
        id="ageing-heading"
        className="mb-3 text-base font-semibold text-text"
      >
        Ageing
      </h2>
      <div className="statement-table overflow-x-auto">
        <table className="w-full min-w-[40rem] text-sm">
          <caption className="sr-only">Amounts outstanding by age</caption>
          <thead className="border-b border-border">
            <tr>
              <td className={th} />
              {a.bands.map((b) => (
                <th key={b} scope="col" className={`${th} text-right`}>
                  {BAND_LABELS[b] ?? b}
                </th>
              ))}
            </tr>
          </thead>
          <tbody>
            {rows.map(([label, values, strong]) => (
              <tr
                key={label}
                className={strong ? 'border-t border-border-strong' : ''}
              >
                <th
                  scope="row"
                  className={`${td} text-left ${strong ? 'font-semibold text-text' : 'font-normal text-muted'}`}
                >
                  {label}
                </th>
                {values.map((v, i) => (
                  <td
                    key={a.bands[i]}
                    className={`${td} ${num} ${strong ? 'font-semibold' : ''} text-text`}
                  >
                    {formatRand(v)}
                  </td>
                ))}
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </section>
  )
}
