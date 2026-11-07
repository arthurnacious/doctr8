import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

import {
  __resetLifecycleForTests,
  applyAuthResponse,
  ensureSession,
  initTabSync,
  refreshDelay,
  refreshSession,
} from './lifecycle'
import { getSession, setSession, toSignedIn } from './session'
import { authResponse, json } from '#/test/fixtures'

describe('session lifecycle', () => {
  beforeEach(() => {
    __resetLifecycleForTests()
    setSession({ status: 'unknown' })
  })
  afterEach(() => {
    __resetLifecycleForTests()
    vi.unstubAllGlobals()
  })

  it('refreshes a minute before expiry, never sooner than 5s', () => {
    expect(refreshDelay(1_000_000 + 600_000, 1_000_000)).toBe(540_000)
    expect(refreshDelay(1_000_000 + 30_000, 1_000_000)).toBe(5_000)
  })

  it('shares one refresh between concurrent callers', async () => {
    const fetchMock = vi
      .fn()
      .mockResolvedValue(json(200, authResponse({ access_token: 'fresh' })))
    vi.stubGlobal('fetch', fetchMock)
    const results = await Promise.all([
      refreshSession(),
      refreshSession(),
      refreshSession(),
    ])
    expect(results).toEqual([true, true, true])
    expect(fetchMock).toHaveBeenCalledTimes(1)
    expect(fetchMock.mock.calls[0][0]).toBe('/api/auth/refresh')
    const s = getSession()
    expect(s.status === 'signed_in' && s.accessToken).toBe('fresh')
  })

  it('a rejected refresh ends the session', async () => {
    setSession(toSignedIn(authResponse()))
    vi.stubGlobal('fetch', vi.fn().mockResolvedValue(json(401, {})))
    expect(await refreshSession()).toBe(false)
    expect(getSession()).toEqual({ status: 'signed_out', reason: 'expired' })
  })

  it('on first load, no cookie means signed out (not expired)', async () => {
    vi.stubGlobal('fetch', vi.fn().mockResolvedValue(json(401, {})))
    expect(await ensureSession()).toEqual({
      status: 'signed_out',
      reason: 'signed_out',
    })
  })

  it('an unreachable API counts as signed out for guards', async () => {
    vi.stubGlobal(
      'fetch',
      vi.fn().mockRejectedValue(new TypeError('Failed to fetch')),
    )
    expect((await ensureSession()).status).toBe('signed_out')
  })

  it('does not refresh again once the session is known', async () => {
    const fetchMock = vi.fn()
    vi.stubGlobal('fetch', fetchMock)
    setSession(toSignedIn(authResponse()))
    await ensureSession()
    expect(fetchMock).not.toHaveBeenCalled()
  })

  it('skips the network if another tab refreshed while we waited for the lock', async () => {
    setSession(toSignedIn(authResponse({ access_token: 'old' })))
    const fetchMock = vi.fn()
    vi.stubGlobal('fetch', fetchMock)
    vi.stubGlobal('navigator', {
      locks: {
        request: async (_name: string, fn: () => Promise<unknown>) => {
          // While we were queued, the other tab refreshed and shared its token.
          setSession(
            toSignedIn(authResponse({ access_token: 'from-other-tab' })),
          )
          return fn()
        },
      },
    })
    expect(await refreshSession()).toBe(true)
    expect(fetchMock).not.toHaveBeenCalled()
  })

  it('shares new tokens and sign-outs with other tabs, in memory', async () => {
    initTabSync()
    const other = new BroadcastChannel('doctr8.auth')
    const received: Array<unknown> = []
    other.onmessage = (e) => received.push(e.data)
    applyAuthResponse(authResponse({ access_token: 'shared' }))
    await vi.waitFor(() => expect(received).toHaveLength(1))
    expect(received[0]).toMatchObject({
      type: 'session',
      response: { access_token: 'shared' },
    })

    // And this tab follows the other's sign-out.
    other.postMessage({ type: 'signed_out' })
    await vi.waitFor(() => expect(getSession().status).toBe('signed_out'))
    other.close()
  })

  it('never writes tokens to web storage', () => {
    applyAuthResponse(authResponse({ access_token: 'secret-token' }))
    const stored =
      JSON.stringify({ ...localStorage }) +
      JSON.stringify({ ...sessionStorage })
    expect(stored).not.toContain('secret-token')
  })
})
