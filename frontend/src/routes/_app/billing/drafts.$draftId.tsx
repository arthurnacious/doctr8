import { Link, createFileRoute } from '@tanstack/react-router'
import { ArrowLeft } from 'lucide-react'

import { Button } from '#/components/ui/Button'
import { Alert } from '#/components/ui/Field'
import { useProviders, useServiceCentres } from '#/features/billing/api'
import { DraftEditor } from '#/features/billing/draft/DraftEditor'
import { useBillingContext, useDraft } from '#/features/billing/draft/api'
import { todayInSA } from '#/features/billing/statement'
import { ApiError, errorMessage } from '#/lib/api/errors'
import { requirePermission } from '#/lib/auth/guards'

export const Route = createFileRoute('/_app/billing/drafts/$draftId')({
  beforeLoad: () => requirePermission({ anyOf: ['billing.manage'] }),
  head: () => ({ meta: [{ title: 'New invoice · doctr8' }] }),
  component: DraftPage,
})

function DraftPage() {
  const { draftId } = Route.useParams()
  const draft = useDraft(draftId)
  const context = useBillingContext(draft.data?.account_id)
  const providers = useProviders()
  const centres = useServiceCentres()
  const failed =
    draft.error ?? context.error ?? providers.error ?? centres.error
  const ready = draft.data && context.data && providers.data && centres.data

  return (
    <div className="mx-auto flex max-w-6xl flex-col gap-6">
      {draft.data && (
        <Link
          to="/patients/file/$accountId"
          params={{ accountId: draft.data.account_id }}
          search={{ tab: 'billing' }}
          className="inline-flex min-h-11 items-center gap-2 self-start rounded-lg pr-3 text-sm font-medium text-muted hover:text-text"
        >
          <ArrowLeft className="size-4" aria-hidden="true" />
          Back to the file
        </Link>
      )}
      <h1 className="text-2xl font-semibold tracking-tight text-text">
        New invoice{' '}
        {context.data && (
          <span className="text-base font-normal text-muted">
            · file {context.data.account_no} · draft
          </span>
        )}
      </h1>
      {failed ? (
        <div className="flex flex-col items-start gap-4">
          <Alert>
            {failed instanceof ApiError && failed.status === 404
              ? 'This draft no longer exists. It may have been finalised or discarded.'
              : errorMessage(failed)}
          </Alert>
          <Button variant="secondary" onClick={() => void draft.refetch()}>
            Try again
          </Button>
        </div>
      ) : !ready ? (
        <p className="py-12 text-center text-sm text-muted">Loading draft…</p>
      ) : (
        <DraftEditor
          key={draft.data.id}
          draft={draft.data}
          context={context.data}
          providers={providers.data}
          centres={centres.data}
          today={todayInSA()}
        />
      )}
    </div>
  )
}
