import { describe, expect, it } from 'vitest'

import {
  ageOn,
  defaultValues,
  emptyLine,
  finaliseSchema,
  fromContent,
  isBlank,
  lineAmount,
  lineWarnings,
  mapBackendError,
  parseRand,
  priceOverridden,
  toContent,
} from './model'
import type {
  BillingContext,
  ContextPatient,
  DraftValues,
  Icd10Ref,
  LineValues,
} from './model'

const TODAY = '2026-11-07'

const patient = (over: Partial<ContextPatient> = {}): ContextPatient => ({
  patient_id: 'p0',
  dependant_code: '00',
  title: null,
  first_name: 'Lerato',
  last_name: 'Test',
  date_of_birth: '1985-04-12',
  sex: 'female',
  status: 'active',
  ...over,
})

const ctx: BillingContext = {
  account_id: 'a1',
  account_no: 32,
  status: 'open',
  medical_aid: {
    plan_id: 'plan',
    scheme_name: 'S',
    plan_name: 'P',
    membership_number: 'M',
  },
  patients: [patient()],
}

const icd = (code: string, over: Partial<Icd10Ref> = {}): Icd10Ref => ({
  code,
  description: code,
  valid_as_primary: true,
  sex: null,
  min_age_years: null,
  max_age_years: null,
  ...over,
})

const line = (over: Partial<LineValues> = {}): LineValues => ({
  ...emptyLine(TODAY),
  item: { kind: 'tariff', code: '0190', description: 'Consultation' },
  unit_price: '520.00',
  icd10: [icd('J06.9')],
  ...over,
})

const values = (over: Partial<DraftValues> = {}): DraftValues => ({
  ...defaultValues(ctx, TODAY),
  treating_provider_id: 'd1',
  service_centre_id: 'c1',
  place_of_service: '11',
  lines: [line()],
  ...over,
})

const errors = (v: DraftValues) => {
  const r = finaliseSchema(TODAY).safeParse(v)
  return r.success
    ? {}
    : Object.fromEntries(
        r.error.issues.map((i) => [i.path.join('.'), i.message]),
      )
}

describe('money and dates', () => {
  it('parses rands as typed', () => {
    expect(parseRand('520')).toBe(52000)
    expect(parseRand('520.5')).toBe(52050)
    expect(parseRand(' R 1 234,50 ')).toBe(123450)
    expect(parseRand('5.555')).toBeNull()
    expect(parseRand('abc')).toBeNull()
    expect(parseRand('')).toBeNull()
  })

  it('line amounts round like the backend', () => {
    expect(lineAmount(line({ quantity: '1.5', unit_price: '3.33' }))).toBe(500)
    expect(lineAmount(line({ unit_price: 'x' }))).toBeNull()
  })

  it('ages in full years', () => {
    expect(ageOn('1985-04-12', '2026-04-11')).toBe(40)
    expect(ageOn('1985-04-12', '2026-04-12')).toBe(41)
  })
})

describe('defaults', () => {
  it('picks the only active patient', () => {
    expect(defaultValues(ctx, TODAY).patient_id).toBe('p0')
    const two = {
      ...ctx,
      patients: [
        patient(),
        patient({ patient_id: 'p1', dependant_code: '01' }),
      ],
    }
    expect(defaultValues(two, TODAY).patient_id).toBe('')
  })
})

describe('draft content', () => {
  it('holds the request fields and the form values', () => {
    const v = values({
      lines: [
        line({ modifiers: [{ code: '0018', description: 'x' }] }),
        emptyLine(TODAY),
      ],
    })
    const c = toContent(v)
    expect(c).toMatchObject({
      patient_id: 'p0',
      treating_provider_id: 'd1',
      referring_provider_id: undefined,
      lines: [
        {
          tariff_code: '0190',
          nappi_code: undefined,
          unit_price_cents: 52000,
          place_of_service: '11',
          icd10_codes: ['J06.9'],
          modifiers: ['0018'],
        },
      ],
    })
    expect((c.lines as Array<unknown>).length).toBe(1) // blank line not sent
    expect(
      fromContent(
        JSON.parse(JSON.stringify(c)) as Record<string, unknown>,
        ctx,
        TODAY,
      ),
    ).toEqual(v)
  })

  it('NAPPI lines send a NAPPI code', () => {
    const c = toContent(
      values({
        lines: [
          line({
            item: { kind: 'nappi', code: '716541001', description: 'x' },
          }),
        ],
      }),
    )
    expect((c.lines as Array<Record<string, unknown>>)[0]).toMatchObject({
      nappi_code: '716541001',
      tariff_code: undefined,
    })
  })

  it('empty or foreign content gives defaults', () => {
    expect(fromContent({}, ctx, TODAY)).toEqual({
      ...defaultValues(ctx, TODAY),
      lines: [expect.objectContaining({ item: null })],
    })
  })
})

