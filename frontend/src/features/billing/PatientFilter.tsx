/**
 * Pick one patient to filter invoices by. Searches the patient list (POST,
 * so names stay out of URLs); only the chosen patient's id goes in the URL.
 */
import { useId, useState } from 'react'
import { useQuery } from '@tanstack/react-query'
import { X } from 'lucide-react'

import { Button } from '#/components/ui/Button'
import { TextField } from '#/components/ui/Field'
import { useDebouncedValue } from '#/lib/hooks/useDebouncedValue'
import { searchPatients } from '../patients/api'
import { listName } from '../patients/format'

export function PatientFilter({
  patientId,
  label,
  onChange,
}: {
  patientId: string | undefined
  /** Name of the chosen patient, when known. */
  label: string | undefined
  onChange: (patientId: string | undefined, label?: string) => void
}) {
  const [query, setQuery] = useState('')
  const q = useDebouncedValue(query.trim(), 250)
  const results = useQuery({
    queryKey: ['patients', 'list', { query: q, status: 'all', page: 1 }],
    queryFn: () => searchPatients({ query: q, status: 'all', page: 1 }),
    enabled: q.length >= 2,
  })
  const listId = useId()

  if (patientId) {
    return (
      <div>
        <p className="mb-1.5 text-sm font-medium text-text">Patient</p>
        <div className="flex min-h-11 items-center justify-between gap-2 rounded-lg border border-border bg-card pl-3">
          <span className="truncate text-sm text-text">
            {label ?? 'Chosen patient'}
          </span>
          <Button
            variant="ghost"
            aria-label="Clear the patient filter"
            onClick={() => onChange(undefined)}
          >
            <X className="size-4" aria-hidden="true" />
          </Button>
        </div>
      </div>
    )
  }

  const matches = q.length >= 2 ? (results.data?.items.slice(0, 8) ?? []) : []
  return (
    <div className="relative">
      <TextField
        label="Patient"
        placeholder="Name, file or ID number"
        autoComplete="off"
        value={query}
        onChange={(e) => setQuery(e.target.value)}
        aria-controls={listId}
      />
      {matches.length > 0 && (
        <ul
          id={listId}
          aria-label="Matching patients"
          className="absolute z-10 mt-1 w-full overflow-hidden rounded-lg border border-border-strong bg-card"
        >
          {matches.map((m) => (
            <li key={m.patient_id}>
              <button
                type="button"
                className="flex min-h-11 w-full items-center justify-between gap-2 px-3 text-left text-sm text-text hover:bg-raised focus:bg-raised"
                onClick={() => {
                  setQuery('')
                  onChange(m.patient_id, `${listName(m)} (${m.dependant_code})`)
                }}
              >
                <span>
                  {listName(m)}{' '}
                  <span className="text-muted">({m.dependant_code})</span>
                </span>
                <span className="tabular-nums text-muted">
                  File {m.account_no}
                </span>
              </button>
            </li>
          ))}
        </ul>
      )}
    </div>
  )
}
