import { useState } from 'react'
import { createFileRoute, useNavigate } from '@tanstack/react-router'

import { Button } from '#/components/ui/Button'
import {
  Alert,
  Checkbox,
  PasswordField,
  TextField,
} from '#/components/ui/Field'
import { ApiError, errorMessage } from '#/lib/api/errors'
import { signIn } from '#/lib/auth/actions'
import { parseRedirectSearch, redirectIfSignedIn } from '#/lib/auth/guards'
import { destinationFor } from '#/lib/auth/navigation'

export const Route = createFileRoute('/_auth/sign-in')({
  validateSearch: parseRedirectSearch,
  beforeLoad: ({ search }) => redirectIfSignedIn(search),
  head: () => ({ meta: [{ title: 'Sign in · doctr8' }] }),
  component: SignIn,
})

function SignIn() {
  const { redirect, expired } = Route.useSearch()
  const navigate = useNavigate()
  const [error, setError] = useState<string | null>(null)
  const [busy, setBusy] = useState(false)

  async function onSubmit(e: React.FormEvent<HTMLFormElement>) {
    e.preventDefault()
    const form = new FormData(e.currentTarget)
    setBusy(true)
    setError(null)
    try {
      const step = await signIn({
        email: String(form.get('email') ?? '').trim(),
        password: String(form.get('password') ?? ''),
        remember: form.get('remember') === 'on',
      })
      await navigate({ ...destinationFor(step, redirect), replace: true })
    } catch (err) {
      setError(
        err instanceof ApiError && err.status === 401
          ? 'That email and password combination is incorrect.'
          : errorMessage(err),
      )
      setBusy(false)
    }
  }

  return (
    <div>
      <h1 className="text-2xl font-semibold tracking-tight text-text">
        Sign in
      </h1>
      <p className="mt-1.5 text-sm text-muted">
        Welcome back. Sign in to your practice.
      </p>

      <form
        onSubmit={onSubmit}
        noValidate={false}
        className="mt-8 flex flex-col gap-5"
      >
        {expired && !error && (
          <Alert tone="info">Your session ended. Please sign in again.</Alert>
        )}
        {error && <Alert>{error}</Alert>}

        <TextField
          label="Email"
          name="email"
          type="email"
          autoComplete="username"
          required
          autoFocus
        />
        <PasswordField
          label="Password"
          name="password"
          autoComplete="current-password"
          required
        />
        <Checkbox
          label="Keep me signed in"
          name="remember"
          hint="Leave this off on shared computers."
        />
        <Button type="submit" busy={busy} fullWidth>
          {busy ? 'Signing in…' : 'Sign in'}
        </Button>
      </form>
    </div>
  )
}
