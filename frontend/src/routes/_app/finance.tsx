import { createFileRoute } from '@tanstack/react-router'

import { EmptyState } from '#/components/layout/EmptyState'

export const Route = createFileRoute('/_app/finance')({
  component: FinancePage,
})

function FinancePage() {
  return (
    <EmptyState title="Finance">
      <p>This section isn't built yet.</p>
    </EmptyState>
  )
}
