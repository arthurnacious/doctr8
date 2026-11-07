import { Link, createFileRoute, useNavigate } from '@tanstack/react-router'
import { ArrowLeft, Printer } from 'lucide-react'

import { Button } from '#/components/ui/Button'
import { Alert } from '#/components/ui/Field'
import { PrintPageSetup } from '#/features/billing/PrintPageSetup'
import { StatementControls } from '#/features/billing/StatementControls'
import { StatementView } from '#/features/billing/StatementView'
import { useStatement } from '#/features/billing/api'
import {
  asOfError,
  reversedJournalIds,
  todayInSA,
} from '#/features/billing/statement'
import type { ControlValues } from '#/features/billing/StatementControls'
import { ApiError, errorMessage } from '#/lib/api/errors'
import { requirePermission } from '#/lib/auth/guards'
import { useFollowAccount } from '#/lib/ws/useRealtime'

/** Ids and dates only: no patient details in the URL. */
interface StatementSearch {
  /** yyyy-mm-dd; omitted = today. */
  as_of?: string
  /** One patient on the file; omitted = everyone. */
  patient?: string
  /** "shown" to include reversed journals (hidden by default). */
  reversed?: 'shown'
}

export const Route = createFileRoute('/_app/patients/statement/$accountId')({
  validateSearch: (s: Record<string, unknown>): StatementSearch => ({
    as_of:
      typeof s.as_of === 'string' && !asOfError(s.as_of, todayInSA())
        ? s.as_of
        : undefined,
    patient: typeof s.patient === 'string' ? s.patient : undefined,
    reversed: s.reversed === 'shown' ? 'shown' : undefined,
  }),
  beforeLoad: () => requirePermission({ anyOf: ['billing.view'] }),
  head: () => ({ meta: [{ title: 'Statement · doctr8' }] }),
  component: StatementPage,
})

function StatementPage() {
  const { accountId } = Route.useParams()
  const search = Route.useSearch()
  const navigate = useNavigate({ from: Route.fullPath })
  // Payments, remittances and claims on this account refresh the statement.
  useFollowAccount(accountId)
  const today = todayInSA()
  const statement = useStatement(accountId, { asOf: search.as_of })
  const values: ControlValues = {
    asOf: search.as_of ?? today,
    patientId: search.patient,
    showReversed: search.reversed === 'shown',
  }
  const change = (next: Partial<ControlValues>) => {
    const v = { ...values, ...next }
    void navigate({
      search: {
        as_of: v.asOf === today ? undefined : v.asOf,
        patient: v.patientId,
        reversed: v.showReversed ? 'shown' : undefined,
      },
      replace: true,
    })
  }

  return (
    <div className="statement-page mx-auto flex max-w-5xl flex-col gap-6">
      <Link
        to="/patients/file/$accountId"
        params={{ accountId }}
        search={{ tab: 'billing' }}
        className="no-print inline-flex min-h-11 items-center gap-2 self-start rounded-lg pr-3 text-sm font-medium text-muted hover:text-text"
      >
        <ArrowLeft className="size-4" aria-hidden="true" />
        Back to the file
      </Link>
      <div className="flex flex-wrap items-center justify-between gap-3">
        <h1 className="text-2xl font-semibold tracking-tight text-text">
          Statement
          {statement.data && (
            <>
              {' '}
              <span className="statement-key tabular-nums text-accent">
                {statement.data.account.account_no}
              </span>
            </>
          )}
        </h1>
        {statement.data && (
          <Button
            variant="secondary"
            className="no-print"
            onClick={() => window.print()}
          >
            <Printer className="size-4" aria-hidden="true" />
            Print or save as PDF
          </Button>
        )}
      </div>
      {statement.data && (
        <PrintPageSetup
          practice={statement.data.practice.name}
          accountNo={statement.data.account.account_no}
        />
      )}

      {statement.isError ? (
        <div className="flex flex-col items-start gap-4">
          <Alert>
            {statement.error instanceof ApiError &&
            statement.error.status === 404
              ? "This account doesn't exist in this practice."
              : errorMessage(statement.error)}
          </Alert>
          <Button variant="secondary" onClick={() => void statement.refetch()}>
            Try again
          </Button>
        </div>
      ) : !statement.data ? (
        <p className="py-12 text-center text-sm text-muted">
          Loading statement…
        </p>
      ) : (
        <>
          <StatementControls
            values={values}
            today={today}
            patients={statement.data.patients}
            onChange={change}
          />
          {statement.isPlaceholderData && (
            <p role="status" className="text-sm text-muted">
              Updating…
            </p>
          )}
          <StatementView
            statement={statement.data}
            hidden={
              values.showReversed
                ? new Set<string>()
                : reversedJournalIds(statement.data)
            }
            patientId={
              statement.data.patients.some(
                (p) => p.patient_id === values.patientId,
              )
                ? values.patientId
                : undefined
            }
          />
        </>
      )}
    </div>
  )
}
