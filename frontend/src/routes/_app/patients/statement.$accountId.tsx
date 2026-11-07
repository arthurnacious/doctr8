import { Link, createFileRoute } from '@tanstack/react-router'
import { ArrowLeft } from 'lucide-react'

import { Button } from '#/components/ui/Button'
import { Alert } from '#/components/ui/Field'
import { StatementView } from '#/features/billing/StatementView'
import { useStatement } from '#/features/billing/api'
import { reversedJournalIds } from '#/features/billing/statement'
import { ApiError, errorMessage } from '#/lib/api/errors'
import { requirePermission } from '#/lib/auth/guards'
import { useFollowAccount } from '#/lib/ws/useRealtime'

export const Route = createFileRoute('/_app/patients/statement/$accountId')({
  beforeLoad: () => requirePermission({ anyOf: ['billing.view'] }),
  head: () => ({ meta: [{ title: 'Statement · doctr8' }] }),
  component: StatementPage,
})

function StatementPage() {
  const { accountId } = Route.useParams()
  // Payments, remittances and claims on this account refresh the statement.
  useFollowAccount(accountId)
  const statement = useStatement(accountId, {})

  return (
    <div className="mx-auto flex max-w-5xl flex-col gap-6">
      <Link
        to="/patients/file/$accountId"
        params={{ accountId }}
        search={{ tab: 'billing' }}
        className="no-print inline-flex min-h-11 items-center gap-2 self-start rounded-lg pr-3 text-sm font-medium text-muted hover:text-text"
      >
        <ArrowLeft className="size-4" aria-hidden="true" />
        Back to the file
      </Link>
      <h1 className="text-2xl font-semibold tracking-tight text-text">
        Statement
        {statement.data && (
          <>
            {' '}
            <span className="tabular-nums text-accent">
              {statement.data.account.account_no}
            </span>
          </>
        )}
      </h1>

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
        <StatementView
          statement={statement.data}
          hidden={reversedJournalIds(statement.data)}
        />
      )}
    </div>
  )
}
