import { Link, createFileRoute, useNavigate } from '@tanstack/react-router'
import { ArrowLeft } from 'lucide-react'

import { Button } from '#/components/ui/Button'
import { Alert } from '#/components/ui/Field'
import { Tabs } from '#/components/ui/Tabs'
import { usePatientFile, usePerson } from '#/features/patients/api'
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
import type { ReactNode } from 'react'
import type { PatientFile } from '#/features/patients/types'

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
      </Panel>

      {selected && (
        <section
          aria-labelledby="patient-heading"
          className="rounded-lg border border-border bg-panel p-5"
        >
          <h2 id="patient-heading" className="text-lg font-semibold text-text">
            {fullName(selected)}{' '}
            <span className="text-sm font-normal text-muted">
              · dependant {selected.dependant_code}
            </span>
          </h2>
          <div className="mt-4">
            <Tabs
              label="Patient"
              tabs={TABS}
              selected={tab}
              onSelect={(t) => select({ tab: t })}
            >
              {tab === 'details' && (
                <DetailsTab personId={selected.person_id} />
              )}
              {tab === 'medical-aid' && (
                <MedicalAidTab
                  file={f}
                  dependantCode={selected.dependant_code}
                />
              )}
              {(tab === 'visits' || tab === 'billing') && (
                <p className="py-6 text-sm text-muted">
                  Available once booking and billing are set up.
                </p>
              )}
            </Tabs>
          </div>
        </section>
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

function DetailsTab({ personId }: { personId: string }) {
  const person = usePerson(personId)
  if (person.isError) return <Alert>{errorMessage(person.error)}</Alert>
  if (!person.data)
    return <p className="text-sm text-muted">Loading details…</p>
  const p = person.data
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
  if (!file.medical_aid) {
    return (
      <p className="text-sm text-muted">
        This file is private: there is no medical aid.
      </p>
    )
  }
  const m = file.medical_aid
  return (
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
  )
}
