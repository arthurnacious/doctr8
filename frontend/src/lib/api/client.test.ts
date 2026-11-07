import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

import { api } from './client'
import { ApiError, errorMessage } from './errors'
import { __resetLifecycleForTests } from '../auth/lifecycle'
import { getSession, setSession, toSignedIn } from '../auth/session'
import { apiError, authResponse, json } from '#/test/fixtures'

function authHeader(call: Array<unknown>): string | null {
  return new Headers((call[1] as RequestInit).headers).get('Authorization')
}

describe('api client', () => {
  beforeEach(() => {
    __resetLifecycleForTests()
    setSession(toSignedIn(authResponse({ access_token: 'old' })))
  })
  afterEach(() => vi.unstubAllGlobals())

  it('sends the bearer token and JSON', async () => {
    const fetchMock = vi.fn().mockResolvedValue(json(200, { ok: 1 }))
    vi.stubGlobal('fetch', fetchMock)
    expect(await api('/roles', { method: 'POST', json: { a: 1 } })).toEqual({
      ok: 1,
    })
    const [url, init] = fetchMock.mock.calls[0] as [string, RequestInit]
    expect(url).toBe('/api/roles')
    expect(authHeader(fetchMock.mock.calls[0])).toBe('Bearer old')
    expect(new Headers(init.headers).get('Content-Type')).toBe(
      'application/json',
    )
    expect(init.body).toBe('{"a":1}')
  })

  it('on 401 refreshes once and retries with the new token', async () => {
    const fetchMock = vi
      .fn()
      .mockResolvedValueOnce(
        apiError(401, 'unauthorized', 'Authentication required'),
      )
      .mockResolvedValueOnce(json(200, authResponse({ access_token: 'new' })))
      .mockResolvedValueOnce(json(200, { ok: 1 }))
    vi.stubGlobal('fetch', fetchMock)
    expect(await api('/roles')).toEqual({ ok: 1 })
    expect(fetchMock.mock.calls.map((c) => c[0])).toEqual([
      '/api/roles',
      '/api/auth/refresh',
      '/api/roles',
    ])
    expect(authHeader(fetchMock.mock.calls[2])).toBe('Bearer new')
  })

  it('concurrent 401s share a single refresh', async () => {
    let refreshes = 0
    vi.stubGlobal(
      'fetch',
      vi.fn(async (url: string, init: RequestInit) => {
        if (url === '/api/auth/refresh') {
          refreshes++
          return json(200, authResponse({ access_token: 'new' }))
        }
        const ok =
          new Headers(init.headers).get('Authorization') === 'Bearer new'
        return ok ? json(200, { url }) : apiError(401, 'unauthorized', 'x')
      }),
    )
    await Promise.all([api('/a'), api('/b'), api('/c')])
    expect(refreshes).toBe(1)
  })

  it('if the refresh fails the session ends', async () => {
    vi.stubGlobal(
      'fetch',
      vi
        .fn()
        .mockResolvedValueOnce(apiError(401, 'unauthorized', 'x'))
        .mockResolvedValueOnce(json(401, {})),
    )
    await expect(api('/roles')).rejects.toMatchObject({ status: 401 })
    expect(getSession()).toEqual({ status: 'signed_out', reason: 'expired' })
  })

  it('a 401 even after refreshing ends the session (no loop)', async () => {
    const fetchMock = vi
      .fn()
      .mockResolvedValueOnce(apiError(401, 'unauthorized', 'x'))
      .mockResolvedValueOnce(json(200, authResponse({ access_token: 'new' })))
      .mockResolvedValueOnce(apiError(401, 'unauthorized', 'x'))
    vi.stubGlobal('fetch', fetchMock)
    await expect(api('/roles')).rejects.toBeInstanceOf(ApiError)
    expect(fetchMock).toHaveBeenCalledTimes(3)
    expect(getSession().status).toBe('signed_out')
  })

  it('unauthenticated calls never refresh', async () => {
    const fetchMock = vi
      .fn()
      .mockResolvedValue(
        apiError(401, 'invalid_credentials', 'Invalid email or password'),
      )
    vi.stubGlobal('fetch', fetchMock)
    await expect(
      api('/auth/login', { method: 'POST', auth: false, json: {} }),
    ).rejects.toMatchObject({
      code: 'invalid_credentials',
    })
    expect(fetchMock).toHaveBeenCalledTimes(1)
    expect(authHeader(fetchMock.mock.calls[0])).toBeNull()
  })

  it('maps errors and empty bodies', async () => {
    vi.stubGlobal(
      'fetch',
      vi
        .fn()
        .mockResolvedValueOnce(apiError(409, 'conflict', 'Already taken'))
        .mockResolvedValueOnce(
          new Response('<html>bad gateway</html>', { status: 502 }),
        )
        .mockResolvedValueOnce(new Response(null, { status: 204 })),
    )
    await expect(api('/x')).rejects.toMatchObject({
      status: 409,
      code: 'conflict',
      message: 'Already taken',
    })
    await expect(api('/x')).rejects.toMatchObject({
      status: 502,
      message: 'Something went wrong. Please try again.',
    })
    expect(await api('/x')).toBeUndefined()
  })

  it('turns any error into a safe message', () => {
    expect(errorMessage(new ApiError(422, 'v', 'Name is required'))).toBe(
      'Name is required',
    )
    expect(errorMessage(new TypeError('Failed to fetch'))).toMatch(
      /Can't reach doctr8/,
    )
    expect(errorMessage(new Error('internal detail'))).toBe(
      'Something went wrong. Please try again.',
    )
  })
})
