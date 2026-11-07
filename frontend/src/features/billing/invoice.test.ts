import { describe, expect, it } from 'vitest'

import {
  claimLabel,
  dueLabel,
  lineState,
  providerName,
  sexLabel,
} from './invoice'

describe('invoice wording', () => {
  it('claim status', () => {
    expect(claimLabel(null)).toBe('Not claimed')
    expect(claimLabel('rejected')).toBe('Claim rejected')
  })

  it('what is owed', () => {
    expect(dueLabel(0, 0)).toBe('Settled')
    expect(dueLabel(100, 0)).toBe('Outstanding')
    expect(dueLabel(0, -50)).toBe('In credit')
    expect(dueLabel(100, -100)).toBe('Outstanding')
  })

  it('credited lines', () => {
    expect(lineState({ amount_cents: 100, credited_cents: 0 })).toBe('open')
    expect(lineState({ amount_cents: 100, credited_cents: 40 })).toBe(
      'part-credited',
    )
    expect(lineState({ amount_cents: 100, credited_cents: 100 })).toBe(
      'credited',
    )
  })

  it('doctor names', () => {
    expect(
      providerName({ title: 'Dr', first_name: 'thandi', last_name: 'Doctor' }),
    ).toBe('Dr T Doctor')
    expect(
      providerName({ title: null, first_name: null, last_name: 'Assist' }),
    ).toBe('Assist')
  })

  it('sex is printed only when known', () => {
    expect(sexLabel('female')).toBe('Female')
    expect(sexLabel('unknown')).toBeNull()
    expect(sexLabel(null)).toBeNull()
  })
})
