import { describe, expect, it } from 'vitest'

import { can, initials, toSignedIn } from './session'
import { authResponse } from '#/test/fixtures'

describe('session', () => {
  it('converts an auth response, with an absolute expiry', () => {
    const s = toSignedIn(authResponse(), 1_000)
    expect(s.status).toBe('signed_in')
    expect(s.expiresAt).toBe(1_000 + 600_000)
    expect(s.permissions.has('billing.view')).toBe(true)
  })

  it('checks permissions only inside a practice', () => {
    const inPractice = toSignedIn(authResponse())
    expect(can(inPractice, 'billing.view')).toBe(true)
    expect(can(inPractice, 'roles.manage')).toBe(false)
    expect(can(inPractice, { anyOf: ['roles.manage', 'patients.view'] })).toBe(
      true,
    )
    const noPractice = toSignedIn(authResponse({ practice: null }))
    expect(can(noPractice, 'billing.view')).toBe(false)
    expect(can({ status: 'signed_out' }, 'billing.view')).toBe(false)
    expect(can({ status: 'unknown' }, 'billing.view')).toBe(false)
  })

  it('builds initials', () => {
    expect(initials('Thabo Mokoena')).toBe('TM')
    expect(initials('  naledi  van der Merwe ')).toBe('NM')
    expect(initials('Cher')).toBe('C')
    expect(initials('')).toBe('')
  })
})
