/**
 * What the screens do: sign in, the second step, choosing/switching
 * practice and signing out. Each returns where the user should go next.
 */
import { api } from '../api/client'
import { ApiError } from '../api/errors'
import { AUTH_BASE, applyAuthResponse, signedOutLocally } from './lifecycle'
import { getSession } from './session'
import {
  TWO_STEP_STUB_ENABLED,
  stubDeviceTrusted,
  stubTrustDevice,
  stubVerify,
} from './stubs'
import type { SecondFactor } from './stubs'
import type { AuthResponse, SignedIn } from './session'

export type NextStep = 'two_step' | 'choose_practice' | 'no_practice' | 'app'

export function nextStepFor(
  s: Pick<SignedIn, 'practice' | 'practices'>,
): NextStep {
  if (s.practice) return 'app'
  return s.practices.length === 0 ? 'no_practice' : 'choose_practice'
}

function signedIn(): SignedIn {
  const s = getSession()
  if (s.status !== 'signed_in') throw new Error('expected a signed-in session')
  return s
}

/** Held in memory between the password step and the second step (stub). */
let pendingTwoStep: AuthResponse | null = null

export function hasPendingTwoStep(): boolean {
  return pendingTwoStep !== null
}

export async function signIn(input: {
  email: string
  password: string
  /** "Keep me signed in". Sent now; the API doesn't act on it yet. */
  remember: boolean
}): Promise<NextStep> {
  const response = await api<AuthResponse>('/auth/login', {
    method: 'POST',
    auth: false,
    json: {
      email: input.email,
      password: input.password,
      remember: input.remember,
    },
  })
  if (TWO_STEP_STUB_ENABLED && !stubDeviceTrusted(response.user.id)) {
    pendingTwoStep = response
    return 'two_step'
  }
  applyAuthResponse(response)
  return nextStepFor(signedIn())
}

export async function verifyTwoStep(input: {
  factor: SecondFactor
  rememberDevice: boolean
}): Promise<NextStep> {
  const pending = pendingTwoStep
  if (!pending) {
    throw new ApiError(
      401,
      'two_step_expired',
      'Your sign-in timed out. Please sign in again.',
    )
  }
  if (!stubVerify(input.factor)) {
    throw new ApiError(
      401,
      'invalid_code',
      input.factor.kind === 'code'
        ? "That code isn't right. Check your authenticator app and try again."
        : "That recovery code isn't right.",
    )
  }
  if (input.rememberDevice) stubTrustDevice(pending.user.id)
  pendingTwoStep = null
  applyAuthResponse(pending)
  return nextStepFor(signedIn())
}

/** Abandon a half-finished sign-in. */
export async function cancelTwoStep(): Promise<void> {
  pendingTwoStep = null
  // The stub's password step already set a refresh cookie: revoke it.
  await fetch(`${AUTH_BASE}/logout`, {
    method: 'POST',
    credentials: 'same-origin',
  }).catch(() => {})
}

/**
 * Get a token for another practice. Session listeners (router.tsx) clear
 * the query cache and re-run route guards; the WebSocket reconnects.
 */
export async function switchPractice(practiceId: string): Promise<void> {
  try {
    const response = await api<AuthResponse>('/auth/switch-practice', {
      method: 'POST',
      auth: false, // authenticated by the refresh cookie
      json: { tenant_id: practiceId },
    })
    applyAuthResponse(response)
  } catch (err) {
    if (err instanceof ApiError && err.status === 401)
      signedOutLocally('expired')
    throw err
  }
}

export async function signOut(): Promise<void> {
  pendingTwoStep = null
  try {
    await fetch(`${AUTH_BASE}/logout`, {
      method: 'POST',
      credentials: 'same-origin',
    })
  } catch {
    // Offline: forget the session locally anyway.
  }
  signedOutLocally('signed_out')
}

/** Test hook. */
export function __resetActionsForTests(): void {
  pendingTwoStep = null
}
