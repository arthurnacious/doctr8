import { describe, expect, it } from 'vitest'

import { parsePermissions } from './auth/permissions'
import { NAV_ITEMS, navRule, visibleNav } from './nav'

const labels = (keys: Array<string>) =>
  visibleNav(parsePermissions(keys)).map((i) => i.label)

describe('navigation', () => {
  it('always shows the dashboard', () => {
    expect(labels([])).toEqual(['Dashboard'])
  })

  it('shows sections the user may see', () => {
    expect(labels(['patients.view'])).toEqual([
      'Dashboard',
      'Diary',
      'Patients',
    ])
    expect(labels(['billing.view'])).toEqual(['Dashboard', 'Billing', 'Claims'])
    expect(labels(['journals.manage'])).toEqual(['Dashboard', 'Finance'])
    expect(labels(['roles.view'])).toEqual(['Dashboard', 'Settings'])
  })

  it('shows everything to an owner, in order', () => {
    const all = NAV_ITEMS.flatMap((i) =>
      i.requires && 'anyOf' in i.requires ? i.requires.anyOf : [],
    )
    expect(labels(all)).toEqual([
      'Dashboard',
      'Diary',
      'Patients',
      'Billing',
      'Claims',
      'Finance',
      'Settings',
    ])
  })

  it('exposes the same rule for route guards', () => {
    expect(navRule('/billing')).toEqual({ anyOf: ['billing.view'] })
    expect(navRule('/')).toBeUndefined()
  })
})
