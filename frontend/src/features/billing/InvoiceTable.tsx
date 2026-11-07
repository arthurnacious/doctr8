/**
 * A list of invoices. The invoice number is a link, so keyboard users Tab
 * through invoices and Enter opens one. Amounts are aligned tabular figures.
 */
import { Link } from '@tanstack/react-router'

import { formatDate } from '../patients/format'
import { claimLabel } from './invoice'
import { formatRand } from './money'
import { num, td, th } from './tables'
import type { InvoiceListItem } from './types'

export function InvoiceTable({
  items,
  caption,
  showAccount = false,
}: {
  items: ReadonlyArray<InvoiceListItem>
  caption: string
  /** Practice-wide lists show the file number too. */
  showAccount?: boolean
}) {
  return (
    <div className="overflow-x-auto">
      <table className="w-full min-w-[40rem] text-sm">
        <caption className="sr-only">{caption}</caption>
        <thead className="border-b border-border">
          <tr>
            <th scope="col" className={th}>
              Invoice
            </th>
            <th scope="col" className={th}>
              Date
            </th>
            {showAccount && (
              <th scope="col" className={`${th} hidden md:table-cell`}>
                File
              </th>
            )}
            <th scope="col" className={th}>
              Patient
            </th>
            <th scope="col" className={`${th} hidden lg:table-cell`}>
              Doctor
            </th>
            <th scope="col" className={`${th} text-right`}>
              Total
            </th>
            <th scope="col" className={`${th} text-right`}>
              Medical aid owes
            </th>
            <th scope="col" className={`${th} text-right`}>
              Patient owes
            </th>
            <th scope="col" className={`${th} hidden sm:table-cell`}>
              Claim
            </th>
          </tr>
        </thead>
        <tbody>
          {items.map((i) => (
            <tr
              key={i.id}
              className="border-b border-border last:border-0 hover:bg-raised"
            >
              <td className={td}>
                <Link
                  to="/billing/invoices/$invoiceId"
                  params={{ invoiceId: i.id }}
                  className="inline-flex min-h-11 items-center font-semibold tabular-nums text-text underline-offset-4 hover:underline"
                >
                  {i.invoice_no}
                </Link>
              </td>
              <td
                className={`${td} whitespace-nowrap py-3 tabular-nums text-muted`}
              >
                {formatDate(i.invoice_date)}
              </td>
              {showAccount && (
                <td
                  className={`${td} hidden py-3 tabular-nums text-muted md:table-cell`}
                >
                  {i.account_no}
                </td>
              )}
              <td className={`${td} py-3 text-text`}>
                {i.patient.first_name} {i.patient.last_name}{' '}
                <span className="tabular-nums text-muted">
                  ({i.patient.dependant_code})
                </span>
              </td>
              <td className={`${td} hidden py-3 text-muted lg:table-cell`}>
                {i.treating_provider}
              </td>
              <td className={`${td} ${num} py-3 text-text`}>
                {formatRand(i.total_cents - i.credited_cents)}
              </td>
              <td className={`${td} ${num} py-3 text-text`}>
                {formatRand(i.medical_aid_due_cents)}
              </td>
              <td className={`${td} ${num} py-3 text-text`}>
                {formatRand(i.patient_due_cents)}
              </td>
              <td className={`${td} hidden py-3 text-muted sm:table-cell`}>
                {claimLabel(i.claim_status)}
              </td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  )
}
