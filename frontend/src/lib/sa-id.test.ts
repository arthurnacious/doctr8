import { describe, expect, it } from 'vitest'

import { checkSaId, dobMatchesSaId, luhnValid } from './sa-id'

const TODAY = new Date('2026-11-07T10:00:00Z')

/** Append the Luhn check digit to 12 digits. */
function withCheck(twelve: string): string {
  for (let c = 0; c <= 9; c++) if (luhnValid(twelve + c)) return twelve + c
  throw new Error('unreachable')
}

describe('SA ID numbers', () => {
  it('accepts a valid number and derives the date of birth', () => {
    expect(checkSaId('800101 5009 087', TODAY)).toEqual({
      ok: true,
      dateOfBirth: '1980-01-01',
    })
  })

  it('rejects bad length, dates, citizenship and check digits', () => {
    expect(checkSaId('800101500908', TODAY)).toMatchObject({
      ok: false,
      error: /13 digits/,
    })
    expect(checkSaId(withCheck('800230500908'), TODAY)).toMatchObject({
      ok: false,
      error: /real date/,
    })
    expect(checkSaId(withCheck('800101500938'), TODAY)).toMatchObject({
      ok: false,
      error: /citizenship/,
    })
    expect(checkSaId('8001015009088', TODAY)).toMatchObject({
      ok: false,
      error: /typo/,
    })
  })

  it('never repeats the number in an error', () => {
    for (const bad of ['8001015009088', '12345', '9913325009087']) {
      const r = checkSaId(bad, TODAY)
      expect(r.ok).toBe(false)
      if (!r.ok) expect(r.error).not.toContain(bad.slice(0, 6))
    }
  })

  it('picks the most recent century not in the future', () => {
    expect(checkSaId(withCheck('050101580008'), TODAY)).toMatchObject({
      dateOfBirth: '2005-01-01',
    })
    expect(checkSaId(withCheck('300101580008'), TODAY)).toMatchObject({
      dateOfBirth: '1930-01-01',
    })
    expect(checkSaId(withCheck('261225580008'), TODAY)).toMatchObject({
      dateOfBirth: '1926-12-25',
    })
    expect(checkSaId(withCheck('000229500008'), TODAY)).toMatchObject({
      dateOfBirth: '2000-02-29',
    })
  })

  it('lets the user correct the century, but not the day', () => {
    const id = withCheck('250101580008')
    expect(checkSaId(id, TODAY)).toMatchObject({ dateOfBirth: '2025-01-01' })
    expect(dobMatchesSaId('1925-01-01', id)).toBe(true)
    expect(dobMatchesSaId('1925-01-02', id)).toBe(false)
    expect(dobMatchesSaId('', id)).toBe(false)
  })

  it('Luhn catches any single-digit typo', () => {
    const good = '8001015009087'
    expect(luhnValid(good)).toBe(true)
    for (let i = 0; i < good.length; i++) {
      const bad =
        good.slice(0, i) + ((Number(good[i]) + 1) % 10) + good.slice(i + 1)
      expect(luhnValid(bad)).toBe(false)
    }
  })
})
