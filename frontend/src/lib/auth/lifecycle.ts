/**
 * Session lifecycle: applying token responses, refreshing, and keeping
 * tabs in step. The access token stays in memory; the refresh token is an
 * httpOnly cookie on /api/auth that only the browser sees.
 *
 * Refresh is:
 * - single-flight within a tab (concurrent 401s share one refresh);
 * - serialised across tabs with the Web Locks API, because the API rotates
 *   refresh tokens and treats reuse of an old one as theft;
 * - shared: the tab that refreshes broadcasts the new access token to the
 *   others over a BroadcastChannel (memory to memory, never storage).
 */
import { ApiError, toApiError } from '../api/errors'
import { getSession, setSession, toSignedIn } from './session'
import type { AuthResponse, SessionState } from './session'

export const AUTH_BASE = '/api/auth'
const CHANNEL_NAME = 'doctr8.auth'
const LOCK_NAME = 'doctr8.auth.refresh'
/** Refresh this long before the access token expires. */
const REFRESH_AHEAD_MS = 60_000
const MIN_REFRESH_DELAY_MS = 5_000

type TabMessage =
  | { type: 'session'; response: AuthResponse; expiresAt: number }
  | { type: 'signed_out' }

let refreshTimer: ReturnType<typeof setTimeout> | null = null
let inflight: Promise<boolean> | null = null
let channel: BroadcastChannel | null | undefined

function tabChannel(): BroadcastChannel | null {
  if (channel !== undefined) return channel
  if (typeof BroadcastChannel === 'undefined') return (channel = null)
  channel = new BroadcastChannel(CHANNEL_NAME)
  channel.onmessage = (e: MessageEvent<TabMessage>) => {
    const msg = e.data
    if (msg.type === 'session') {
      applyAuthResponse(msg.response, {
        broadcast: false,
        expiresAt: msg.expiresAt,
      })
    } else {
      signedOutLocally('signed_out', { broadcast: false })
    }
  }
  return channel
}

/** Start listening to other tabs. Call once in the browser. */
export function initTabSync(): void {
  tabChannel()
}

export function applyAuthResponse(
  response: AuthResponse,
  opts: { broadcast?: boolean; expiresAt?: number } = {},
): void {
  const next = toSignedIn(response)
  if (opts.expiresAt !== undefined) next.expiresAt = opts.expiresAt
  setSession(next)
  scheduleRefresh(next.expiresAt)
  if (opts.broadcast !== false) {
    tabChannel()?.postMessage({
      type: 'session',
      response,
      expiresAt: next.expiresAt,
    } satisfies TabMessage)
  }
}

/** Forget the session in this tab (and, by default, the others). */
export function signedOutLocally(
  reason: 'signed_out' | 'expired',
  opts: { broadcast?: boolean } = {},
): void {
  if (refreshTimer) clearTimeout(refreshTimer)
  refreshTimer = null
  setSession({ status: 'signed_out', reason })
  if (opts.broadcast !== false) {
    tabChannel()?.postMessage({ type: 'signed_out' } satisfies TabMessage)
  }
}

export function refreshDelay(
  expiresAt: number,
  now: number = Date.now(),
): number {
  return Math.max(MIN_REFRESH_DELAY_MS, expiresAt - now - REFRESH_AHEAD_MS)
}

function scheduleRefresh(expiresAt: number): void {
  if (typeof window === 'undefined') return
  if (refreshTimer) clearTimeout(refreshTimer)
  refreshTimer = setTimeout(() => {
    refreshTimer = null
    // Failures surface on the next API call (401 → refresh → sign in).
    refreshSession().catch(() => {})
  }, refreshDelay(expiresAt))
}

async function withRefreshLock<T>(fn: () => Promise<T>): Promise<T> {
  const locks = typeof navigator === 'undefined' ? undefined : navigator.locks
  if (!locks) return fn()
  return locks.request(LOCK_NAME, fn)
}

/**
 * Exchange the refresh cookie for a new access token. Resolves true on
 * success, false if the session is over (the user must sign in again).
 * Rejects only on network/server errors.
 */
export function refreshSession(): Promise<boolean> {
  inflight ??= doRefresh().finally(() => {
    inflight = null
  })
  return inflight
}

async function doRefresh(): Promise<boolean> {
  const before = currentAccessToken()
  return withRefreshLock(async () => {
    // Another tab may have refreshed while we waited for the lock and
    // shared its token with us. Don't spend the (now rotated) cookie again.
    const s = getSession()
    if (
      s.status === 'signed_in' &&
      s.accessToken !== before &&
      s.expiresAt - Date.now() > REFRESH_AHEAD_MS
    ) {
      return true
    }
    const res = await fetch(`${AUTH_BASE}/refresh`, {
      method: 'POST',
      credentials: 'same-origin',
    })
    if (res.ok) {
      applyAuthResponse((await res.json()) as AuthResponse)
      return true
    }
    if (res.status === 401) {
      signedOutLocally(s.status === 'signed_in' ? 'expired' : 'signed_out')
      return false
    }
    throw await toApiError(res)
  })
}

/**
 * Resolve the session for a route guard: on first load, try the refresh
 * cookie once. Never throws; an unreachable API counts as signed out.
 */
export async function ensureSession(): Promise<SessionState> {
  if (getSession().status === 'unknown') {
    try {
      await refreshSession()
    } catch (err) {
      if (!(err instanceof ApiError) && !(err instanceof TypeError)) throw err
      setSession({ status: 'signed_out', reason: 'signed_out' })
    }
  }
  return getSession()
}

export function currentAccessToken(): string | null {
  const s = getSession()
  return s.status === 'signed_in' ? s.accessToken : null
}

/** Test hook: reset module state between tests. */
export function __resetLifecycleForTests(): void {
  if (refreshTimer) clearTimeout(refreshTimer)
  refreshTimer = null
  inflight = null
  channel?.close()
  channel = undefined
}
