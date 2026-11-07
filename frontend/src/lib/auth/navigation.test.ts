import { describe, expect, it } from 'vitest'

import { destinationFor } from './navigation'

describe('destinationFor', () => {
  it('carries the original destination through each step', () => {
    expect(destinationFor('two_step', '/billing')).toEqual({
      href: '/two-step?redirect=%2Fbilling',
    })
    expect(destinationFor('choose_practice', '/billing')).toEqual({
      href: '/choose-practice?redirect=%2Fbilling',
    })
    expect(destinationFor('no_practice', undefined)).toEqual({
      href: '/choose-practice',
    })
    expect(destinationFor('app', '/billing')).toEqual({ href: '/billing' })
    expect(destinationFor('app', undefined)).toEqual({ href: '/' })
  })

  it('never lands off-site', () => {
    expect(destinationFor('app', 'https://evil.example')).toEqual({ href: '/' })
  })
})
