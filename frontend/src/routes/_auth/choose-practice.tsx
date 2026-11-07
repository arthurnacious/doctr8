import { useState } from 'react'
import { createFileRoute, redirect, useNavigate } from '@tanstack/react-router'
import { ChevronRight } from 'lucide-react'

import { Button } from '#/components/ui/Button'
import { Alert } from '#/components/ui/Field'
import { errorMessage } from '#/lib/api/errors'
import { signOut, switchPractice } from '#/lib/auth/actions'
import { parseRedirectSearch, safeRedirect } from '#/lib/auth/guards'
import { ensureSession } from '#/lib/auth/lifecycle'
import { useSession } from '#/lib/auth/session'

export const Route = createFileRoute('/_auth/choose-practice')({
  validateSearch: parseRedirectSearch,
  beforeLoad: async ({ search }) => {
    const s = await ensureSession()
    if (s.status !== 'signed_in') {
      throw redirect({ to: '/sign-in', search: { redirect: search.redirect } })
    }
    // Skipped when the API already picked the only practice.
    if (s.practice && s.practices.length <= 1) {
      throw redirect({ href: safeRedirect(search.redirect) })
    }
  },
  head: () => ({ meta: [{ title: 'Choose a practice · doctr8' }] }),
  component: ChoosePractice,
})

function ChoosePractice() {
  const { redirect: target } = Route.useSearch()
  const session = useSession()
  const navigate = useNavigate()
  const [busyId, setBusyId] = useState<string | null>(null)
  const [error, setError] = useState<string | null>(null)

  if (session.status !== 'signed_in') return null

  async function choose(id: string) {
    setBusyId(id)
    setError(null)
    try {
      await switchPractice(id)
      await navigate({ href: safeRedirect(target), replace: true })
    } catch (err) {
      setError(errorMessage(err))
      setBusyId(null)
    }
  }

  async function onSignOut() {
    await signOut()
    await navigate({ to: '/sign-in', replace: true })
  }

  if (session.practices.length === 0) {
    return (
      <div>
        <h1 className="text-2xl font-semibold tracking-tight text-text">
          No practice yet
        </h1>
        <p className="mt-1.5 text-sm text-muted">
          You're signed in as {session.user.email}, but you're not a member of
          any practice. Ask a practice owner to invite you.
        </p>
        <Button
          variant="secondary"
          className="mt-8"
          fullWidth
          onClick={onSignOut}
        >
          Sign out
        </Button>
      </div>
    )
  }

  return (
    <div>
      <h1 className="text-2xl font-semibold tracking-tight text-text">
        Choose a practice
      </h1>
      <p className="mt-1.5 text-sm text-muted">
        You belong to more than one practice. You can switch later from the
        sidebar.
      </p>
      {error && (
        <div className="mt-6">
          <Alert>{error}</Alert>
        </div>
      )}
      <ul className="mt-8 flex flex-col gap-2" aria-label="Your practices">
        {session.practices.map((p) => (
          <li key={p.id}>
            <button
              type="button"
              onClick={() => choose(p.id)}
              disabled={busyId !== null}
              aria-busy={busyId === p.id || undefined}
              className="flex min-h-14 w-full items-center justify-between gap-3 rounded-lg border border-border bg-card px-4 text-left transition-colors hover:border-border-strong hover:bg-raised disabled:opacity-60"
            >
              <span className="min-w-0">
                <span className="block truncate font-semibold text-text">
                  {p.name}
                </span>
                {p.id === session.practice?.id && (
                  <span className="block text-xs text-subtle">
                    Current practice
                  </span>
                )}
              </span>
              <ChevronRight
                className="size-5 shrink-0 text-muted"
                aria-hidden="true"
              />
            </button>
          </li>
        ))}
      </ul>
      <Button variant="ghost" className="mt-6" fullWidth onClick={onSignOut}>
        Sign out
      </Button>
    </div>
  )
}
