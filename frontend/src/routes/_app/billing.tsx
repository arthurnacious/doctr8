import { createFileRoute } from '@tanstack/react-router'

import { EmptyState } from '#/components/layout/EmptyState'
import { requirePermission } from '#/lib/auth/guards'
import { navRule } from '#/lib/nav'

export const Route = createFileRoute('/_app/billing')({
  beforeLoad: () => requirePermission(navRule('/billing')),
  component: BillingPage,
})

function BillingPage() {
  return (
    <EmptyState title="Billing">
      <p>This section isn't built yet.</p>
    </EmptyState>
  )
}
