import { useEffect, useRef, useState } from 'react'
import { Link, createFileRoute, useNavigate } from '@tanstack/react-router'
import { ArrowLeft, FileText, Pencil, Plus } from 'lucide-react'

import { Button } from '#/components/ui/Button'
import { Alert } from '#/components/ui/Field'
import { Tabs } from '#/components/ui/Tabs'
import {
  usePatientFile,
  usePerson,
  useSetPatientStatus,
} from '#/features/patients/api'
import {
  AddDependantForm,
  ChangeMedicalAidForm,
  EditMembershipForm,
  EditPersonForm,
} from '#/features/patients/edit/EditForms'
import { friendlyError } from '#/features/patients/errors'
import { InvoiceTable } from '#/features/billing/InvoiceTable'
import { useAccountInvoices } from '#/features/billing/api'
import { AccountPayment } from '#/features/billing/corrections/InvoiceActions'
import { todayInSA } from '#/features/billing/statement'
import { useAccountDrafts, useCreateDraft } from '#/features/billing/draft/api'
import {
  FILE_MODE_HELP,
  FILE_MODE_LABELS,
  ID_TYPE_LABELS,
  SEX_LABELS,
  formatAddress,
  formatDate,
  fullName,
} from '#/features/patients/format'
import { ApiError, errorMessage } from '#/lib/api/errors'
import { useCan } from '#/lib/auth/session'
import type { ReactNode } from 'react'
import type { PatientFile, PersonDetails } from '#/features/patients/types'

const TABS = [
  { id: 'details', label: 'Details' },
  { id: 'medical-aid', label: 'Medical aid' },
  { id: 'visits', label: 'Visits' },
  { id: 'billing', label: 'Billing' },
] as const
type Tab = (typeof TABS)[number]['id']

interface FileSearch {
  patient?: string
  tab?: Tab
}

export const Route = createFileRoute('/_app/patients/file/$accountId')({
  validateSearch: (s: Record<string, unknown>): FileSearch => ({
    patient: typeof s.patient === 'string' ? s.patient : undefined,
    tab: TABS.some((t) => t.id === s.tab) ? (s.tab as Tab) : undefined,
  }),
  head: () => ({ meta: [{ title: 'Patient file · doctr8' }] }),
  component: PatientFilePage,
})

