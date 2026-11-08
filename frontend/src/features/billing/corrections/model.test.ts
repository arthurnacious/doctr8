import { describe, expect, it } from 'vitest'

import {
  creditNoteDefaults,
  creditNoteRequest,
  creditNoteSchema,
  journalRequest,
  journalSchema,
  paymentRequest,
  paymentSchema,
  transferRequest,
  transferSchema,
} from './model'
import type { InvoiceLine } from '../types'

const line = (id: string, amount: number, credited = 0): InvoiceLine => ({
  id,
  line_no: 1,
  service_date: '2026-11-07',
  tariff_code: '0190',
  nappi_code: null,
  description: 'x',
  quantity: '1',
  unit_price_cents: amount,
  amount_cents: amount,
  credited_cents: credited,
  place_of_service: '11',
  icd10_codes: ['J06.9'],
  modifiers: [],
  assistant_provider_id: null,
})

const issues = (r: {
  success: boolean
  error?: { issues: Array<{ path: Array<PropertyKey>; message: string }> }
}) =>
  Object.fromEntries(
    (r.error?.issues ?? []).map((i) => [
      i.path.map(String).join('.'),
      i.message,
    ]),
  )

describe('credit notes', () => {
  const lines = [
    line('a', 52000),
    line('b', 2500, 2500),
    line('c', 10000, 4000),
  ]

  it('offers only lines with something left to credit', () => {
    const d = creditNoteDefaults(lines)
    expect(d.lines.map((l) => [l.invoice_line_id, l.amount])).toEqual([
      ['a', '520.00'],
      ['c', '60.00'],
    ])
  })

  it('needs a reason and at least one line', () => {
    expect(
      Object.keys(
        issues(creditNoteSchema(lines).safeParse(creditNoteDefaults(lines))),
      ).sort(),
    ).toEqual(['lines', 'reason'])
  })

  it("can't credit more than is left on a line", () => {
    const v = creditNoteDefaults(lines)
    v.reason = 'Not dispensed'
    v.lines[1] = { ...v.lines[1], selected: true, amount: '70' }
    expect(
      issues(creditNoteSchema(lines).safeParse(v))['lines.1.amount'],
    ).toMatch(/At most 60\.00/)
  })

  it('whole lines send no amount; part credits send theirs', () => {
    const v = creditNoteDefaults(lines)
    v.reason = '  Not dispensed '
    v.lines[0].selected = true
    v.lines[1] = { ...v.lines[1], selected: true, amount: '25' }
    expect(creditNoteSchema(lines).safeParse(v).success).toBe(true)
    expect(creditNoteRequest(v, lines)).toEqual({
      reason: 'Not dispensed',
      lines: [
        { invoice_line_id: 'a' },
        { invoice_line_id: 'c', amount_cents: 2500 },
      ],
    })
  })
})

describe('payments', () => {
  const base = {
    method: 'cash' as const,
    amount: '100',
    reference: '',
    txn_date: '2026-11-07',
    allocate: 'auto' as const,
  }

  it('checks amount and date', () => {
    const s = paymentSchema('2026-11-07')
    expect(s.safeParse(base).success).toBe(true)
    expect(
      Object.keys(
        issues(s.safeParse({ ...base, amount: '0', txn_date: '2026-11-08' })),
      ).sort(),
    ).toEqual(['amount', 'txn_date'])
  })

  it('pays this invoice first, up to what the patient owes on it', () => {
    const inv = { id: 'i1', patient_due_cents: 3000 }
    expect(paymentRequest({ ...base, allocate: 'invoice' }, 'a1', inv)).toEqual(
      {
        account_id: 'a1',
        method: 'cash',
        amount_cents: 10000,
        reference: undefined,
        txn_date: '2026-11-07',
        allocations: [{ invoice_id: 'i1', amount_cents: 3000 }],
      },
    )
    expect(paymentRequest(base, 'a1', inv).allocations).toBeUndefined()
    expect(
      paymentRequest({ ...base, allocate: 'invoice' }, 'a1', {
        id: 'i1',
        patient_due_cents: 0,
      }).allocations,
    ).toBeUndefined()
  })
})

describe('liability moves', () => {
  const s = transferSchema({ medical_aid: 9000, patient: 0 })

  it('needs a reason and stays within what that side owes', () => {
    expect(
      issues(s.safeParse({ from: 'medical_aid', amount: '90', notes: '' })),
    ).toHaveProperty('notes')
    expect(
      issues(
        s.safeParse({ from: 'medical_aid', amount: '91', notes: 'co-payment' }),
      ).amount,
    ).toMatch(/owes only 90\.00/)
    expect(
      issues(s.safeParse({ from: 'patient', amount: '1', notes: 'x' })).amount,
    ).toMatch(/patient owes only 0\.00/)
    expect(
      transferRequest(
        { from: 'medical_aid', amount: '90', notes: ' co-payment ' },
        'i1',
      ),
    ).toEqual({
      invoice_id: 'i1',
      from: 'medical_aid',
      amount_cents: 9000,
      notes: 'co-payment',
    })
  })
})

describe('journals', () => {
  it('reduce is a negative entry, increase positive', () => {
    const v = {
      reason_type_id: 'r',
      bucket: 'patient' as const,
      direction: 'reduce' as const,
      amount: '5',
      notes: 'small balance',
    }
    expect(journalSchema.safeParse(v).success).toBe(true)
    expect(journalRequest(v, 'a1', 'i1').entries).toEqual([
      { invoice_id: 'i1', bucket: 'patient', amount_cents: -500 },
    ])
    expect(
      journalRequest({ ...v, direction: 'increase' }, 'a1', 'i1').entries[0]
        .amount_cents,
    ).toBe(500)
    expect(
      Object.keys(
        issues(
          journalSchema.safeParse({ ...v, reason_type_id: '', notes: ' ' }),
        ),
      ).sort(),
    ).toEqual(['notes', 'reason_type_id'])
  })
})
