import { beforeEach, describe, expect, it } from 'vitest'

import {
  STUB_CODE,
  isWellFormed,
  stubDeviceTrusted,
  stubTrustDevice,
  stubVerify,
} from './stubs'

describe('two-step stub', () => {
  beforeEach(() => localStorage.clear())

  it('checks code shapes', () => {
    expect(isWellFormed({ kind: 'code', value: '123 456' })).toBe(true)
    expect(isWellFormed({ kind: 'code', value: '12345' })).toBe(false)
    expect(isWellFormed({ kind: 'code', value: '12a456' })).toBe(false)
    expect(isWellFormed({ kind: 'recovery', value: 'abcd-1234' })).toBe(true)
    expect(isWellFormed({ kind: 'recovery', value: 'ABCD1234' })).toBe(false)
  })

  it('accepts only the stub code, or any well-formed recovery code', () => {
    expect(stubVerify({ kind: 'code', value: STUB_CODE })).toBe(true)
    expect(stubVerify({ kind: 'code', value: '123456' })).toBe(false)
    expect(stubVerify({ kind: 'recovery', value: 'WXYZ-9876' })).toBe(true)
    expect(stubVerify({ kind: 'recovery', value: 'nope' })).toBe(false)
  })

  it('trusts a device for 30 days', () => {
    const now = 1_000_000
    expect(stubDeviceTrusted('u-1', now)).toBe(false)
    stubTrustDevice('u-1', now)
    expect(stubDeviceTrusted('u-1', now + 29 * 86_400_000)).toBe(true)
    expect(stubDeviceTrusted('u-1', now + 31 * 86_400_000)).toBe(false)
    expect(stubDeviceTrusted('u-2', now)).toBe(false)
  })
})