function PatientFilePage() {
  const { accountId } = Route.useParams()
  const search = Route.useSearch()
  const navigate = useNavigate({ from: Route.fullPath })
  const file = usePatientFile(accountId)

  if (file.isError) {
    const notFound = file.error instanceof ApiError && file.error.status === 404
    return (
      <div className="mx-auto flex max-w-5xl flex-col items-start gap-4">
        <BackLink />
        <Alert>
          {notFound
            ? "This file doesn't exist in this practice."
            : errorMessage(file.error)}
        </Alert>
        {!notFound && (
          <Button variant="secondary" onClick={() => void file.refetch()}>
            Try again
          </Button>
        )}
      </div>
    )
  }
  if (!file.data) {
    return <p className="py-12 text-center text-sm text-muted">Loading file…</p>
  }

  const f = file.data
  const selected =
    f.patients.find((p) => p.patient_id === search.patient) ??
    f.patients.find((p) => p.dependant_code === '00') ??
    f.patients.at(0)
  const tab: Tab = search.tab ?? 'details'
  const select = (next: FileSearch) =>
    void navigate({ search: (prev) => ({ ...prev, ...next }), replace: true })

  return (
    <div className="mx-auto flex max-w-5xl flex-col gap-6">
      <BackLink />

      <header className="flex flex-wrap items-baseline gap-x-4 gap-y-2">
        <h1 className="text-2xl font-semibold tracking-tight text-text">
          File <span className="tabular-nums text-accent">{f.account_no}</span>
        </h1>
        <span className="rounded border border-border-strong px-2 py-0.5 text-xs font-medium text-muted">
          {FILE_MODE_LABELS[f.file_mode]}
        </span>
        {f.status === 'closed' && (
          <span className="rounded border border-border-strong px-2 py-0.5 text-xs font-medium text-muted">
            Closed
          </span>
        )}
      </header>

      <div className="grid gap-4 md:grid-cols-2">
        <Panel title="Who pays">
          <p className="font-semibold text-text">{fullName(f.guarantor)}</p>
          <p className="mt-1 text-sm text-muted">
            Guarantor (the person the account is billed to)
          </p>
        </Panel>
        <Panel title="Medical aid">
          <FileMedicalAid file={f} />
        </Panel>
      </div>

      <Panel title="On this file">
        <p className="mb-3 text-sm text-muted">{FILE_MODE_HELP[f.file_mode]}</p>
        <ul className="flex flex-col gap-1" aria-label="Patients on this file">
          {f.patients.map((p) => {
            const active = p.patient_id === selected?.patient_id
            return (
              <li key={p.patient_id}>
                <button
                  type="button"
                  aria-current={active || undefined}
                  onClick={() => select({ patient: p.patient_id })}
                  className={`flex min-h-11 w-full items-center gap-3 rounded-lg px-3 text-left transition-colors hover:bg-raised ${
                    active ? 'bg-raised' : ''
                  }`}
                >
                  <span className="w-8 shrink-0 font-semibold tabular-nums text-muted">
                    {p.dependant_code}
                  </span>
                  <span className="flex-1 font-medium text-text">
                    {fullName(p)}
                  </span>
                  {p.dependant_code === '00' && f.medical_aid && (
                    <span className="text-xs text-subtle">Main member</span>
                  )}
                  {p.status === 'inactive' && (
                    <span className="rounded border border-border-strong px-1.5 py-0.5 text-xs text-muted">
                      Inactive
                    </span>
                  )}
                </button>
              </li>
            )
          })}
        </ul>
        <AddDependant file={f} onAdded={(id) => select({ patient: id })} />
      </Panel>

      {selected && (
        <section
          aria-labelledby="patient-heading"
          className="rounded-lg border border-border bg-panel p-5"
        >
          <div className="flex flex-wrap items-start justify-between gap-3">
            <h2
              id="patient-heading"
              className="text-lg font-semibold text-text"
            >
              {fullName(selected)}{' '}
              <span className="text-sm font-normal text-muted">
                · dependant {selected.dependant_code}
              </span>
            </h2>
            <StatusControl
              key={selected.patient_id}
              accountId={f.id}
              patient={selected}
            />
          </div>
          <div className="mt-4">
            <Tabs
              label="Patient"
              tabs={TABS}
              selected={tab}
              onSelect={(t) => select({ tab: t })}
            >
              {tab === 'details' && (
                <DetailsTab
                  key={selected.person_id}
                  personId={selected.person_id}
                  accountId={f.id}
                />
              )}
              {tab === 'medical-aid' && (
                <MedicalAidTab
                  file={f}
                  dependantCode={selected.dependant_code}
                />
              )}
              {tab === 'visits' && (
                <p className="py-6 text-sm text-muted">
                  Available once booking is set up.
                </p>
              )}
              {tab === 'billing' && <BillingTab accountId={f.id} />}
            </Tabs>
          </div>
        </section>
      )}
    </div>
  )
}

/** Billing is per file (account): invoices and the statement cover everyone on it. */
function BillingTab({ accountId }: { accountId: string }) {
  const canView = useCan('billing.view')
  if (!canView) {
    return (
      <p className="py-6 text-sm text-muted">
        You don't have access to billing in this practice.
      </p>
    )
  }
  return <AccountBilling accountId={accountId} />
}

