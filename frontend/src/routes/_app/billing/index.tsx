import { useId, useState } from 'react'
import { createFileRoute, useNavigate } from '@tanstack/react-router'

import { Button } from '#/components/ui/Button'
import { Alert, TextField } from '#/components/ui/Field'
import { InvoiceTable } from '#/features/billing/InvoiceTable'
import { PatientFilter } from '#/features/billing/PatientFilter'
import {
  INVOICE_PAGE_SIZE,
  useInvoiceSearch,
  useProviders,
} from '#/features/billing/api'
import { providerName } from '#/features/billing/invoice'
import { errorMessage } from '#/lib/api/errors'
import { useCan } from '#/lib/auth/session'
import type { ClaimFilter, DueFilter } from '#/features/billing/api'

/** Dates and ids only: nothing personal in the URL. */
interface BillingSearch {
  from?: string
  to?: string
  doctor?: string
  status?: DueFilter
  claim?: ClaimFilter
  patient?: string
  page?: number
}

const DATE = /^\d{4}-\d{2}-\d{2}$/
const STATUSES: Array<DueFilter> = ['outstanding', 'settled']
const CLAIMS: Array<ClaimFilter> = [
  'none',
  'submitted',
  'accepted',
  'rejected',
  'paid',
]

export const Route = createFileRoute('/_app/billing/')({
  validateSearch: (s: Record<string, unknown>): BillingSearch => {
    const str = (v: unknown) => (typeof v === 'string' && v ? v : undefined)
    const page = Number(s.page)
    return {
      from:
        str(s.from) && DATE.test(String(s.from)) ? String(s.from) : undefined,
      to: str(s.to) && DATE.test(String(s.to)) ? String(s.to) : undefined,
      doctor: str(s.doctor),
      status: STATUSES.find((x) => x === s.status),
      claim: CLAIMS.find((x) => x === s.claim),
      patient: str(s.patient),
      page: Number.isInteger(page) && page > 1 ? page : undefined,
    }
  },
  head: () => ({ meta: [{ title: 'Billing · doctr8' }] }),
  component: BillingPage,
})

