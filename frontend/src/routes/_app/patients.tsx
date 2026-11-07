import { createFileRoute } from '@tanstack/react-router'

import { EmptyState } from '#/components/layout/EmptyState'
import { requirePermission } from '#/lib/auth/guards'
import { navRule } from '#/lib/nav'

export const Route = createFileRoute('/_app/patients')({
  beforeLoad: () => requirePermission(navRule('/patients')),
  component: PatientsPage,
})

function PatientsPage() {
  return (
    <EmptyState title="Patients">
      <p>This section isn't built yet.</p>
    </EmptyState>
  )
}