function AccountBilling({ accountId }: { accountId: string }) {
  const invoices = useAccountInvoices(accountId)
  const canInvoice = useCan('billing.manage')
  const drafts = useAccountDrafts(accountId)
  const create = useCreateDraft()
  const navigate = useNavigate()
  const newInvoice = () =>
    create.mutate(accountId, {
      onSuccess: (d) =>
        void navigate({
          to: '/billing/drafts/$draftId',
          params: { draftId: d.id },
        }),
    })
  return (
    <div className="flex flex-col gap-4 py-2">
      <div className="flex flex-wrap items-center gap-3">
        {canInvoice && (
          <Button busy={create.isPending} onClick={newInvoice}>
            <Plus className="size-4" aria-hidden="true" />
            New invoice
          </Button>
        )}
        <Link
          to="/patients/statement/$accountId"
          params={{ accountId }}
          className="inline-flex min-h-11 items-center gap-2 rounded-lg border border-border bg-card px-4 text-sm font-semibold text-text hover:border-border-strong hover:bg-raised"
        >
          <FileText className="size-4" aria-hidden="true" />
          Open statement
        </Link>
      </div>
      {create.isError && <Alert>{friendlyError(create.error)}</Alert>}
      <AccountPayment accountId={accountId} today={todayInSA()} />
      {(drafts.data?.length ?? 0) > 0 && (
        <section
          aria-labelledby="drafts-heading"
          className="flex flex-col gap-2"
        >
          <h3 id="drafts-heading" className="text-sm font-semibold text-text">
            Drafts (not posted)
          </h3>
          <ul className="flex flex-col gap-1">
            {drafts.data?.map((d) => (
              <li key={d.id}>
                <Link
                  to="/billing/drafts/$draftId"
                  params={{ draftId: d.id }}
                  className="inline-flex min-h-11 items-center gap-2 rounded-lg text-sm text-text underline underline-offset-4"
                >
                  Draft saved{' '}
                  {new Intl.DateTimeFormat('en-ZA', {
                    dateStyle: 'medium',
                    timeStyle: 'short',
                    timeZone: 'Africa/Johannesburg',
                  }).format(new Date(d.updated_at))}
                </Link>
              </li>
            ))}
          </ul>
        </section>
      )}
      <h3 className="text-sm font-semibold text-text">Invoices on this file</h3>
      {invoices.isError ? (
        <Alert>{errorMessage(invoices.error)}</Alert>
      ) : !invoices.data ? (
        <p className="text-sm text-muted">Loading invoices…</p>
      ) : invoices.data.length === 0 ? (
        <p className="text-sm text-muted">No invoices on this file yet.</p>
      ) : (
        <InvoiceTable items={invoices.data} caption="Invoices on this file" />
      )}
    </div>
  )
}

function BackLink() {
  return (
    <Link
      to="/patients"
      className="inline-flex min-h-11 items-center gap-2 self-start rounded-lg pr-3 text-sm font-medium text-muted hover:text-text"
    >
      <ArrowLeft className="size-4" aria-hidden="true" />
      Patients
    </Link>
  )
}

function Panel({ title, children }: { title: string; children: ReactNode }) {
  return (
    <section className="rounded-lg border border-border bg-panel p-5">
      <h2 className="mb-3 text-xs font-semibold uppercase tracking-wide text-subtle">
        {title}
      </h2>
      {children}
    </section>
  )
}

function FileMedicalAid({ file }: { file: PatientFile }) {
  const main = useMainMemberName(file)
  if (!file.medical_aid) {
    return (
      <>
        <p className="font-semibold text-text">Private</p>
        <p className="mt-1 text-sm text-muted">
          No medical aid: the guarantor pays.
        </p>
      </>
    )
  }
  const m = file.medical_aid
  return (
    <dl className="grid grid-cols-[auto_1fr] gap-x-4 gap-y-1 text-sm">
      <dt className="text-muted">Scheme</dt>
      <dd className="font-semibold text-text">
        {m.scheme_name} · {m.plan_name}
      </dd>
      <dt className="text-muted">Number</dt>
      <dd className="font-mono text-text">{m.membership_number}</dd>
      <dt className="text-muted">Main member</dt>
      <dd className="text-text">{main ?? '…'}</dd>
    </dl>
  )
}

/** The main member's name: from the file if they're on it, else fetched. */
function useMainMemberName(file: PatientFile): string | null {
  const id = file.medical_aid?.main_member_person_id
  const onFile = file.patients.find((p) => p.person_id === id)
  const person = usePerson(onFile ? undefined : id)
  if (!id) return null
  if (onFile) return fullName(onFile)
  return person.data ? fullName(person.data) : null
}

