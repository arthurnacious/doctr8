/**
 * One invoice, with everything on the printed invoice: practice, patient
 * and medical aid, service details, lines (credited lines marked), totals,
 * what's still owed, credit notes, claims and the transactions against it.
 */
import { Link } from '@tanstack/react-router'

import { useCan } from '#/lib/auth/session'
import { formatDate, fullName } from '../patients/format'
import { useProviders } from './api'
import { HowToPay } from './HowToPay'
import {
  claimLabel,
  dueLabel,
  lineState,
  providerName,
  sexLabel,
} from './invoice'
import { formatRand } from './money'
import { formatQuantity, providerLabel } from './statement'
import { TransactionsTable, num, td, th } from './tables'
import type { ReactNode } from 'react'
import type { Invoice, InvoiceLine } from './types'

export function InvoiceView({
  invoice: inv,
  actions,
}: {
  invoice: Invoice
  /** Buttons for corrections and payments (stage-specific). */
  actions?: ReactNode
}) {
  const latestClaim = inv.claims.at(-1)
  const net = inv.total_cents - inv.credited_cents
  return (
    <article
      className="invoice flex flex-col gap-6"
      aria-label={`Invoice ${inv.invoice_no}`}
    >
      <InvoiceHeader inv={inv} claim={claimLabel(latestClaim?.status)} />
      {actions}

      <section
        aria-labelledby="lines-heading"
        className="invoice-block rounded-lg border border-border bg-panel p-5"
      >
        <h2
          id="lines-heading"
          className="mb-3 text-base font-semibold text-text"
        >
          Services
        </h2>
        <InvoiceLinesTable lines={inv.lines} />
        <Totals inv={inv} net={net} />
      </section>

      {inv.credit_notes.length > 0 && (
        <section
          aria-labelledby="credits-heading"
          className="invoice-block rounded-lg border border-border bg-panel p-5"
        >
          <h2
            id="credits-heading"
            className="mb-3 text-base font-semibold text-text"
          >
            Credit notes
          </h2>
          <ul className="flex flex-col gap-2 text-sm">
            {inv.credit_notes.map((c) => (
              <li
                key={c.id}
                className="flex flex-wrap items-baseline justify-between gap-2"
              >
                <span className="text-text">
                  <span className="font-semibold tabular-nums">
                    Credit note {c.credit_note_no}
                  </span>
                  <span className="text-muted">
                    {' '}
                    · {formatDate(c.credit_date)} · {c.reason}
                  </span>
                </span>
                <span className={`${num} text-text`}>
                  {formatRand(-c.total_cents)}
                </span>
              </li>
            ))}
          </ul>
        </section>
      )}

      {inv.claims.length > 0 && (
        <section
          aria-labelledby="claims-heading"
          className="invoice-block rounded-lg border border-border bg-panel p-5"
        >
          <h2
            id="claims-heading"
            className="mb-3 text-base font-semibold text-text"
          >
            Claims
          </h2>
          <ul className="flex flex-col gap-2 text-sm">
            {inv.claims.map((c) => (
              <li key={c.id} className="flex flex-wrap justify-between gap-2">
                <span className="text-text">
                  {claimLabel(c.status)}
                  <span className="text-muted">
                    {' '}
                    · sent {formatDate(c.submitted_date)}
                    {c.status_reason ? ` · ${c.status_reason}` : ''}
                  </span>
                </span>
                <span className={`${num} text-text`}>
                  {formatRand(c.claimed_cents)}
                </span>
              </li>
            ))}
          </ul>
        </section>
      )}

      <section
        aria-labelledby="txns-heading"
        className="invoice-block rounded-lg border border-border bg-panel p-5"
      >
        <h2
          id="txns-heading"
          className="mb-3 text-base font-semibold text-text"
        >
          Transactions
        </h2>
        <TransactionsTable
          lines={inv.transactions}
          caption={`Transactions on invoice ${inv.invoice_no}`}
        />
      </section>

      <section
        aria-label="Payment details"
        className="invoice-closing invoice-block grid gap-5 rounded-lg border border-border bg-panel p-5 md:grid-cols-2"
      >
        <HowToPay
          practice={inv.practice}
          accountNo={inv.account_no}
          labelClass="invoice-label"
          document="invoice"
        />
        <div>
          <h2 className="invoice-label text-xs font-semibold uppercase tracking-wide text-subtle">
            Medical aid and patient responsibility
          </h2>
          <p className="mt-2 text-sm text-text">
            {inv.medical_aid
              ? 'This invoice is claimed from your medical aid, which pays according to its rules and your plan. Anything it does not pay is the responsibility of the account holder.'
              : 'This is a private account: the account holder is responsible for the full amount.'}
          </p>
        </div>
        <p className="print-only text-xs text-subtle md:col-span-2">
          Invoice {inv.invoice_no} dated {formatDate(inv.invoice_date)}, printed{' '}
          {new Intl.DateTimeFormat('en-ZA', {
            dateStyle: 'medium',
            timeStyle: 'short',
            timeZone: 'Africa/Johannesburg',
          }).format(new Date())}
          .
        </p>
      </section>
    </article>
  )
}

