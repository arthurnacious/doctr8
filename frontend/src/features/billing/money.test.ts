import { describe, expect, it } from 'vitest'

import { formatRand, formatRandOrBlank } from './money'

const plain = (s: string) => s.replaceAll(String.fromCharCode(0x202f), ' ')

describe('formatRand', () => {
  it('always shows two decimals', () => {
    expect(plain(formatRand(0))).toBe('R 0.00')
    expect(plain(formatRand(5))).toBe('R 0.05')
    expect(plain(formatRand(52000))).toBe('R 520.00')
  })

  it('groups thousands', () => {
    expect(plain(formatRand(123456789))).toBe('R 1 234 567.89')
    expect(plain(formatRand(100000))).toBe('R 1 000.00')
  })

  it('marks money in with a minus sign', () => {
    expect(plain(formatRand(-30000))).toBe('-R 300.00')
  })

  it('leaves zero blank when asked', () => {
    expect(formatRandOrBlank(0)).toBe('')
    expect(plain(formatRandOrBlank(1))).toBe('R 0.01')
  })
})