function Field({ label, children }: { label: string; children: ReactNode }) {
  return (
    <div className="flex flex-col gap-0.5 sm:flex-row sm:gap-4">
      <dt className="w-44 shrink-0 text-sm text-muted">{label}</dt>
      <dd className="text-sm text-text">{children ?? '—'}</dd>
    </div>
  )
}

/**
 * Which form is open, a "saved" note, and focus back on the button that
 * opened the form once it closes (the button re-renders, so it's found by
 * name rather than kept).
 */
function useEditing<T extends string>() {
  const [editing, setEditing] = useState<T | null>(null)
  const [saved, setSaved] = useState<string | null>(null)
  const buttons = useRef(new Map<T, HTMLButtonElement | null>())
  const restore = useRef<T | null>(null)
  useEffect(() => {
    if (editing === null && restore.current) {
      buttons.current.get(restore.current)?.focus()
      restore.current = null
    }
  }, [editing])
  return {
    editing,
    saved,
    buttonRef: (what: T) => (el: HTMLButtonElement | null) => {
      buttons.current.set(what, el)
    },
    open: (what: T) => {
      restore.current = what
      setSaved(null)
      setEditing(what)
    },
    close: (message: string | null) => {
      setEditing(null)
      setSaved(message)
    },
  }
}

function Saved({ message }: { message: string | null }) {
  return (
    <p role="status" className="text-sm text-muted">
      {message}
    </p>
  )
}

function DetailsTab({
  personId,
  accountId,
}: {
  personId: string
  accountId: string
}) {
  const person = usePerson(personId)
  const canEdit = useCan('patients.manage')
  const e = useEditing<'person'>()
  if (person.isError) return <Alert>{errorMessage(person.error)}</Alert>
  if (!person.data)
    return <p className="text-sm text-muted">Loading details…</p>
  const p = person.data
  if (e.editing) {
    return (
      <EditPersonForm
        person={p}
        accountId={accountId}
        onDone={(ok) => e.close(ok ? 'Details saved.' : null)}
      />
    )
  }
  return (
    <div className="flex flex-col gap-4">
      {canEdit && (
        <div className="flex flex-wrap items-center gap-3">
          <Button
            variant="secondary"
            ref={e.buttonRef('person')}
            onClick={() => e.open('person')}
          >
            <Pencil className="size-4" aria-hidden="true" />
            Edit details
          </Button>
          <Saved message={e.saved} />
        </div>
      )}
      <PersonDetailsList p={p} />
    </div>
  )
}

function PersonDetailsList({ p }: { p: PersonDetails }) {
  return (
    <dl className="flex flex-col gap-3">
      <Field label="Name">{fullName(p)}</Field>
      {p.preferred_name && (
        <Field label="Preferred name">{p.preferred_name}</Field>
      )}
      <Field label="Date of birth">{formatDate(p.date_of_birth)}</Field>
      <Field label="Sex">{p.sex ? SEX_LABELS[p.sex] : null}</Field>
      <Field
        label={p.id_type ? ID_TYPE_LABELS[p.id_type] : 'Identity document'}
      >
        {p.id_number ? (
          <span className="font-mono">
            {p.id_number}
            {p.id_type === 'passport' && p.id_country
              ? ` (${p.id_country})`
              : ''}
          </span>
        ) : null}
      </Field>
      <Field label="Mobile">{p.mobile}</Field>
      <Field label="Home phone">{p.home_phone}</Field>
      <Field label="Email">{p.email}</Field>
      <Field label="Address">{formatAddress(p)}</Field>
    </dl>
  )
}