function Label({ children }: { children: ReactNode }) {
  return (
    <p className="invoice-label text-xs font-semibold uppercase tracking-wide text-subtle">
      {children}
    </p>
  )
}

function InvoiceHeader({ inv, claim }: { inv: Invoice; claim: string }) {
  const p = inv.practice
  const canOpenFile = useCan('patients.view')
  const pt = inv.patient
  const sex = sexLabel(pt.sex)
  return (
    <header className="invoice-block grid gap-5 rounded-lg border border-border bg-panel p-5 md:grid-cols-2">
      <div>
        <Label>Practice</Label>
        <p className="mt-1 text-lg font-semibold text-text">{p.name}</p>
        <p className="text-sm text-muted">
          {p.pcns_number && (
            <span className="block">Practice no {p.pcns_number}</span>
          )}
          {[
            p.address_line1,
            p.address_line2,
            p.suburb,
            [p.city, p.postal_code].filter(Boolean).join(' '),
          ]
            .filter(Boolean)
            .map((l) => (
              <span key={l} className="block">
                {l}
              </span>
            ))}
          {p.phone && <span className="block">Tel {p.phone}</span>}
          {p.email && <span className="block">{p.email}</span>}
        </p>
      </div>

      <dl className="grid grid-cols-[auto_1fr] content-start gap-x-4 gap-y-1 text-sm">
        <dt className="text-muted">Invoice</dt>
        <dd className="font-semibold tabular-nums text-text">
          {inv.invoice_no}
        </dd>
        <dt className="text-muted">Invoice date</dt>
        <dd className="text-text">{formatDate(inv.invoice_date)}</dd>
        <dt className="text-muted">Account</dt>
        <dd className="tabular-nums text-text">
          {canOpenFile ? (
            <Link
              to="/patients/file/$accountId"
              params={{ accountId: inv.account_id }}
              search={{ tab: 'billing' }}
              className="no-print underline underline-offset-4"
            >
              {inv.account_no}
            </Link>
          ) : null}
          <span className={canOpenFile ? 'print-only' : ''}>
            {inv.account_no}
          </span>
        </dd>
        <dt className="text-muted">Claim</dt>
        <dd className="text-text">{claim}</dd>
      </dl>

      <div>
        <Label>Patient</Label>
        <p className="mt-1 text-sm text-text">
          <span className="font-semibold">{fullName(pt)}</span>{' '}
          <span className="tabular-nums text-muted">
            (dependant {pt.dependant_code})
          </span>
        </p>
        <p className="text-sm text-muted">
          {pt.date_of_birth && `Born ${formatDate(pt.date_of_birth)}`}
          {pt.date_of_birth && sex ? ' · ' : ''}
          {sex}
        </p>
      </div>

      <div>
        <Label>Medical aid</Label>
        {inv.medical_aid ? (
          <dl className="mt-1 grid grid-cols-[auto_1fr] gap-x-4 gap-y-1 text-sm">
            <dt className="text-muted">Main member</dt>
            <dd className="font-semibold text-text">
              {inv.medical_aid.main_member_name}
            </dd>
            <dt className="text-muted">Scheme</dt>
            <dd className="text-text">
              {inv.medical_aid.scheme_name} · {inv.medical_aid.plan_name}
            </dd>
            <dt className="text-muted">Membership no</dt>
            <dd className="font-mono text-text">
              {inv.medical_aid.membership_number}
            </dd>
          </dl>
        ) : (
          <p className="mt-1 text-sm text-text">Private: no medical aid</p>
        )}
      </div>

      <div className="md:col-span-2">
        <Label>Service</Label>
        <dl className="mt-1 grid grid-cols-[auto_1fr] gap-x-4 gap-y-1 text-sm">
          <dt className="text-muted">Treating doctor</dt>
          <dd className="text-text">
            {providerLabel(inv.treating_provider, inv.treating_provider.name)}
          </dd>
          {inv.referring_provider && (
            <>
              <dt className="text-muted">Referred by</dt>
              <dd className="text-text">
                {providerLabel(
                  inv.referring_provider,
                  inv.referring_provider.name,
                )}
              </dd>
            </>
          )}
          <dt className="text-muted">Service centre</dt>
          <dd className="text-text">{inv.service_centre.name}</dd>
          {inv.authorisation_number && (
            <>
              <dt className="text-muted">Authorisation</dt>
              <dd className="font-mono text-text">
                {inv.authorisation_number}
              </dd>
            </>
          )}
        </dl>
      </div>
    </header>
  )
}

