/**
 * Statement controls: the "as of" date, a patient filter, and whether
 * reversed journals are shown. State lives in the URL (ids and dates only,
 * no patient details) so a statement view can be shared or reloaded.
 */
import { useId, useState } from 'react'

import { Checkbox, TextField } from '#/components/ui/Field'
import { fullName } from '../patients/format'
import { asOfError } from './statement'
import type { StatementPatient } from './types'

export interface ControlValues {
  asOf: string
  patientId: string | undefined
  showReversed: boolean
}

export function StatementControls({
  values,
  today,
  patients,
  onChange,
}: {
  values: ControlValues
  today: string
  patients: ReadonlyArray<StatementPatient>
  onChange: (next: Partial<ControlValues>) => void
}) {
  // The input may hold an unfinished date; only valid ones are applied.
  const [draft, setDraft] = useState(values.asOf)
  const [shown, setShown] = useState(values.asOf)
  if (shown !== values.asOf) {
    setShown(values.asOf)
    setDraft(values.asOf)
  }
  const error = draft ? asOfError(draft, today) : null
  const selectId = useId()

  return (
    <div
      role="group"
      aria-label="Statement options"
      className="no-print grid gap-4 rounded-lg border border-border bg-panel p-4 sm:grid-cols-[minmax(0,14rem)_minmax(0,16rem)_1fr] sm:items-start"
    >
      <TextField
        label="Statement as of"
        type="date"
        max={today}
        value={draft}
        error={error ?? undefined}
        onChange={(e) => {
          const v = e.target.value
          setDraft(v)
          if (!v) onChange({ asOf: today })
          else if (!asOfError(v, today)) onChange({ asOf: v })
        }}
      />
      {patients.length > 1 && (
        <div>
          <label
            htmlFor={selectId}
            className="mb-1.5 block text-sm font-medium text-text"
          >
            Patient
          </label>
          <select
            id={selectId}
            value={values.patientId ?? ''}
            onChange={(e) =>
              onChange({ patientId: e.target.value || undefined })
            }
            className="block min-h-11 w-full rounded-lg border border-border bg-card px-3 text-base text-text hover:border-border-strong"
          >
            <option value="">Everyone on the file</option>
            {patients.map((p) => (
              <option key={p.patient_id} value={p.patient_id}>
                {p.dependant_code} · {fullName(p)}
              </option>
            ))}
          </select>
        </div>
      )}
      <div className="sm:pt-6">
        <Checkbox
          label="Hide reversed journals"
          hint="Journals posted and later reversed cancel out, so they're left off."
          checked={!values.showReversed}
          onChange={(e) => onChange({ showReversed: !e.target.checked })}
        />
      </div>
    </div>
  )
}
