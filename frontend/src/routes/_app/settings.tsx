import { createFileRoute } from '@tanstack/react-router'

import { EmptyState } from '#/components/layout/EmptyState'
import { requirePermission } from '#/lib/auth/guards'
import { navRule } from '#/lib/nav'

export const Route = createFileRoute('/_app/settings')({
  beforeLoad: () => requirePermission(navRule('/settings')),
  component: SettingsPage,
})

function SettingsPage() {
  return (
    <EmptyState title="Settings">
      <p>This section isn't built yet.</p>
    </EmptyState>
  )
}