function InvoiceLinesTable({ lines }: { lines: ReadonlyArray<InvoiceLine> }) {
  const providers = useProviders()
  const assistant = (id: string | null) => {
    const p = id ? providers.data?.find((x) => x.id === id) : undefined
    return p
      ? providerLabel(
          {
            id: p.id,
            name: providerName(p),
            practice_number: p.practice_number,
            hpcsa_number: p.hpcsa_number,
          },
          '',
        )
      : null
  }
  return (
    <div className="invoice-table overflow-x-auto">
      <table className="w-full min-w-[48rem] text-sm">
        <caption className="sr-only">Invoice lines</caption>
        <thead className="border-b border-border">
          <tr>
            <th scope="col" className={th}>
              Date
            </th>
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
              Unit price
            </th>
            <th scope="col" className={`${th} text-right`}>
              Amount
            </th>
          </tr>
        </thead>
        <tbody>
          {lines.map((l) => {
            const state = lineState(l)
            return (
              <tr
                key={l.id}
                className="border-b border-border last:border-0"
                data-credited={state}
              >
                <td
                  className={`${td} whitespace-nowrap tabular-nums text-muted`}
                >
                  {formatDate(l.service_date)}
                </td>
                <td className={`${td} font-mono text-text`}>
                  {l.tariff_code ?? '—'}
                </td>
                <td className={`${td} text-text`}>
                  <span className={state === 'credited' ? 'line-through' : ''}>
                    {l.description}
                  </span>
                  {l.assistant_provider_id && (
                    <span className="block text-xs text-muted">
                      Assistant: {assistant(l.assistant_provider_id) ?? '…'}
                    </span>
                  )}
                  {state !== 'open' && (
                    <span className="mt-0.5 block text-xs font-semibold text-danger">
                      {state === 'credited'
                        ? 'Reversed by credit note'
                        : `Part credited: ${formatRand(l.credited_cents)}`}
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
                <td className={`${td} ${num} text-muted`}>
                  {formatRand(l.unit_price_cents)}
                </td>
                <td className={`${td} ${num} text-text`}>
                  <span className={state === 'credited' ? 'line-through' : ''}>
                    {formatRand(l.amount_cents)}
                  </span>
                </td>
              </tr>
            )
          })}
        </tbody>
      </table>
    </div>
  )
}

function Totals({ inv, net }: { inv: Invoice; net: number }) {
  const owed = inv.medical_aid_due_cents + inv.patient_due_cents
  const row = (label: string, cents: number, strong = false) => (
    <>
      <dt className={strong ? 'font-semibold text-text' : 'text-muted'}>
        {label}
      </dt>
      <dd className={`${num} ${strong ? 'font-semibold' : ''} text-text`}>
        {formatRand(cents)}
      </dd>
    </>
  )
  return (
    <div className="invoice-totals mt-4 flex flex-wrap justify-end gap-x-12 gap-y-4 border-t border-border pt-4">
      <dl
        aria-label="Invoice totals"
        className="grid grid-cols-[auto_auto] gap-x-6 gap-y-1 text-sm"
      >
        {row('Invoice total', inv.total_cents)}
        {inv.credited_cents > 0 && row('Credited', -inv.credited_cents)}
        {row('Net', net, true)}
      </dl>
      <dl
        aria-label="Still owed"
        className="grid grid-cols-[auto_auto] gap-x-6 gap-y-1 text-sm"
      >
        {row('Medical aid owes', inv.medical_aid_due_cents)}
        {row('Patient owes', inv.patient_due_cents)}
        {row(
          dueLabel(inv.medical_aid_due_cents, inv.patient_due_cents) ===
            'Settled'
            ? 'Settled'
            : 'Outstanding',
          owed,
          true,
        )}
      </dl>
    </div>
  )
}
