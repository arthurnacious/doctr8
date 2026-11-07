/**
 * Session actions used by the layouts. Stage 3 placeholder: the real
 * sign-out and practice switch (API calls, cache clearing, WebSocket)
 * arrive with the auth flow.
 */
import { setSession } from './session'

export function signOut(): void {
  setSession({ status: 'signed_out' })
}

export function switchPractice(_practiceId: string): Promise<void> {
  return Promise.resolve()
}
