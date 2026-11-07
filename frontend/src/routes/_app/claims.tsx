import { createFileRoute } from '@tanstack/react-router'

import { EmptyState } from '#/components/layout/EmptyState'
import { requirePermission } from '#/lib/auth/guards'
import { navRule } from '#/lib/nav'

export const Route = createFileRoute('/_app/claims')({
  beforeLoad: () => requirePermission(navRule('/claims')),
  component: ClaimsPage,
})

function ClaimsPage() {
  return (
    <EmptyState title="Claims">
      <p>This section isn't built yet.</p>
    </EmptyState>
  )
}
