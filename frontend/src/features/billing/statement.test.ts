import { describe, expect, it } from 'vitest'

import {
  asOfError,
  todayInSA,
  formatQuantity,
  isSettled,
  outstanding,
  providerLabel,
  reversedJournalIds,
  visibleLines,
  visitsOf,
} from './statement'
import type {
  Statement,
  StatementInvoice,
  StatementLine,
  StatementPatient,
} from './types'

const patient = (id: string, code: string): StatementPatient => ({
  patient_id: id,
  person_id: `person-${id}`,
  dependant_code: code,
  title: null,
  first_name: id,
  last_name: 'Test',
  date_of_birth: null,
})

const invoice = (
  over: Partial<StatementInvoice> & { invoice_no: number },
): StatementInvoice => ({
  id: `inv-${over.invoice_no}`,
  invoice_date: '2026-10-01',
  patient_id: 'p0',
  dependant_code: '00',
  treating_provider_id: 'd1',
  referring_provider_id: null,
  treating_provider: 'Dr A Doc',
  referring_provider: null,
  service_centre: 'Rooms',
  authorisation_number: null,
  total_cents: 100_00,
  lines: [],
  transactions: [],
  medical_aid_due_cents: 0,
  patient_due_cents: 0,
  ...over,
})

const line = (
  id: string,
  over: Partial<StatementLine> = {},
): StatementLine => ({
  transaction_id: id,
  date: '2026-10-01',
  kind: 'journal',
  description: 'x',
  medical_aid_cents: 0,
  patient_cents: 0,
  reversed: false,
  reverses_transaction_id: null,
  ...over,
})

describe('visits', () => {
  const patients = [patient('p0', '00'), patient('p1', '01')]

  it('groups by patient and day, newest visit last', () => {
    const v = visitsOf(
      [
        invoice({ invoice_no: 5, invoice_date: '2026-10-20' }),
        invoice({
          invoice_no: 2,
          invoice_date: '2026-09-01',
          patient_id: 'p1',
        }),
        invoice({ invoice_no: 3, invoice_date: '2026-10-20' }),
        invoice({
          invoice_no: 4,
          invoice_date: '2026-10-20',
          patient_id: 'p1',
        }),
      ],
      patients,
    )
    expect(v.map((x) => [x.date, x.patient?.dependant_code])).toEqual([
      ['2026-09-01', '01'],
      ['2026-10-20', '00'],
      ['2026-10-20', '01'],
    ])
    expect(v[1].invoices.map((i) => i.invoice_no)).toEqual([3, 5])
  })

  it('copes with a patient no longer on the file', () => {
    const [v] = visitsOf(
      [invoice({ invoice_no: 1, patient_id: 'gone' })],
      patients,
    )
    expect(v.patient).toBeUndefined()
  })
})

describe('settled and outstanding', () => {
  it('adds both sides', () => {
    const i = invoice({
      invoice_no: 1,
      medical_aid_due_cents: 300,
      patient_due_cents: 50,
    })
    expect(outstanding(i)).toBe(350)
    expect(isSettled(i)).toBe(false)
    expect(isSettled(invoice({ invoice_no: 2 }))).toBe(true)
  })

  it('a credit balance is not settled', () => {
    expect(isSettled(invoice({ invoice_no: 1, patient_due_cents: -10 }))).toBe(
      false,
    )
  })
})

describe('reversed journals', () => {
  const statement = {
    unallocated: [
      line('j1', { reversed: true }),
      line('r1', { kind: 'reversal', reverses_transaction_id: 'j1' }),
    ],
    invoices: [
      invoice({
        invoice_no: 1,
        transactions: [
          line('inv', { kind: 'invoice' }),
          line('pay', { kind: 'patient_payment', reversed: true }),
          line('rpay', { kind: 'reversal', reverses_transaction_id: 'pay' }),
          line('j2'),
        ],
      }),
    ],
  } as unknown as Statement

  it('hides reversed journals and their reversals only', () => {
    const hidden = reversedJournalIds(statement)
    expect([...hidden]).toEqual(['j1'])
    expect(visibleLines(statement.unallocated, hidden)).toEqual([])
    expect(
      visibleLines(statement.invoices[0].transactions, hidden).map(
        (l) => l.transaction_id,
      ),
    ).toEqual(['inv', 'pay', 'rpay', 'j2'])
  })

  it('shows everything with an empty hidden set', () => {
    expect(visibleLines(statement.unallocated, new Set())).toHaveLength(2)
  })
})

describe('labels', () => {
  it('names a doctor with practice and council numbers', () => {
    expect(
      providerLabel(
        {
          id: 'd',
          name: 'Dr A Doc',
          practice_number: '1234567',
          hpcsa_number: 'MP1',
        },
        '?',
      ),
    ).toBe('Dr A Doc · Pr 1234567 · HPCSA MP1')
    expect(
      providerLabel(
        { id: 'd', name: 'Dr B', practice_number: null, hpcsa_number: null },
        '?',
      ),
    ).toBe('Dr B')
    expect(providerLabel(undefined, 'Dr Fallback')).toBe('Dr Fallback')
  })

  it('quantities drop trailing zeros', () => {
    expect(formatQuantity('1.00')).toBe('1')
    expect(formatQuantity('2.5')).toBe('2.5')
  })
})

describe('statement date', () => {
  it('uses the South African date', () => {
    // 23:30 UTC is already the next day in Johannesburg (UTC+2).
    expect(todayInSA(new Date('2026-11-07T23:30:00Z'))).toBe('2026-11-08')
    expect(todayInSA(new Date('2026-11-07T21:59:00Z'))).toBe('2026-11-07')
  })

  it('rejects future and malformed dates', () => {
    expect(asOfError('2026-11-07', '2026-11-07')).toBeNull()
    expect(asOfError('2026-11-08', '2026-11-07')).toMatch(/future/)
    expect(asOfError('', '2026-11-07')).toMatch(/valid date/)
    expect(asOfError('1899-01-01', '2026-11-07')).toMatch(/year/)
  })
})
