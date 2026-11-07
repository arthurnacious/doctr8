import { useState } from 'react'
import { createFileRoute, redirect, useNavigate } from '@tanstack/react-router'

import { Button } from '#/components/ui/Button'
import { Alert, Checkbox, TextField } from '#/components/ui/Field'
import { errorMessage } from '#/lib/api/errors'
import {
  cancelTwoStep,
  hasPendingTwoStep,
  verifyTwoStep,
} from '#/lib/auth/actions'
import { parseRedirectSearch } from '#/lib/auth/guards'
import { destinationFor } from '#/lib/auth/navigation'
import {
  STUB_CODE,
  TWO_STEP_STUB_ENABLED,
  isWellFormed,
} from '#/lib/auth/stubs'

export const Route = createFileRoute('/_auth/two-step')({
  validateSearch: parseRedirectSearch,
  // Only reachable straight after the password step.
  beforeLoad: ({ search }) => {
    if (!hasPendingTwoStep()) {
      throw redirect({ to: '/sign-in', search: { redirect: search.redirect } })
    }
  },
  head: () => ({ meta: [{ title: 'Two-step verification · doctr8' }] }),
  component: TwoStep,
})

function TwoStep() {
  const { redirect: target } = Route.useSearch()
  const navigate = useNavigate()
  const [mode, setMode] = useState<'code' | 'recovery'>('code')
  const [error, setError] = useState<string | null>(null)
  const [busy, setBusy] = useState(false)

  async function onSubmit(e: React.FormEvent<HTMLFormElement>) {
    e.preventDefault()
    const form = new FormData(e.currentTarget)
    const factor = { kind: mode, value: String(form.get('factor') ?? '') }
    if (!isWellFormed(factor)) {
      setError(
        mode === 'code'
          ? 'Enter the 6-digit code.'
          : 'Enter a recovery code like ABCD-1234.',
      )
      return
    }
    setBusy(true)
    setError(null)
    try {
      const step = await verifyTwoStep({
        factor,
        rememberDevice: form.get('remember_device') === 'on',
      })
      await navigate({ ...destinationFor(step, target), replace: true })
    } catch (err) {
      setError(errorMessage(err))
      setBusy(false)
    }
  }

  async function onCancel() {
    await cancelTwoStep()
    await navigate({
      to: '/sign-in',
      search: { redirect: target },
      replace: true,
    })
  }

  return (
    <div>
      <h1 className="text-2xl font-semibold tracking-tight text-text">
        Two-step verification
      </h1>
      <p className="mt-1.5 text-sm text-muted">
        {mode === 'code'
          ? 'Enter the 6-digit code from your authenticator app.'
          : 'Enter one of the recovery codes you saved when you set up two-step verification.'}
      </p>

      {TWO_STEP_STUB_ENABLED && (
        <div className="mt-6">
          <Alert tone="info">
            <strong className="font-semibold text-text">
              Development stub.
            </strong>{' '}
            The API doesn't support two-step verification yet, so this check
            happens in the browser and isn't secure. Use code{' '}
            <span className="font-semibold text-text">{STUB_CODE}</span>.
          </Alert>
        </div>
      )}

      <form onSubmit={onSubmit} className="mt-6 flex flex-col gap-5">
        {error && <Alert>{error}</Alert>}

        {mode === 'code' ? (
          <TextField
            key="code"
            label="6-digit code"
            name="factor"
            inputMode="numeric"
            autoComplete="one-time-code"
            pattern="[0-9 ]*"
            maxLength={7}
            required
            autoFocus
            className="[&_input]:text-lg [&_input]:tracking-[0.4em]"
          />
        ) : (
          <TextField
            key="recovery"
            label="Recovery code"
            name="factor"
            autoComplete="off"
            autoCapitalize="characters"
            spellCheck={false}
            placeholder="ABCD-1234"
            required
            autoFocus
          />
        )}

        <Checkbox
          label="Don't ask again on this device for 30 days"
          name="remember_device"
        />

        <Button type="submit" busy={busy} fullWidth>
          {busy ? 'Checking…' : 'Verify'}
        </Button>

        <div className="flex flex-col gap-1">
          <Button
            variant="ghost"
            onClick={() => {
              setMode(mode === 'code' ? 'recovery' : 'code')
              setError(null)
            }}
          >
            {mode === 'code'
              ? 'Use a recovery code instead'
              : 'Use my authenticator app instead'}
          </Button>
          <Button variant="ghost" onClick={onCancel}>
            Back to sign in
          </Button>
        </div>
      </form>
    </div>
  )
}
