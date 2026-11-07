import { createFileRoute } from '@tanstack/react-router'

import { EmptyState } from '#/components/layout/EmptyState'
import { useSession } from '#/lib/auth/session'

export const Route = createFileRoute('/_app/')({ component: Dashboard })

/** Empty for now: a welcome and nothing else. No fake charts or numbers. */
function Dashboard() {
  const session = useSession()
  const firstName =
    session.status === 'signed_in'
      ? session.user.full_name.split(/\s+/)[0]
      : undefined
  const practice =
    session.status === 'signed_in' ? session.practice?.name : undefined

  return (
    <EmptyState title={firstName ? `Welcome, ${firstName}` : 'Welcome'}>
      <p>
        {practice ? `You're signed in to ${practice}. ` : ''}
        Your day's work will appear here as doctr8 grows.
      </p>
    </EmptyState>
  )
}
