/**
 * The authenticated WebSocket to the API's /ws (backend/src/common/ws).
 *
 * - Authenticates with the first message, never in the URL.
 * - After a token refresh, re-authenticates on the open socket (no gap in
 *   events). After a practice switch the socket is restarted, because the
 *   API scopes a connection to one practice.
 * - Reconnects with jittered exponential backoff. Close codes:
 *   4001 auth failed/expired → refresh, then reconnect;
 *   4009 practice/user changed → reconnect with the current token;
 *   4003 no practice → stop.
 *
 * Nothing listens to events yet: `subscribe()` is there for later.
 */
export type RealtimeMessage =
  | { type: 'ready'; tenant_id: string; expires_at: number }
  | { type: 'reauthenticated'; expires_at: number }
  | {
      type: 'event'
      event: string
      id: string
      account_id?: string
      invoice_id?: string
      status?: string
    }
  | { type: 'resync' }
  | { type: 'subscribed' | 'unsubscribed'; account_id: string }
  | { type: 'pong' }
  | { type: 'error'; code: string; message: string }

export type RealtimeStatus = 'idle' | 'connecting' | 'open' | 'reconnecting'

/** The subset of WebSocket we use (lets tests pass a fake). */
export interface SocketLike {
  readonly readyState: number
  onopen: ((ev: unknown) => void) | null
  onmessage: ((ev: { data: unknown }) => void) | null
  onclose: ((ev: { code: number }) => void) | null
  send: (data: string) => void
  close: (code?: number, reason?: string) => void
}

export interface RealtimeOptions {
  url: () => string
  getToken: () => string | null
  /** Refresh the access token; resolves false if the session is over. */
  refresh: () => Promise<boolean>
  createSocket?: (url: string) => SocketLike
  random?: () => number
}

const OPEN = 1

export const CLOSE_AUTH = 4001
export const CLOSE_NO_PRACTICE = 4003
export const CLOSE_IDENTITY_CHANGED = 4009

/** Delay before reconnect attempt `n` (0-based): 0.5s doubling to 30s, jittered. */
export function backoffDelay(
  attempt: number,
  random: () => number = Math.random,
): number {
  const ceiling = Math.min(30_000, 500 * 2 ** attempt)
  return Math.round(ceiling / 2 + (random() * ceiling) / 2)
}

export function realtimeUrl(loc: { protocol: string; host: string }): string {
  return `${loc.protocol === 'https:' ? 'wss' : 'ws'}://${loc.host}/api/ws`
}

export class RealtimeConnection {
  private socket: SocketLike | null = null
  private running = false
  private attempt = 0
  private timer: ReturnType<typeof setTimeout> | null = null
  private listeners = new Set<(m: RealtimeMessage) => void>()
  private statusListeners = new Set<(s: RealtimeStatus) => void>()
  private _status: RealtimeStatus = 'idle'

  constructor(private readonly opts: RealtimeOptions) {}

  get status(): RealtimeStatus {
    return this._status
  }

  start(): void {
    if (this.running) return
    this.running = true
    this.attempt = 0
    this.open()
  }

  stop(): void {
    this.running = false
    if (this.timer) clearTimeout(this.timer)
    this.timer = null
    const s = this.socket
    this.socket = null
    s?.close(1000, 'closed by client')
    this.setStatus('idle')
  }

  /** Practice switch: a new connection for the new practice. */
  restart(): void {
    this.stop()
    this.start()
  }

  /** Token refresh: send the new token on the open socket. */
  reauthenticate(): void {
    const token = this.opts.getToken()
    if (token && this.socket?.readyState === OPEN) {
      this.socket.send(JSON.stringify({ type: 'auth', token }))
    }
  }

  subscribe(listener: (m: RealtimeMessage) => void): () => void {
    this.listeners.add(listener)
    return () => this.listeners.delete(listener)
  }

  onStatus(listener: (s: RealtimeStatus) => void): () => void {
    this.statusListeners.add(listener)
    return () => this.statusListeners.delete(listener)
  }

  private setStatus(s: RealtimeStatus): void {
    this._status = s
    for (const l of this.statusListeners) l(s)
  }

  private open(): void {
    if (!this.running || !this.opts.getToken()) return
    this.setStatus(this.attempt === 0 ? 'connecting' : 'reconnecting')
    const create =
      this.opts.createSocket ??
      ((url: string) => new WebSocket(url) as unknown as SocketLike)
    const socket = create(this.opts.url())
    this.socket = socket

    socket.onopen = () => {
      const token = this.opts.getToken()
      if (token) socket.send(JSON.stringify({ type: 'auth', token }))
    }
    socket.onmessage = (ev) => {
      if (typeof ev.data !== 'string') return
      let msg: RealtimeMessage
      try {
        msg = JSON.parse(ev.data) as RealtimeMessage
      } catch {
        return
      }
      if (msg.type === 'ready') {
        this.attempt = 0
        this.setStatus('open')
      }
      for (const l of this.listeners) l(msg)
    }
    socket.onclose = (ev) => {
      if (this.socket !== socket) return // superseded or stopped
      this.socket = null
      if (!this.running) return
      this.onClosed(ev.code)
    }
  }

  private onClosed(code: number): void {
    if (code === CLOSE_NO_PRACTICE) {
      this.running = false
      this.setStatus('idle')
      return
    }
    if (code === CLOSE_IDENTITY_CHANGED) {
      this.open()
      return
    }
    if (code === CLOSE_AUTH) {
      this.setStatus('reconnecting')
      this.opts.refresh().then(
        (ok) => (ok ? this.open() : this.stop()),
        () => this.scheduleReconnect(),
      )
      return
    }
    this.scheduleReconnect()
  }

  private scheduleReconnect(): void {
    if (!this.running) return
    this.setStatus('reconnecting')
    const delay = backoffDelay(this.attempt++, this.opts.random)
    this.timer = setTimeout(() => {
      this.timer = null
      this.open()
    }, delay)
  }
}
