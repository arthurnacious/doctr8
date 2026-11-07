import { createFileRoute } from '@tanstack/react-router'

import { EmptyState } from '#/components/layout/EmptyState'
import { requirePermission } from '#/lib/auth/guards'
import { navRule } from '#/lib/nav'

export const Route = createFileRoute('/_app/diary')({
  beforeLoad: () => requirePermission(navRule('/diary')),
  component: DiaryPage,
})

function DiaryPage() {
  return (
    <EmptyState title="Diary">
      <p>This section isn't built yet.</p>
    </EmptyState>
  )
}
