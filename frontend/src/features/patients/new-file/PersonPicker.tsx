import { useId, useState } from 'react'

import { Button } from '#/components/ui/Button'
import { TextField } from '#/components/ui/Field'
import { useDebouncedValue } from '#/lib/hooks/useDebouncedValue'
import { usePersonSearch } from '../api'
import { formatDate, fullName } from '../format'
import type { ExistingPerson } from './model'

/** Pick someone already in the practice (by name, ID or file number). */
export function PersonPicker({
  label,
  value,
  onChange,
  error,
}: {
  label: string
  value: ExistingPerson | null
  onChange: (p: ExistingPerson | null) => void
  error?: string
}) {
  const [query, setQuery] = useState('')
  const debounced = useDebouncedValue(query, 250)
  const results = usePersonSearch(debounced)
  const listId = useId()

  if (value) {
    return (
      <div className="flex flex-wrap items-center justify-between gap-3 rounded-lg border border-border bg-card px-4 py-3">
        <div>
          <p className="text-xs text-subtle">{label}</p>
          <p className="font-semibold text-text">{value.label}</p>
        </div>
        <Button variant="ghost" onClick={() => onChange(null)}>
          Choose someone else
        </Button>
      </div>
    )
  }

  const items = results.data ?? []
  return (
    <div className="flex flex-col gap-2">
      <TextField
        label={label}
        hint="Search by name, ID number or file number"
        type="search"
        value={query}
        autoComplete="off"
        spellCheck={false}
        error={error}
        aria-controls={listId}
        onChange={(e) => setQuery(e.target.value)}
      />
      <ul
        id={listId}
        aria-label="Matching people"
        className="flex flex-col gap-1"
      >
        {debounced.trim().length >= 2 &&
          results.isSuccess &&
          items.length === 0 && (
            <li className="px-1 text-sm text-muted">
              Nobody matches. Enter them as someone new instead.
            </li>
          )}
        {items.map((p) => (
          <li key={p.id}>
            <button
              type="button"
              onClick={() => onChange({ id: p.id, label: fullName(p) })}
              className="flex min-h-11 w-full items-center justify-between gap-3 rounded-lg border border-border bg-card px-3 text-left text-sm hover:border-border-strong hover:bg-raised"
            >
              <span className="font-medium text-text">{fullName(p)}</span>
              <span className="text-muted">{formatDate(p.date_of_birth)}</span>
            </button>
          </li>
        ))}
      </ul>
    </div>
  )
}
