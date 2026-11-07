import { afterEach, describe, expect, it } from 'vitest'
import { isRedirect } from '@tanstack/react-router'

import { parseRedirectSearch, requirePermission, safeRedirect } from './guards'
import { setSession, toSignedIn } from './session'
import { authResponse } from '#/test/fixtures'

describe('guards', () => {
  afterEach(() => setSession({ status: 'unknown' }))

  it('only allows same-site redirects', () => {
    expect(safeRedirect('/billing?x=1')).toBe('/billing?x=1')
    for (const bad of [
      'https://evil.example',
      '//evil.example',
      '/\\evil.example',
      'billing',
      '',
      undefined,
      42,
    ]) {
      expect(safeRedirect(bad)).toBe('/')
    }
  })

  it('parses redirect search params', () => {
    expect(parseRedirectSearch({ redirect: '/x', expired: 'true' })).toEqual({
      redirect: '/x',
      expired: true,
    })
    expect(parseRedirectSearch({ redirect: 5, expired: 'no' })).toEqual({
      redirect: undefined,
      expired: undefined,
    })
  })

  it('sends users without the permission home', () => {
    setSession(toSignedIn(authResponse({ permissions: ['patients.view'] })))
    expect(() => requirePermission({ anyOf: ['patients.view'] })).not.toThrow()
    expect(() => requirePermission(undefined)).not.toThrow()
    let thrown: unknown
    try {
      requirePermission({ anyOf: ['billing.view'] })
    } catch (e) {
      thrown = e
    }
    expect(isRedirect(thrown)).toBe(true)
  })
})
