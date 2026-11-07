import { createFileRoute } from '@tanstack/react-router'

import { EmptyState } from '#/components/layout/EmptyState'

export const Route = createFileRoute('/_app/diary')({ component: DiaryPage })

function DiaryPage() {
  return (
    <EmptyState title="Diary">
      <p>This section isn't built yet.</p>
    </EmptyState>
  )
}
