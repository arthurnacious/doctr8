import { describe, expect, it } from 'vitest'

import { emptyPerson } from '../new-file/model'
import {
  addDependantRequest,
  addDependantSchema,
  changeAidRequest,
  changeAidSchema,
  editMembershipSchema,
  editPersonSchema,
  personToDraft,
  toPersonUpdate,
} from './model'
import type { PersonDetails } from '../types'

const person = (over: Partial<PersonDetails> = {}): PersonDetails => ({
  id: 'p1',
  title: 'Ms',
  first_name: 'Naledi',
  last_name: 'Mokoena',
  preferred_name: 'Ledi',
  date_of_birth: '1980-01-01',
  sex: 'female',
  id_type: 'sa_id',
  id_number: '8001015009087',
  id_country: null,
  email: null,
  mobile: '0825551234',
  home_phone: '0111234567',
  address_line1: '1 Main Rd',
  address_line2: 'Unit 4',
  suburb: null,
  city: 'Durban',
  postal_code: '4001',
  created_at: '',
  updated_at: '',
  ...over,
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

describe('personToDraft / toPersonUpdate', () => {
  it('round-trips what the form shows', () => {
    const d = personToDraft(person())
    expect(d).toMatchObject({
      first_name: 'Naledi',
      id_kind: 'sa_id',
      id_number: '8001015009087',
      sex: 'female',
      email: '',
      suburb: '',
    })
    expect(editPersonSchema.safeParse({ person: d }).success).toBe(true)
  })

  it('keeps fields the form does not show (PUT replaces the record)', () => {
    const p = person()
    const out = toPersonUpdate(p, { ...personToDraft(p), first_name: 'Nal' })
    expect(out).toMatchObject({
      first_name: 'Nal',
      preferred_name: 'Ledi',
      home_phone: '0111234567',
      address_line2: 'Unit 4',
    })
  })

  it('keeps an "other" identity document unless replaced', () => {
    const p = person({
      id_type: 'other',
      id_number: 'ASYL123',
      id_country: 'CD',
    })
    const d = personToDraft(p)
    expect(d.id_kind).toBe('none')
    expect(toPersonUpdate(p, d)).toMatchObject({
      id_type: 'other',
      id_number: 'ASYL123',
      id_country: 'CD',
    })
    const replaced = toPersonUpdate(p, {
      ...d,
      id_kind: 'passport',
      id_number: 'a1234567',
      id_country: 'zw',
    })
    expect(replaced).toMatchObject({
      id_type: 'passport',
      id_number: 'A1234567',
      id_country: 'ZW',
    })
  })

  it('clears optional fields the user emptied', () => {
    const p = person()
    const out = toPersonUpdate(p, { ...personToDraft(p), mobile: '', city: '' })
    expect(out.mobile).toBeUndefined()
    expect(out.city).toBeUndefined()
  })

  it('normalises the SA ID number', () => {
    const p = person()
    const out = toPersonUpdate(p, {
      ...personToDraft(p),
      id_number: '800101 5009 087',
    })
    expect(out.id_number).toBe('8001015009087')
  })

  it('rejects a bad check digit', () => {
    const d = { ...personToDraft(person()), id_number: '8001015009088' }
    expect(issues(editPersonSchema.safeParse({ person: d }))).toHaveProperty(
      'person.id_number',
    )
  })
})

describe('medical aid', () => {
  it('membership edits need a plan and a sensible number', () => {
    expect(
      editMembershipSchema.safeParse({
        plan_id: 'x',
        membership_number: 'ab 12',
      }).success,
    ).toBe(true)
    const e = issues(
      editMembershipSchema.safeParse({ plan_id: '', membership_number: 'a#1' }),
    )
    expect(Object.keys(e).sort()).toEqual(['membership_number', 'plan_id'])
  })

  it('private needs nothing else', () => {
    const v = {
      kind: 'private' as const,
      scheme_id: '',
      plan_id: '',
      membership_number: '',
    }
    expect(changeAidSchema.safeParse(v).success).toBe(true)
    expect(changeAidRequest(v)).toEqual({ membership_id: null })
  })

  it('a new medical aid needs scheme, plan and number', () => {
    const v = {
      kind: 'medical_aid' as const,
      scheme_id: '',
      plan_id: '',
      membership_number: '',
    }
    expect(Object.keys(issues(changeAidSchema.safeParse(v))).sort()).toEqual([
      'membership_number',
      'plan_id',
      'scheme_id',
    ])
    expect(
      changeAidRequest({
        ...v,
        scheme_id: 's',
        plan_id: 'p',
        membership_number: ' ab 1 ',
      }),
    ).toEqual({
      new_membership: {
        scheme_id: 's',
        plan_id: 'p',
        membership_number: 'AB1',
      },
    })
  })
})

describe('adding a dependant', () => {
  const base = {
    source: 'new' as const,
    person: {
      ...emptyPerson(),
      first_name: 'Kid',
      last_name: 'Dube',
      id_kind: 'none' as const,
    },
    existing: null,
    code: '02',
  }
  const schema = addDependantSchema(['00', '01'])

  it('accepts a new person with a free code', () => {
    expect(schema.safeParse(base).success).toBe(true)
    expect(addDependantRequest(base)).toEqual({
      person: { first_name: 'Kid', last_name: 'Dube' },
      dependant_code: '02',
    })
  })

  it('checks the code', () => {
    const code = (c: string) =>
      issues(schema.safeParse({ ...base, code: c })).code
    expect(code('2')).toMatch(/2-digit/)
    expect(code('00')).toMatch(/main member/)
    expect(code('01')).toMatch(/already used/)
  })

  it('validates the new person under person.*', () => {
    const e = issues(
      schema.safeParse({ ...base, person: { ...base.person, first_name: '' } }),
    )
    expect(e).toHaveProperty('person.first_name')
  })

  it('someone already in the practice must be chosen', () => {
    const v = { ...base, source: 'existing' as const }
    expect(issues(schema.safeParse(v))).toHaveProperty('existing')
    const chosen = { ...v, existing: { id: 'p9', label: 'X' } }
    expect(schema.safeParse(chosen).success).toBe(true)
    expect(addDependantRequest(chosen)).toEqual({
      person_id: 'p9',
      dependant_code: '02',
    })
  })
})
