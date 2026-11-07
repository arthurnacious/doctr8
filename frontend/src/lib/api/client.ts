/**
 * The one way to call the API. Adds the bearer token; on a 401 it refreshes
 * once and retries once, and if that fails the session ends (the app layout
 * then sends the user to sign in).
 */
import {
  currentAccessToken,
  refreshSession,
  signedOutLocally,
} from '../auth/lifecycle'
import { toApiError } from './errors'

export const API_BASE = '/api'

export interface ApiOptions extends Omit<RequestInit, 'body'> {
  /** JSON body (sets Content-Type). */
  json?: unknown
  /** Send the access token and handle 401s (default true). */
  auth?: boolean
}

function send(path: string, opts: ApiOptions, token: string | null) {
  const { json, auth = true, headers, ...init } = opts
  const h = new Headers(headers)
  if (json !== undefined) h.set('Content-Type', 'application/json')
  if (auth && token) h.set('Authorization', `Bearer ${token}`)
  return fetch(`${API_BASE}${path}`, {
    credentials: 'same-origin',
    ...init,
    headers: h,
    body: json === undefined ? undefined : JSON.stringify(json),
  })
}

async function parse<T>(res: Response): Promise<T> {
  if (!res.ok) throw await toApiError(res)
  if (res.status === 204) return undefined as T
  return (await res.json()) as T
}

export async function api<T>(path: string, opts: ApiOptions = {}): Promise<T> {
  const auth = opts.auth ?? true
  const res = await send(path, opts, currentAccessToken())
  if (res.status !== 401 || !auth) return parse<T>(res)

  // Access token expired or revoked: refresh once, then retry once.
  if (await refreshSession()) {
    const retry = await send(path, opts, currentAccessToken())
    if (retry.status === 401) signedOutLocally('expired')
    return parse<T>(retry)
  }
  return parse<T>(res)
}
