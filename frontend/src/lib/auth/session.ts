/**
 * The signed-in session, held in memory only.
 *
 * The access token never touches localStorage, sessionStorage or cookies:
 * reloading the page drops it and the app gets a new one from the httpOnly
 * refresh cookie. Components read the session with `useSession()`.
 */
import { useSyncExternalStore } from 'react'

import { parsePermissions, satisfies } from './permissions'
import type { Permission, PermissionRule } from './permissions'

export interface UserSummary {
  id: string
  email: string
  full_name: string
  email_verified_at: string | null
  created_at: string
}

export interface PracticeSummary {
  id: string
  slug: string
  name: string
}

/** Body of /auth/login, /auth/refresh and /auth/switch-practice. */
export interface AuthResponse {
  access_token: string
  token_type: 'Bearer'
  expires_in: number
  user: UserSummary
  practice: PracticeSummary | null
  permissions: Array<string>
  practices: Array<PracticeSummary>
}

export interface SignedIn {
  status: 'signed_in'
  accessToken: string
  /** Epoch milliseconds. */
  expiresAt: number
  user: UserSummary
  practice: PracticeSummary | null
  permissions: ReadonlySet<Permission>
  practices: ReadonlyArray<PracticeSummary>
}

export type SessionState =
  /** Not yet known: the app hasn't tried the refresh cookie. */
  | { status: 'unknown' }
  /** `expired`: the session ended on its own (refresh failed). */
  | { status: 'signed_out'; reason?: 'signed_out' | 'expired' }
  | SignedIn

type Listener = () => void

let state: SessionState = { status: 'unknown' }
const listeners = new Set<Listener>()

export function getSession(): SessionState {
  return state
}

export function setSession(next: SessionState): void {
  state = next
  for (const l of listeners) l()
}

export function subscribe(listener: Listener): () => void {
  listeners.add(listener)
  return () => listeners.delete(listener)
}

export function toSignedIn(
  r: AuthResponse,
  now: number = Date.now(),
): SignedIn {
  return {
    status: 'signed_in',
    accessToken: r.access_token,
    expiresAt: now + r.expires_in * 1000,
    user: r.user,
    practice: r.practice,
    permissions: parsePermissions(r.permissions),
    practices: r.practices,
  }
}

const serverSnapshot: SessionState = { status: 'unknown' }

/** React hook: re-renders when the session changes. */
export function useSession(): SessionState {
  return useSyncExternalStore(subscribe, getSession, () => serverSnapshot)
}

/** Does the current session satisfy a permission (or rule)? */
export function can(
  session: SessionState,
  permissionOrRule: Permission | PermissionRule,
): boolean {
  if (session.status !== 'signed_in' || !session.practice) return false
  const rule =
    typeof permissionOrRule === 'string'
      ? { allOf: [permissionOrRule] }
      : permissionOrRule
  return satisfies(session.permissions, rule)
}

export function useCan(permissionOrRule: Permission | PermissionRule): boolean {
  return can(useSession(), permissionOrRule)
}

/** "Thandi Nkosi" → "TN". */
export function initials(fullName: string): string {
  const parts = fullName.trim().split(/\s+/).filter(Boolean)
  const first = parts.at(0)?.[0] ?? ''
  const last = parts.length > 1 ? (parts.at(-1)?.[0] ?? '') : ''
  return (first + last).toUpperCase()
}