function MedicalAidTab({
  file,
  dependantCode,
}: {
  file: PatientFile
  dependantCode: string
}) {
  const main = useMainMemberName(file)
  const canEdit = useCan('accounts.manage')
  const e = useEditing<'membership' | 'change'>()

  if (e.editing === 'membership')
    return (
      <EditMembershipForm
        file={file}
        onDone={(ok) => e.close(ok ? 'Medical aid saved.' : null)}
      />
    )
  if (e.editing === 'change')
    return (
      <ChangeMedicalAidForm
        file={file}
        onDone={(ok) => e.close(ok ? 'Medical aid changed.' : null)}
      />
    )

  const actions = canEdit && (
    <div className="flex flex-wrap items-center gap-3">
      {file.medical_aid && (
        <Button
          variant="secondary"
          ref={e.buttonRef('membership')}
          onClick={() => e.open('membership')}
        >
          <Pencil className="size-4" aria-hidden="true" />
          Edit plan or number
        </Button>
      )}
      <Button
        variant="secondary"
        ref={e.buttonRef('change')}
        onClick={() => e.open('change')}
      >
        {file.medical_aid ? 'Change medical aid' : 'Add medical aid'}
      </Button>
      <Saved message={e.saved} />
    </div>
  )

  if (!file.medical_aid) {
    return (
      <div className="flex flex-col gap-4">
        {actions}
        <p className="text-sm text-muted">
          This file is private: there is no medical aid.
        </p>
      </div>
    )
  }
  const m = file.medical_aid
  return (
    <div className="flex flex-col gap-4">
      {actions}
      <dl className="flex flex-col gap-3">
        <Field label="Scheme">{m.scheme_name}</Field>
        <Field label="Plan">{m.plan_name}</Field>
        <Field label="Membership number">
          <span className="font-mono">{m.membership_number}</span>
        </Field>
        <Field label="Main member">{main}</Field>
        <Field label="Dependant code">
          <span className="tabular-nums">{dependantCode}</span>
        </Field>
      </dl>
    </div>
  )
}

/** Family files only: add a new or existing person as a dependant. */
function AddDependant({
  file,
  onAdded,
}: {
  file: PatientFile
  onAdded: (patientId: string) => void
}) {
  const canAdd = useCan('patients.manage')
  const e = useEditing<'add'>()
  if (!canAdd || file.file_mode !== 'family_per_file' || file.status !== 'open')
    return null
  if (e.editing) {
    return (
      <div className="mt-4 border-t border-border pt-4">
        <AddDependantForm
          file={file}
          onDone={(added) => {
            e.close(added ? 'Dependant added.' : null)
            if (added) onAdded(added)
          }}
        />
      </div>
    )
  }
  return (
    <div className="mt-3 flex flex-wrap items-center gap-3">
      <Button
        variant="secondary"
        ref={e.buttonRef('add')}
        onClick={() => e.open('add')}
      >
        <Plus className="size-4" aria-hidden="true" />
        Add dependant
      </Button>
      <Saved message={e.saved} />
    </div>
  )
}

/** Active/inactive, with a confirmation step before marking inactive. */
function StatusControl({
  accountId,
  patient,
}: {
  accountId: string
  patient: PatientFile['patients'][number]
}) {
  const canEdit = useCan('patients.manage')
  const set = useSetPatientStatus(accountId)
  const [confirming, setConfirming] = useState(false)
  if (!canEdit) return null

  const change = (status: 'active' | 'inactive') =>
    set.mutate(
      { patientId: patient.patient_id, status },
      { onSuccess: () => setConfirming(false) },
    )

  if (patient.status === 'inactive') {
    return (
      <div className="flex flex-col items-end gap-2">
        <Button
          variant="secondary"
          busy={set.isPending}
          onClick={() => change('active')}
        >
          Mark active
        </Button>
        {set.error ? <Alert>{friendlyError(set.error)}</Alert> : null}
      </div>
    )
  }
  if (!confirming) {
    return (
      <Button variant="secondary" onClick={() => setConfirming(true)}>
        Mark inactive
      </Button>
    )
  }
  return (
    <div
      role="group"
      aria-label="Confirm marking inactive"
      className="flex w-full flex-col gap-3 rounded-lg border border-border bg-card p-4 sm:max-w-md"
    >
      <p className="text-sm text-text">
        Mark {fullName(patient)} inactive? They stay on this file and can be
        found with the Inactive filter. You can mark them active again.
      </p>
      <div className="flex flex-wrap gap-3">
        <Button
          variant="danger"
          busy={set.isPending}
          onClick={() => change('inactive')}
          autoFocus
        >
          Mark inactive
        </Button>
        <Button variant="secondary" onClick={() => setConfirming(false)}>
          Cancel
        </Button>
      </div>
      {set.error ? <Alert>{friendlyError(set.error)}</Alert> : null}
    </div>
  )
}
