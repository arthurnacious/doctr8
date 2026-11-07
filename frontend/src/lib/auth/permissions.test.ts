import { describe, expect, it } from 'vitest'

import { PERMISSIONS, parsePermissions, satisfies } from './permissions'

describe('permissions', () => {
  it('mirrors the 17 API permissions', () => {
    expect(PERMISSIONS).toHaveLength(17)
    expect(new Set(PERMISSIONS).size).toBe(17)
  })

  it('drops keys this build does not know', () => {
    expect([...parsePermissions(['billing.view', 'future.thing'])]).toEqual([
      'billing.view',
    ])
  })

  it('evaluates rules', () => {
    const granted = parsePermissions(['billing.view', 'patients.view'])
    expect(satisfies(granted, undefined)).toBe(true)
    expect(satisfies(granted, { anyOf: ['roles.view', 'billing.view'] })).toBe(
      true,
    )
    expect(satisfies(granted, { anyOf: ['roles.view'] })).toBe(false)
    expect(
      satisfies(granted, { allOf: ['billing.view', 'patients.view'] }),
    ).toBe(true)
    expect(satisfies(granted, { allOf: ['billing.view', 'roles.view'] })).toBe(
      false,
    )
    expect(satisfies(granted, { anyOf: [] })).toBe(false)
  })
})
