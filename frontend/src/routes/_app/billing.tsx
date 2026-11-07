import { createFileRoute } from '@tanstack/react-router'

import { EmptyState } from '#/components/layout/EmptyState'

export const Route = createFileRoute('/_app/billing')({
  component: BillingPage,
})

function BillingPage() {
  return (
    <EmptyState title="Billing">
      <p>This section isn't built yet.</p>
    </EmptyState>
  )
}