function BillingPage() {
  const search = Route.useSearch()
  const navigate = useNavigate({ from: Route.fullPath })
  const canSearchPatients = useCan('patients.view')
  const providers = useProviders()
  const [patientLabel, setPatientLabel] = useState<string | undefined>()
  const page = search.page ?? 1
  const result = useInvoiceSearch({
    from: search.from,
    to: search.to,
    providerId: search.doctor,
    patientId: search.patient,
    status: search.status,
    claimStatus: search.claim,
    page,
  })
  const set = (next: Partial<BillingSearch>) =>
    void navigate({
      search: (prev) => ({ ...prev, ...next, page: next.page }),
      replace: true,
    })
  const doctorId = useId()
  const statusId = useId()
  const claimId = useId()
  const rangeError =
    search.from && search.to && search.from > search.to
      ? 'The start date is after the end date'
      : undefined

  const data = result.data
  const pages = data
    ? Math.max(1, Math.ceil(data.total / INVOICE_PAGE_SIZE))
    : 1
  const label =
    patientLabel ??
    (data?.items[0]
      ? `${data.items[0].patient.last_name}, ${data.items[0].patient.first_name} (${data.items[0].patient.dependant_code})`
      : undefined)
  const select =
    'block min-h-11 w-full rounded-lg border border-border bg-card px-3 text-base text-text hover:border-border-strong'

  return (
    <div className="mx-auto flex max-w-6xl flex-col gap-6">
      <h1 className="text-2xl font-semibold tracking-tight text-text">
        Billing
      </h1>

      <div
        role="group"
        aria-label="Filter invoices"
        className="grid gap-4 rounded-lg border border-border bg-panel p-4 sm:grid-cols-2 lg:grid-cols-3"
      >
        <TextField
          label="From"
          type="date"
          value={search.from ?? ''}
          onChange={(e) => set({ from: e.target.value || undefined })}
        />
        <TextField
          label="To"
          type="date"
          value={search.to ?? ''}
          error={rangeError}
          onChange={(e) => set({ to: e.target.value || undefined })}
        />
        <div>
          <label
            htmlFor={doctorId}
            className="mb-1.5 block text-sm font-medium text-text"
          >
            Doctor
          </label>
          <select
            id={doctorId}
            className={select}
            value={search.doctor ?? ''}
            onChange={(e) => set({ doctor: e.target.value || undefined })}
          >
            <option value="">All doctors</option>
            {(providers.data ?? [])
              .filter((p) => p.kind === 'practice')
              .map((p) => (
                <option key={p.id} value={p.id}>
                  {providerName(p)}
                </option>
              ))}
          </select>
        </div>
        <div>
          <label
            htmlFor={statusId}
            className="mb-1.5 block text-sm font-medium text-text"
          >
            Status
          </label>
          <select
            id={statusId}
            className={select}
            value={search.status ?? ''}
            onChange={(e) =>
              set({ status: (e.target.value || undefined) as DueFilter })
            }
          >
            <option value="">Any</option>
            <option value="outstanding">Outstanding</option>
            <option value="settled">Settled</option>
          </select>
        </div>
        <div>
          <label
            htmlFor={claimId}
            className="mb-1.5 block text-sm font-medium text-text"
          >
            Claim
          </label>
          <select
            id={claimId}
            className={select}
            value={search.claim ?? ''}
            onChange={(e) =>
              set({ claim: (e.target.value || undefined) as ClaimFilter })
            }
          >
            <option value="">Any</option>
            <option value="none">Not claimed</option>
            <option value="submitted">Submitted</option>
            <option value="accepted">Accepted</option>
            <option value="rejected">Rejected</option>
            <option value="paid">Paid</option>
          </select>
        </div>
        {canSearchPatients && (
          <PatientFilter
            patientId={search.patient}
            label={search.patient ? label : undefined}
            onChange={(id, l) => {
              setPatientLabel(l)
              set({ patient: id })
            }}
          />
        )}
      </div>

      {result.isError ? (
        <div className="flex flex-col items-start gap-4">
          <Alert>{errorMessage(result.error)}</Alert>
          <Button variant="secondary" onClick={() => void result.refetch()}>
            Try again
          </Button>
        </div>
      ) : !data ? (
        <p className="py-12 text-center text-sm text-muted">
          Loading invoices…
        </p>
      ) : data.items.length === 0 ? (
        <p className="rounded-lg border border-border bg-panel px-5 py-10 text-center text-sm text-muted">
          {Object.values(search).some(Boolean)
            ? 'No invoices match these filters.'
            : 'No invoices yet. Invoices are created from a patient’s file.'}
        </p>
      ) : (
        <section
          aria-label="Invoices"
          className="rounded-lg border border-border bg-panel p-2 sm:p-4"
        >
          <InvoiceTable items={data.items} caption="Invoices" showAccount />
          <nav
            aria-label="Pages"
            className="mt-3 flex flex-wrap items-center justify-between gap-3 px-2"
          >
            <p role="status" className="text-sm text-muted">
              Showing {(page - 1) * INVOICE_PAGE_SIZE + 1}–
              {(page - 1) * INVOICE_PAGE_SIZE + data.items.length} of{' '}
              {data.total}
            </p>
            <div className="flex gap-2">
              <Button
                variant="secondary"
                disabled={page <= 1}
                onClick={() =>
                  set({ page: page - 1 > 1 ? page - 1 : undefined })
                }
              >
                Previous
              </Button>
              <Button
                variant="secondary"
                disabled={page >= pages}
                onClick={() => set({ page: page + 1 })}
              >
                Next
              </Button>
            </div>
          </nav>
        </section>
      )}
    </div>
  )
}
