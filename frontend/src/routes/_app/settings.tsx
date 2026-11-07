import { createFileRoute } from '@tanstack/react-router'

import { EmptyState } from '#/components/layout/EmptyState'

export const Route = createFileRoute('/_app/settings')({
  component: SettingsPage,
})

function SettingsPage() {
  return (
    <EmptyState title="Settings">
      <p>This section isn't built yet.</p>
    </EmptyState>
  )
}
