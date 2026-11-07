import { createFileRoute } from '@tanstack/react-router'

import { EmptyState } from '#/components/layout/EmptyState'

export const Route = createFileRoute('/_app/claims')({ component: ClaimsPage })

function ClaimsPage() {
  return (
    <EmptyState title="Claims">
      <p>This section isn't built yet.</p>
    </EmptyState>
  )
}