describe('before finalising', () => {
  it('a complete invoice passes', () => {
    expect(errors(values())).toEqual({})
  })

  it('names what is missing', () => {
    const e = errors(
      values({
        patient_id: '',
        treating_provider_id: '',
        service_centre_id: '',
        place_of_service: '',
      }),
    )
    expect(Object.keys(e).sort()).toEqual([
      'patient_id',
      'place_of_service',
      'service_centre_id',
      'treating_provider_id',
    ])
    expect(errors(values({ lines: [emptyLine(TODAY)] }))).toEqual({
      lines: 'Add at least one line',
    })
  })

  it('checks each line, on that line', () => {
    const e = errors(
      values({
        lines: [
          line(),
          line({
            item: null,
            quantity: '0',
            unit_price: 'abc',
            icd10: [],
            service_date: '2026-11-08',
          }),
        ],
      }),
    )
    expect(e['lines.1.item']).toMatch(/tariff or NAPPI/)
    expect(e['lines.1.quantity']).toBeDefined()
    expect(e['lines.1.unit_price']).toBeDefined()
    expect(e['lines.1.icd10']).toMatch(/at least one ICD-10/)
    expect(e['lines.1.service_date']).toMatch(/after the invoice date/)
    expect(e['lines.0.item']).toBeUndefined()
  })

  it('the first ICD-10 code must be valid as primary', () => {
    const e = errors(
      values({
        lines: [
          line({
            icd10: [icd('Y40.0', { valid_as_primary: false }), icd('J06.9')],
          }),
        ],
      }),
    )
    expect(e['lines.0.icd10']).toMatch(/primary/)
    expect(
      errors(
        values({
          lines: [
            line({
              icd10: [icd('J06.9'), icd('Y40.0', { valid_as_primary: false })],
            }),
          ],
        }),
      ),
    ).toEqual({})
  })

  it('no future invoices and no self-referral', () => {
    expect(errors(values({ invoice_date: '2026-11-08' })).invoice_date).toMatch(
      /future/,
    )
    expect(
      errors(values({ referring_provider_id: 'd1' })).referring_provider_id,
    ).toBeDefined()
    expect(
      errors(values({ lines: [line({ assistant_provider_id: 'd1' })] }))[
        'lines.0.assistant_provider_id'
      ],
    ).toBeDefined()
  })
})

describe('warnings', () => {
  it('sex and age limits that do not fit the patient', () => {
    const l = line({
      icd10: [
        icd('O80', { sex: 'female', min_age_years: 12, max_age_years: 55 }),
      ],
    })
    expect(lineWarnings(l, patient())).toEqual([])
    expect(lineWarnings(l, patient({ sex: 'male' }))).toEqual([
      'O80 is for female patients only.',
    ])
    expect(
      lineWarnings(l, patient({ date_of_birth: '1960-01-01' }))[0],
    ).toMatch(/ages 12–55; the patient is 66/)
    expect(
      lineWarnings(l, patient({ sex: null, date_of_birth: null })),
    ).toHaveLength(2)
  })

  it('a changed price is noticed', () => {
    const l = line({
      tariff_price: { cents: 52000, year: 2026, source: 'plan' },
    })
    expect(priceOverridden(l)).toBe(false)
    expect(priceOverridden({ ...l, unit_price: '400' })).toBe(true)
  })

  it('blank lines are ignored', () => {
    expect(isBlank(emptyLine(TODAY))).toBe(true)
    expect(isBlank(line())).toBe(false)
  })
})

describe('backend errors', () => {
  it('land on the right line, in plain language', () => {
    const lines = [line(), emptyLine(TODAY), line()]
    // The blank line isn't sent, so backend line 1 is form line 2.
    expect(mapBackendError('lines[1]: unknown tariff code', lines)).toEqual({
      line: 2,
      message: "This tariff code isn't in the code list.",
    })
    expect(
      mapBackendError('That patient is not on this account', lines),
    ).toEqual({
      message: 'That patient is not on this file.',
    })
    expect(mapBackendError('lines[0]: something new', lines)).toEqual({
      line: 0,
      message: 'Something new',
    })
  })
})
