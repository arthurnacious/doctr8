import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

import { RealtimeConnection, backoffDelay, realtimeUrl } from './realtime'
import type { SocketLike } from './realtime'

class FakeSocket implements SocketLike {
  readyState = 0
  sent: Array<unknown> = []
  closed: { code?: number } | null = null
  onopen: ((ev: unknown) => void) | null = null
  onmessage: ((ev: { data: unknown }) => void) | null = null
  onclose: ((ev: { code: number }) => void) | null = null
  constructor(readonly url: string) {}
  send(data: string) {
    this.sent.push(JSON.parse(data))
  }
  close(code?: number) {
    this.closed = { code }
    this.readyState = 3
  }
  // test helpers
  serverOpen() {
    this.readyState = 1
    this.onopen?.({})
  }
  serverSend(msg: unknown) {
    this.onmessage?.({ data: JSON.stringify(msg) })
  }
  serverClose(code: number) {
    this.readyState = 3
    this.onclose?.({ code })
  }
}

function setup(
  over: Partial<ConstructorParameters<typeof RealtimeConnection>[0]> = {},
) {
  const sockets: Array<FakeSocket> = []
  let token: string | null = 't1'
  const refresh = vi.fn(async () => {
    token = 't2'
    return true
  })
  const conn = new RealtimeConnection({
    url: () => 'ws://localhost/api/ws',
    getToken: () => token,
    refresh,
    createSocket: (url) => {
      const s = new FakeSocket(url)
      sockets.push(s)
      return s
    },
    random: () => 0.5,
    ...over,
  })
  return {
    conn,
    sockets,
    refresh,
    setToken: (t: string | null) => (token = t),
    last: () => sockets.at(-1)!,
  }
}

describe('realtime connection', () => {
  beforeEach(() => vi.useFakeTimers())
  afterEach(() => vi.useRealTimers())

  it('authenticates with the first message, not the URL', () => {
    const { conn, last } = setup()
    conn.start()
    expect(last().url).toBe('ws://localhost/api/ws')
    last().serverOpen()
    expect(last().sent).toEqual([{ type: 'auth', token: 't1' }])
    last().serverSend({ type: 'ready', tenant_id: 'p', expires_at: 1 })
    expect(conn.status).toBe('open')
  })

  it('re-authenticates on the open socket after a token refresh', () => {
    const { conn, last, setToken, sockets } = setup()
    conn.start()
    last().serverOpen()
    setToken('t9')
    conn.reauthenticate()
    expect(last().sent.at(-1)).toEqual({ type: 'auth', token: 't9' })
    expect(sockets).toHaveLength(1)
  })

  it('restarts with a new socket on practice switch', () => {
    const { conn, sockets } = setup()
    conn.start()
    sockets[0].serverOpen()
    conn.restart()
    expect(sockets[0].closed).toEqual({ code: 1000 })
    expect(sockets).toHaveLength(2)
  })

  it('stop closes the socket and never reconnects', () => {
    const { conn, sockets } = setup()
    conn.start()
    sockets[0].serverOpen()
    conn.stop()
    sockets[0].serverClose(1006)
    vi.advanceTimersByTime(60_000)
    expect(sockets).toHaveLength(1)
    expect(conn.status).toBe('idle')
  })

  it('4001: refreshes the token, then reconnects with it', async () => {
    const { conn, sockets, refresh } = setup()
    conn.start()
    sockets[0].serverOpen()
    sockets[0].serverClose(4001)
    await vi.waitFor(() => expect(sockets).toHaveLength(2))
    expect(refresh).toHaveBeenCalledTimes(1)
    sockets[1].serverOpen()
    expect(sockets[1].sent).toEqual([{ type: 'auth', token: 't2' }])
  })

  it('4001 with a dead session stops', async () => {
    const { conn, sockets } = setup({ refresh: async () => false })
    conn.start()
    sockets[0].serverClose(4001)
    await vi.waitFor(() => expect(conn.status).toBe('idle'))
    vi.advanceTimersByTime(60_000)
    expect(sockets).toHaveLength(1)
  })

  it('4003 (no practice) stops; 4009 reconnects at once', () => {
    const a = setup()
    a.conn.start()
    a.sockets[0].serverClose(4003)
    vi.advanceTimersByTime(60_000)
    expect(a.sockets).toHaveLength(1)

    const b = setup()
    b.conn.start()
    b.sockets[0].serverClose(4009)
    expect(b.sockets).toHaveLength(2)
  })

  it('other drops reconnect with backoff, reset by a successful ready', () => {
    const { conn, sockets } = setup()
    conn.start()
    sockets[0].serverClose(1006)
    expect(conn.status).toBe('reconnecting')
    vi.advanceTimersByTime(374)
    expect(sockets).toHaveLength(1)
    vi.advanceTimersByTime(1) // 375ms = 500 * 0.75 at attempt 0
    expect(sockets).toHaveLength(2)
    sockets[1].serverClose(1006)
    vi.advanceTimersByTime(749)
    expect(sockets).toHaveLength(2)
    vi.advanceTimersByTime(1) // attempt 1: 750ms
    expect(sockets).toHaveLength(3)
    sockets[2].serverOpen()
    sockets[2].serverSend({ type: 'ready', tenant_id: 'p', expires_at: 1 })
    sockets[2].serverClose(1006)
    vi.advanceTimersByTime(375) // back to attempt 0
    expect(sockets).toHaveLength(4)
  })

  it('ignores close events from superseded sockets', () => {
    const { conn, sockets } = setup()
    conn.start()
    conn.restart()
    sockets[0].serverClose(1006)
    vi.advanceTimersByTime(60_000)
    expect(sockets).toHaveLength(2)
  })

  it('does not connect without a token', () => {
    const { conn, sockets, setToken } = setup()
    setToken(null)
    conn.start()
    expect(sockets).toHaveLength(0)
  })

  it('delivers messages to subscribers and tolerates junk', () => {
    const { conn, sockets } = setup()
    const got: Array<unknown> = []
    const off = conn.subscribe((m) => got.push(m))
    conn.start()
    sockets[0].onmessage?.({ data: 'not json' })
    sockets[0].serverSend({ type: 'resync' })
    off()
    sockets[0].serverSend({ type: 'resync' })
    expect(got).toEqual([{ type: 'resync' }])
  })

  it('backoff grows to a 30s ceiling with jitter', () => {
    expect(backoffDelay(0, () => 0)).toBe(250)
    expect(backoffDelay(0, () => 1)).toBe(500)
    expect(backoffDelay(3, () => 1)).toBe(4000)
    expect(backoffDelay(20, () => 1)).toBe(30_000)
    expect(backoffDelay(20, () => 0)).toBe(15_000)
  })

  it('builds the socket URL from the page origin', () => {
    expect(realtimeUrl({ protocol: 'http:', host: 'localhost:5173' })).toBe(
      'ws://localhost:5173/api/ws',
    )
    expect(realtimeUrl({ protocol: 'https:', host: 'app.doctr8.co.za' })).toBe(
      'wss://app.doctr8.co.za/api/ws',
    )
  })
})
