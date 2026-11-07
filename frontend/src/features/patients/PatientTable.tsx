import { useRef, useState } from 'react'

import { nextRowIndex } from './keyboard'
import { formatDate, listName, medicalAidLabel } from './format'
import type { PatientListItem } from './types'

interface Props {
  rows: ReadonlyArray<PatientListItem>
  onOpen: (row: PatientListItem) => void
  /** Accessible description, e.g. "Patients, 25 of 132". */
  caption: string
}

/**
 * The patient table. One row is in the tab order at a time: Up/Down (and
 * Home/End) move between rows, Enter opens the file. Clicking a row opens
 * it too.
 */
export function PatientTable({ rows, onOpen, caption }: Props) {
  const [active, setActive] = useState(0)
  const rowRefs = useRef<Array<HTMLTableRowElement | null>>([])
  const current = Math.min(active, Math.max(0, rows.length - 1))

  function onKeyDown(
    e: React.KeyboardEvent<HTMLTableRowElement>,
    index: number,
  ) {
    if (e.key === 'Enter') {
      e.preventDefault()
      onOpen(rows[index])
      return
    }
    const next = nextRowIndex(e.key, index, rows.length)
    if (next === null) return
    e.preventDefault()
    setActive(next)
    rowRefs.current[next]?.focus()
  }

  return (
    <div className="overflow-x-auto rounded-lg border border-border">
      <table className="w-full border-collapse text-left text-sm">
        <caption className="sr-only">{caption}</caption>
        <thead className="bg-panel text-xs font-semibold text-muted">
          <tr>
            <th scope="col" className="px-4 py-3">
              File
            </th>
            <th scope="col" className="px-4 py-3">
              Name
            </th>
            <th scope="col" className="px-4 py-3">
              Dep.
            </th>
            <th scope="col" className="hidden px-4 py-3 sm:table-cell">
              Date of birth
            </th>
            <th scope="col" className="hidden px-4 py-3 lg:table-cell">
              ID number
            </th>
            <th scope="col" className="px-4 py-3">
              Medical aid
            </th>
            <th scope="col" className="hidden px-4 py-3 md:table-cell">
              Main member
            </th>
          </tr>
        </thead>
        <tbody>
          {rows.map((r, i) => (
            <tr
              key={r.patient_id}
              ref={(el) => {
                rowRefs.current[i] = el
              }}
              tabIndex={i === current ? 0 : -1}
              onKeyDown={(e) => onKeyDown(e, i)}
              onFocus={() => setActive(i)}
              onClick={() => onOpen(r)}
              aria-label={`Open file ${r.account_no} for ${listName(r)}`}
              className="h-12 cursor-pointer border-t border-border bg-page text-text transition-colors hover:bg-card focus-visible:bg-card focus-visible:shadow-[inset_0_0_0_2px_var(--color-focus)] focus-visible:outline-none"
            >
              <td className="px-4 font-semibold tabular-nums">
                {r.account_no}
              </td>
              <td className="px-4">
                <span className="font-medium">{listName(r)}</span>
                {r.status === 'inactive' && (
                  <span className="ml-2 rounded border border-border-strong px-1.5 py-0.5 text-xs text-muted">
                    Inactive
                  </span>
                )}
              </td>
              <td className="px-4 tabular-nums text-muted">
                {r.dependant_code}
              </td>
              <td className="hidden px-4 tabular-nums text-muted sm:table-cell">
                {formatDate(r.date_of_birth)}
              </td>
              <td className="hidden px-4 font-mono text-xs text-muted lg:table-cell">
                {r.id_number_masked ?? '—'}
              </td>
              <td className="px-4 text-muted">
                {medicalAidLabel(r.scheme_name, r.plan_name)}
              </td>
              <td className="hidden px-4 text-muted md:table-cell">
                {r.main_member ? listName(r.main_member) : '—'}
              </td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  )
}
