import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

import { PRACTICE_A, PRACTICE_B, authResponse, json } from '#/test/fixtures'

async function load(stub: boolean) {
  vi.resetModules()
  vi.stubEnv('VITE_TWO_STEP_STUB', stub ? 'true' : 'false')
  const actions = await import('./actions')
  const session = await import('./session')
  const lifecycle = await import('./lifecycle')
  lifecycle.__resetLifecycleForTests()
  session.setSession({ status: 'unknown' })
  return { ...actions, session }
}

describe('auth actions', () => {
  beforeEach(() => localStorage.clear())
  afterEach(() => {
    vi.unstubAllGlobals()
    vi.unstubAllEnvs()
  })

  it('signs in straight to the app with one practice', async () => {
    const a = await load(false)
    const fetchMock = vi.fn(async (..._args: Array<unknown>) =>
      json(200, authResponse()),
    )
    vi.stubGlobal('fetch', fetchMock)
    expect(
      await a.signIn({ email: 'a@b.co', password: 'pw', remember: true }),
    ).toBe('app')
    expect(a.session.getSession().status).toBe('signed_in')
    const body = JSON.parse(
      (fetchMock.mock.calls[0][1] as RequestInit).body as string,
    )
    expect(body).toEqual({ email: 'a@b.co', password: 'pw', remember: true })
  })

  it('asks to choose when the user has several practices and none was picked', async () => {
    const a = await load(false)
    vi.stubGlobal(
      'fetch',
      vi.fn().mockResolvedValue(
        json(
          200,
          authResponse({
            practice: null,
            practices: [PRACTICE_A, PRACTICE_B],
          }),
        ),
      ),
    )
    expect(
      await a.signIn({ email: 'a@b.co', password: 'pw', remember: false }),
    ).toBe('choose_practice')
  })

  it('says so when the user has no practice', async () => {
    const a = await load(false)
    vi.stubGlobal(
      'fetch',
      vi
        .fn()
        .mockResolvedValue(
          json(200, authResponse({ practice: null, practices: [] })),
        ),
    )
    expect(
      await a.signIn({ email: 'a@b.co', password: 'pw', remember: false }),
    ).toBe('no_practice')
  })

  it('with the stub, holds the session until the second step passes', async () => {
    const a = await load(true)
    vi.stubGlobal(
      'fetch',
      vi.fn(async (..._args: Array<unknown>) => json(200, authResponse())),
    )
    expect(
      await a.signIn({ email: 'a@b.co', password: 'pw', remember: false }),
    ).toBe('two_step')
    expect(a.session.getSession().status).toBe('unknown')
    expect(a.hasPendingTwoStep()).toBe(true)

    await expect(
      a.verifyTwoStep({
        factor: { kind: 'code', value: '123456' },
        rememberDevice: false,
      }),
    ).rejects.toMatchObject({
      code: 'invalid_code',
    })
    expect(a.session.getSession().status).toBe('unknown')

    expect(
      await a.verifyTwoStep({
        factor: { kind: 'code', value: '000 000' },
        rememberDevice: false,
      }),
    ).toBe('app')
    expect(a.session.getSession().status).toBe('signed_in')
    expect(a.hasPendingTwoStep()).toBe(false)
  })

  it('with the stub, a recovery code works and a remembered device skips the step', async () => {
    const a = await load(true)
    vi.stubGlobal(
      'fetch',
      vi.fn(async (..._args: Array<unknown>) => json(200, authResponse())),
    )
    await a.signIn({ email: 'a@b.co', password: 'pw', remember: false })
    await a.verifyTwoStep({
      factor: { kind: 'recovery', value: 'abcd-1234' },
      rememberDevice: true,
    })
    expect(
      await a.signIn({ email: 'a@b.co', password: 'pw', remember: false }),
    ).toBe('app')
  })

  it('cancelling the second step revokes the half-made session', async () => {
    const a = await load(true)
    const fetchMock = vi.fn(async (..._args: Array<unknown>) =>
      json(200, authResponse()),
    )
    vi.stubGlobal('fetch', fetchMock)
    await a.signIn({ email: 'a@b.co', password: 'pw', remember: false })
    await a.cancelTwoStep()
    expect(fetchMock.mock.calls.at(-1)?.[0]).toBe('/api/auth/logout')
    expect(a.hasPendingTwoStep()).toBe(false)
  })

  it('switching practice takes the new token and permissions', async () => {
    const a = await load(false)
    a.session.setSession(
      a.session.toSignedIn(
        authResponse({ practices: [PRACTICE_A, PRACTICE_B] }),
      ),
    )
    const fetchMock = vi.fn().mockResolvedValue(
      json(
        200,
        authResponse({
          access_token: 'b-token',
          practice: PRACTICE_B,
          permissions: ['roles.view'],
        }),
      ),
    )
    vi.stubGlobal('fetch', fetchMock)
    await a.switchPractice('p-b')
    const s = a.session.getSession()
    expect(
      s.status === 'signed_in' && [
        s.accessToken,
        s.practice?.id,
        [...s.permissions],
      ],
    ).toEqual(['b-token', 'p-b', ['roles.view']])
    const [url, init] = fetchMock.mock.calls[0] as [string, RequestInit]
    expect(url).toBe('/api/auth/switch-practice')
    expect(JSON.parse(init.body as string)).toEqual({ tenant_id: 'p-b' })
  })

  it('a dead session while switching ends it', async () => {
    const a = await load(false)
    a.session.setSession(a.session.toSignedIn(authResponse()))
    vi.stubGlobal(
      'fetch',
      vi
        .fn()
        .mockResolvedValue(
          json(401, { error: { code: 'unauthorized', message: 'x' } }),
        ),
    )
    await expect(a.switchPractice('p-b')).rejects.toMatchObject({ status: 401 })
    expect(a.session.getSession()).toEqual({
      status: 'signed_out',
      reason: 'expired',
    })
  })

  it('signing out calls the API and forgets the session, even offline', async () => {
    const a = await load(false)
    a.session.setSession(a.session.toSignedIn(authResponse()))
    vi.stubGlobal('fetch', vi.fn().mockRejectedValue(new TypeError('offline')))
    await a.signOut()
    expect(a.session.getSession()).toEqual({
      status: 'signed_out',
      reason: 'signed_out',
    })
  })
})
