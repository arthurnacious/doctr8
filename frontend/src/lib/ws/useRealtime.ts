import { useEffect } from 'react'

import { currentAccessToken, refreshSession } from '../auth/lifecycle'
import { RealtimeConnection, realtimeUrl } from './realtime'
import type { SessionState } from '../auth/session'

/** The parts of a connection the binding drives (lets tests use a fake). */
export interface Controllable {
  start: () => void
  stop: () => void
  restart: () => void
}

/**
 * Ties the connection's lifetime to "a screen wants realtime for practice X".
 * Releasing is deferred by a tick so a layout that unmounts and immediately
 * remounts (route transitions, React StrictMode) keeps its socket instead of
 * closing and reopening it. A different practice always gets a new socket.
 */
export function createRealtimeBinding(
  conn: Controllable,
  defer: (fn: () => void) => () => void = (fn) => {
    const t = setTimeout(fn, 0)
    return () => clearTimeout(t)
  },
) {
  let active: string | null = null
  let cancelStop: (() => void) | null = null

  return {
    acquire(practiceId: string): void {
      cancelStop?.()
      cancelStop = null
      if (active === practiceId) return
      if (active === null) conn.start()
      else conn.restart()
      active = practiceId
    },
    release(): void {
      cancelStop?.()
      cancelStop = defer(() => {
        cancelStop = null
        active = null
        conn.stop()
      })
    },
  }
}

let connection: RealtimeConnection | null = null
let binding: ReturnType<typeof createRealtimeBinding> | null = null

/** The app's single realtime connection (browser only). */
export function getRealtime(): RealtimeConnection {
  connection ??= new RealtimeConnection({
    url: () => realtimeUrl(window.location),
    getToken: currentAccessToken,
    refresh: refreshSession,
  })
  return connection
}

/**
 * Keep the WebSocket in step with the session: open while signed in to a
 * practice, restart on practice switch, re-authenticate after a refresh,
 * close on sign-out (and when the app layout goes away).
 */
export function useRealtime(session: SessionState): void {
  const practiceId =
    session.status === 'signed_in' ? (session.practice?.id ?? null) : null
  const token = session.status === 'signed_in' ? session.accessToken : null

  useEffect(() => {
    if (!practiceId) return
    binding ??= createRealtimeBinding(getRealtime())
    binding.acquire(practiceId)
    return () => binding?.release()
  }, [practiceId])

  useEffect(() => {
    if (token) getRealtime().reauthenticate()
  }, [token])
}
