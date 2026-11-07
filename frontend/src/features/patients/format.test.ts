import { describe, expect, it } from 'vitest'

import {
  formatDate,
  fullName,
  listName,
  medicalAidLabel,
  pageCount,
  pageSummary,
  formatAddress,
} from './format'
import { nextRowIndex } from './keyboard'

describe('patient formatting', () => {
  it('formats calendar dates without timezone drift', () => {
    expect(formatDate('1979-03-02')).toBe('02 Mar 1979')
    expect(formatDate('2000-01-01T00:00:00Z')).toBe('01 Jan 2000')
    expect(formatDate(null)).toBe('—')
    expect(formatDate('nonsense')).toBe('—')
  })

  it('names', () => {
    expect(listName({ first_name: 'Naledi', last_name: 'Mokoena' })).toBe(
      'Mokoena, Naledi',
    )
    expect(
      fullName({ title: 'Dr', first_name: 'Naledi', last_name: 'Mokoena' }),
    ).toBe('Dr Naledi Mokoena')
    expect(
      fullName({ title: null, first_name: 'Naledi', last_name: 'Mokoena' }),
    ).toBe('Naledi Mokoena')
  })

  it('medical aid', () => {
    expect(medicalAidLabel('Test Health', 'Classic')).toBe(
      'Test Health · Classic',
    )
    expect(medicalAidLabel(null, null)).toBe('Private')
  })

  it('pages', () => {
    expect(pageSummary(1, 25, 0)).toBe('No patients')
    expect(pageSummary(1, 25, 10)).toBe('Showing 1–10 of 10')
    expect(pageSummary(2, 25, 132)).toBe('Showing 26–50 of 132')
    expect(pageSummary(6, 25, 132)).toBe('Showing 126–132 of 132')
    expect(pageCount(25, 0)).toBe(1)
    expect(pageCount(25, 132)).toBe(6)
  })
})

describe('addresses', () => {
  it('joins the parts that are present', () => {
    const blank = {
      address_line1: null,
      address_line2: null,
      suburb: null,
      city: null,
      postal_code: null,
    }
    expect(formatAddress(blank)).toBeNull()
    expect(
      formatAddress({
        ...blank,
        address_line1: '12 Long St',
        city: 'Durban',
        postal_code: '4001',
      }),
    ).toBe('12 Long St, Durban, 4001')
    expect(formatAddress({ ...blank, suburb: '  ' })).toBeNull()
  })
})

describe('row keyboard navigation', () => {
  it('moves within bounds', () => {
    expect(nextRowIndex('ArrowDown', 0, 3)).toBe(1)
    expect(nextRowIndex('ArrowDown', 2, 3)).toBe(2)
    expect(nextRowIndex('ArrowUp', 0, 3)).toBe(0)
    expect(nextRowIndex('Home', 2, 3)).toBe(0)
    expect(nextRowIndex('End', 0, 3)).toBe(2)
    expect(nextRowIndex('a', 0, 3)).toBeNull()
    expect(nextRowIndex('ArrowDown', 0, 0)).toBeNull()
  })
})
