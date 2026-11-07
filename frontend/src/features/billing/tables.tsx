/** Table pieces shared by the statement and the invoice. */
import { formatDate } from '../patients/format'
import { formatRandOrBlank } from './money'
import type { StatementLine } from './types'

/** Number cells: right-aligned tabular figures that line up. */
export const num = 'text-right tabular-nums whitespace-nowrap'
export const th = 'px-2 py-1.5 text-left text-xs font-semibold text-subtle'
export const td = 'px-2 py-1.5 align-top'

export function TransactionsTable({
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
