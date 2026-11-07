import { createFileRoute } from '@tanstack/react-router'

import { EmptyState } from '#/components/layout/EmptyState'

export const Route = createFileRoute('/_app/patients/file/$accountId')({
  validateSearch: (s: Record<string, unknown>) => ({
    patient: typeof s.patient === 'string' ? s.patient : undefined,
  }),
  component: PatientFile,
})

/** Placeholder until the file view (next stage). */
function PatientFile() {
  const { accountId } = Route.useParams()
  return (
    <EmptyState title="Patient file">
      <p>File {accountId.slice(0, 8)} opens here in the next step.</p>
    </EmptyState>
  )
}
