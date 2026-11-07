import { createFileRoute } from '@tanstack/react-router'

import { EmptyState } from '#/components/layout/EmptyState'

export const Route = createFileRoute('/_app/patients')({
  component: PatientsPage,
})

function PatientsPage() {
  return (
    <EmptyState title="Patients">
      <p>This section isn't built yet.</p>
    </EmptyState>
  )
}
