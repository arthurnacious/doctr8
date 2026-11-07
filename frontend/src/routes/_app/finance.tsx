import { createFileRoute } from '@tanstack/react-router'

import { EmptyState } from '#/components/layout/EmptyState'
import { requirePermission } from '#/lib/auth/guards'
import { navRule } from '#/lib/nav'

export const Route = createFileRoute('/_app/finance')({
  beforeLoad: () => requirePermission(navRule('/finance')),
  component: FinancePage,
})

function FinancePage() {
  return (
    <EmptyState title="Finance">
      <p>This section isn't built yet.</p>
    </EmptyState>
  )
}
