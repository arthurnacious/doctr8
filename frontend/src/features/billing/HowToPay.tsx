/** The practice's banking details, printed on statements and invoices. */
import type { Statement } from './types'

type Practice = Statement['practice']

export function hasBankDetails(p: Practice): boolean {
  return Boolean(p.bank_name && p.bank_account_number)
}

export function HowToPay({
  practice: p,
  accountNo,
  labelClass,
  document,
}: {
  practice: Practice
  accountNo: number
  /** Heading class, so each document's print styles apply. */
  labelClass: string
  /** "statement" or "invoice", for the missing-details note. */
  document: string
}) {
  const bank = hasBankDetails(p)
  return (
    <div>
      <h2
        className={`${labelClass} text-xs font-semibold uppercase tracking-wide text-subtle`}
      >
        How to pay
      </h2>
      {bank ? (
        <>
          <dl className="mt-2 grid grid-cols-[auto_1fr] gap-x-4 gap-y-1 text-sm">
            <dt className="text-muted">Bank</dt>
            <dd className="text-text">{p.bank_name}</dd>
            {p.bank_account_name && (
              <>
                <dt className="text-muted">Account name</dt>
                <dd className="text-text">{p.bank_account_name}</dd>
              </>
            )}
            <dt className="text-muted">Account number</dt>
            <dd className="font-mono text-text">{p.bank_account_number}</dd>
            {p.bank_branch_code && (
              <>
                <dt className="text-muted">Branch code</dt>
                <dd className="font-mono text-text">{p.bank_branch_code}</dd>
              </>
            )}
            <dt className="text-muted">Reference</dt>
            <dd className="font-semibold tabular-nums text-text">
              {accountNo}
            </dd>
          </dl>
          <p className="mt-2 text-sm text-muted">
            Please use your account number as the payment reference.
          </p>
        </>
      ) : (
        <p className="no-print mt-2 text-sm text-muted">
          The practice&rsquo;s banking details aren&rsquo;t set up yet, so
          they&rsquo;re left off the printed {document}.
        </p>
      )}
    </div>
  )
}
