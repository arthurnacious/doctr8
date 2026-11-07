import {
  createFileRoute,
  stripSearchParams,
  useNavigate,
} from '@tanstack/react-router'
import { Search } from 'lucide-react'

import { Button } from '#/components/ui/Button'
import { Alert, TextField } from '#/components/ui/Field'
import { usePatientList } from '#/features/patients/api'
import { pageCount, pageSummary } from '#/features/patients/format'
import { useListQuery } from '#/features/patients/listQuery'
import { PatientTable } from '#/features/patients/PatientTable'
import { errorMessage } from '#/lib/api/errors'
import { useDebouncedValue } from '#/lib/hooks/useDebouncedValue'
import type { PatientListItem, StatusFilter } from '#/features/patients/types'

interface ListSearch {
  status: StatusFilter
  page: number
}

const DEFAULT_SEARCH: ListSearch = { status: 'active', page: 1 }

export const Route = createFileRoute('/_app/patients/')({
  // Only the filter and page live in the URL; the search text may be a
  // name or ID number, so it stays in memory (features/patients/listQuery).
  validateSearch: (s: Record<string, unknown>): ListSearch => ({
    status: s.status === 'inactive' || s.status === 'all' ? s.status : 'active',
    page: Math.max(1, Math.floor(Number(s.page)) || 1),
  }),
  // Keep URLs clean: defaults aren't written to the address bar.
  search: { middlewares: [stripSearchParams(DEFAULT_SEARCH)] },
  head: () => ({ meta: [{ title: 'Patients · doctr8' }] }),
  component: PatientList,
})

const STATUS_OPTIONS: Array<{ value: StatusFilter; label: string }> = [
  { value: 'active', label: 'Active' },
  { value: 'inactive', label: 'Inactive' },
  { value: 'all', label: 'All' },
]

function PatientList() {
  const { status, page } = Route.useSearch()
  const navigate = useNavigate({ from: Route.fullPath })
  const [query, setQuery] = useListQuery()
  const debounced = useDebouncedValue(query.trim(), 300)
  const list = usePatientList({ query: debounced, status, page })

  const setSearch = (next: Partial<ListSearch>) =>
    void navigate({ search: (prev) => ({ ...prev, ...next }), replace: true })

  const openFile = (row: PatientListItem) =>
    void navigate({
      to: '/patients/file/$accountId',
      params: { accountId: row.account_id },
      search: { patient: row.patient_id },
    })

  const data = list.data
  const pages = data ? pageCount(data.page_size, data.total) : 1

  return (
    <div className="mx-auto flex max-w-6xl flex-col gap-6">
      <header className="flex flex-wrap items-end justify-between gap-4">
        <h1 className="text-2xl font-semibold tracking-tight text-text">
          Patients
        </h1>
      </header>

      <div className="flex flex-col gap-4 md:flex-row md:items-end">
        <TextField
          className="flex-1"
          label="Search patients"
          hint="Name, file number, ID or passport number, or membership number"
          type="search"
          value={query}
          autoComplete="off"
          spellCheck={false}
          onChange={(e) => {
            setQuery(e.target.value)
            if (page !== 1) setSearch({ page: 1 })
          }}
          trailing={
            <Search className="mr-3 size-4 text-subtle" aria-hidden="true" />
          }
        />
        <fieldset className="md:mb-7">
          <legend className="mb-1.5 text-sm font-medium text-text">Show</legend>
          <div className="flex rounded-lg border border-border bg-card p-0.5">
            {STATUS_OPTIONS.map((o) => (
              <label
                key={o.value}
                className="flex min-h-10 cursor-pointer items-center rounded-md px-3 text-sm text-muted has-[:checked]:bg-raised has-[:checked]:text-text has-[:focus-visible]:outline has-[:focus-visible]:outline-2 has-[:focus-visible]:outline-focus"
              >
                <input
                  type="radio"
                  name="status"
                  value={o.value}
                  checked={status === o.value}
                  onChange={() => setSearch({ status: o.value, page: 1 })}
                  className="sr-only"
                />
                {o.label}
              </label>
            ))}
          </div>
        </fieldset>
      </div>

      {list.isError ? (
        <div className="flex flex-col items-start gap-3">
          <Alert>{errorMessage(list.error)}</Alert>
          <Button variant="secondary" onClick={() => void list.refetch()}>
            Try again
          </Button>
        </div>
      ) : !data ? (
        <p className="py-12 text-center text-sm text-muted">
          Loading patients…
        </p>
      ) : data.total === 0 ? (
        <EmptyList query={debounced} status={status} />
      ) : (
        <>
          <div className={list.isPlaceholderData ? 'opacity-60' : undefined}>
            <PatientTable
              rows={data.items}
              onOpen={openFile}
              caption="Patients"
            />
          </div>
          <nav
            aria-label="Pages"
            className="flex flex-wrap items-center justify-between gap-3"
          >
            <p role="status" className="text-sm text-muted">
              {pageSummary(data.page, data.page_size, data.total)}
            </p>
            <div className="flex gap-2">
              <Button
                variant="secondary"
                disabled={page <= 1}
                onClick={() => setSearch({ page: page - 1 })}
              >
                Previous
              </Button>
              <Button
                variant="secondary"
                disabled={page >= pages}
                onClick={() => setSearch({ page: page + 1 })}
              >
                Next
              </Button>
            </div>
          </nav>
        </>
      )}
    </div>
  )
}

function EmptyList({ query, status }: { query: string; status: StatusFilter }) {
  if (query) {
    return (
      <div className="rounded-lg border border-border bg-panel px-6 py-10 text-center">
        <p className="font-semibold text-text">No patients match that search</p>
        <p className="mt-1 text-sm text-muted">
          Check the spelling, or search by file number, ID number or membership
          number instead.
        </p>
      </div>
    )
  }
  if (status === 'inactive') {
    return (
      <div className="rounded-lg border border-border bg-panel px-6 py-10 text-center">
        <p className="font-semibold text-text">No inactive patients</p>
      </div>
    )
  }
  return (
    <div className="rounded-lg border border-border bg-panel px-6 py-10">
      <p className="text-lg font-semibold text-text">No patients yet</p>
      <p className="mt-2 max-w-prose text-sm text-muted">
        Patients live on files. A file is the billing account: who pays, the
        medical aid if there is one, and everyone on it with their dependant
        code. Create the first file with New file: you'll enter the patient's
        details, who pays, and their medical aid or private billing.
      </p>
      <p className="mt-2 max-w-prose text-sm text-subtle">
        You'll need permission to manage patients. Ask a practice owner if you
        don't see the option.
      </p>
    </div>
  )
}
