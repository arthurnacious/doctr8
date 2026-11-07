import { describe, expect, it } from 'vitest'

import {
  buildFileRequest,
  defaultValues,
  emptyPerson,
  nextCode,
  stepsFor,
  toPersonInput,
  validateStep,
} from './model'
import type { NewFileValues, PersonDraft } from './model'

const person = (
  first: string,
  extra: Partial<PersonDraft> = {},
): PersonDraft => ({
  ...emptyPerson(),
  first_name: first,
  last_name: 'Dube',
  id_kind: 'none',
  ...extra,
})

function values(patch: (v: NewFileValues) => void): NewFileValues {
  const v = defaultValues()
  v.patient = person('Ayanda')
  patch(v)
  return v
}

const aid = { scheme_id: 's1', plan_id: 'p1', membership_number: ' fam 9 ' }

describe('steps per file mode', () => {
  it('only family files have a dependants step', () => {
    expect(stepsFor('patient_per_file')).toEqual([
      'patient',
      'payer',
      'aid',
      'review',
    ])
    expect(stepsFor('debtor_per_visit')).toEqual([
      'patient',
      'payer',
      'aid',
      'review',
    ])
    expect(stepsFor('family_per_file')).toEqual([
      'patient',
      'payer',
      'aid',
      'dependants',
      'review',
    ])
  })
})

describe('person validation', () => {
  it('requires names and a valid SA ID (or another choice)', () => {
    const errors = validateStep(
      'patient',
      'patient_per_file',
      values((v) => (v.patient = emptyPerson())),
    )
    expect(errors['patient.first_name']).toBe('Enter a first name')
    expect(errors['patient.last_name']).toBe('Enter a surname')
    expect(errors['patient.id_number']).toMatch(/SA ID number/)
  })

  it('checks the SA ID checksum and the date of birth against it', () => {
    const bad = validateStep(
      'patient',
      'patient_per_file',
      values((v) => {
        v.patient = person('A', {
          id_kind: 'sa_id',
          id_number: '8001015009088',
        })
      }),
    )
    expect(bad['patient.id_number']).toMatch(/typo/)

    const mismatch = validateStep(
      'patient',
      'patient_per_file',
      values((v) => {
        v.patient = person('A', {
          id_kind: 'sa_id',
          id_number: '8001015009087',
          date_of_birth: '1980-01-02',
        })
      }),
    )
    expect(mismatch['patient.date_of_birth']).toMatch(/doesn't match/)

    const corrected = validateStep(
      'patient',
      'patient_per_file',
      values((v) => {
        v.patient = person('A', {
          id_kind: 'sa_id',
          id_number: '8001015009087',
          date_of_birth: '1880-01-01',
        })
      }),
    )
    expect(corrected['patient.date_of_birth']).toBe('Check the year')

    expect(
      validateStep(
        'patient',
        'patient_per_file',
        values((v) => {
          v.patient = person('A', {
            id_kind: 'sa_id',
            id_number: '800101 5009 087',
            date_of_birth: '1980-01-01',
          })
        }),
      ),
    ).toEqual({})
  })

  it('accepts a passport instead, with its country', () => {
    const noCountry = validateStep(
      'patient',
      'patient_per_file',
      values((v) => {
        v.patient = person('A', { id_kind: 'passport', id_number: 'ab12345' })
      }),
    )
    expect(noCountry['patient.id_country']).toMatch(/country/)
    expect(
      validateStep(
        'patient',
        'patient_per_file',
        values((v) => {
          v.patient = person('A', {
            id_kind: 'passport',
            id_number: 'ab12345',
            id_country: 'zw',
          })
        }),
      ),
    ).toEqual({})
  })

  it('checks contact details only when given', () => {
    const e = validateStep(
      'patient',
      'patient_per_file',
      values((v) => {
        v.patient = person('A', {
          mobile: 'call me',
          email: 'nope',
          postal_code: '123',
          date_of_birth: '2999-01-01',
        })
      }),
    )
    expect(Object.keys(e).sort()).toEqual([
      'patient.date_of_birth',
      'patient.email',
      'patient.mobile',
      'patient.postal_code',
    ])
  })
})

describe('later steps', () => {
  it('someone else paying must be entered or chosen', () => {
    expect(
      validateStep(
        'payer',
        'patient_per_file',
        values((v) => {
          v.payer.who = 'new'
        }),
      )['payer.person.first_name'],
    ).toBe('Enter a first name')
    expect(
      validateStep(
        'payer',
        'patient_per_file',
        values((v) => {
          v.payer.who = 'existing'
        }),
      )['payer.existing'],
    ).toBe('Choose who pays')
  })

  it('medical aid needs scheme, plan, number, and a code for dependants', () => {
    const e = validateStep(
      'aid',
      'patient_per_file',
      values((v) => {
        v.aid.kind = 'medical_aid'
        v.aid.main = 'new'
        v.aid.person = person('Sipho')
      }),
    )
    expect(e['aid.scheme_id']).toBeDefined()
    expect(e['aid.plan_id']).toBeDefined()
    expect(e['aid.membership_number']).toBeDefined()
    expect(e['aid.patient_code']).toMatch(/from the card/)
    expect(
      validateStep(
        'aid',
        'patient_per_file',
        values((v) => (v.aid.kind = 'private')),
      ),
    ).toEqual({})
  })

  it('main member "who pays" needs someone else to be paying', () => {
    expect(
      validateStep(
        'aid',
        'patient_per_file',
        values((v) => {
          v.aid = { ...v.aid, ...aid, kind: 'medical_aid', main: 'payer' }
        }),
      )['aid.main'],
    ).toMatch(/patient pays/)
  })

  it('dependant codes are 01-99, unique, and avoid the principal and patient', () => {
    const e = validateStep(
      'dependants',
      'family_per_file',
      values((v) => {
        v.dependants = [
          { person: person('B'), code: '00' },
          { person: person('C'), code: '02' },
          { person: person('D'), code: '02' },
        ]
      }),
    )
    expect(e['dependants[0].code']).toBe('Use 01 to 99')
    expect(e['dependants[2].code']).toMatch(/already has/)
    expect(e['dependants[1].code']).toBeUndefined()
  })

  it('next free code', () => {
    expect(nextCode(['00'])).toBe('01')
    expect(nextCode(['00', '01', '03'])).toBe('02')
  })
})

describe('building the create request', () => {
  it('patient per file, self-paying, private', () => {
    const r = buildFileRequest(
      values(() => {}),
      'patient_per_file',
    )
    expect(r).toEqual({
      patients: [
        {
          person: { new: { first_name: 'Ayanda', last_name: 'Dube' } },
          dependant_code: '00',
        },
      ],
      guarantor: { patient: 0 },
    })
  })

  it('patient per file, a parent pays privately: the child gets the API default code', () => {
    const r = buildFileRequest(
      values((v) => {
        v.payer = { who: 'new', person: person('Sipho'), existing: null }
      }),
      'patient_per_file',
    )
    expect(r.patients).toEqual([
      { person: { new: { first_name: 'Ayanda', last_name: 'Dube' } } },
    ])
    expect(r.guarantor).toEqual({
      new: { first_name: 'Sipho', last_name: 'Dube' },
    })
  })

  it("patient per file on a parent's medical aid: parent pays and is main member", () => {
    const r = buildFileRequest(
      values((v) => {
        v.payer = { who: 'new', person: person('Sipho'), existing: null }
        v.aid = {
          ...v.aid,
          ...aid,
          kind: 'medical_aid',
          main: 'payer',
          patient_code: '02',
        }
      }),
      'patient_per_file',
    )
    expect(r.patients[0].dependant_code).toBe('02')
    expect(r.medical_aid).toEqual({
      new: {
        scheme_id: 's1',
        plan_id: 'p1',
        membership_number: 'FAM9',
        main_member: 'guarantor',
      },
    })
  })

  it('patient pays, someone already in the practice is main member', () => {
    const r = buildFileRequest(
      values((v) => {
        v.aid = {
          ...v.aid,
          ...aid,
          kind: 'medical_aid',
          main: 'existing',
          existing: { id: 'p-9', label: 'X' },
          patient_code: '01',
        }
      }),
      'debtor_per_visit',
    )
    expect(r.guarantor).toEqual({ patient: 0 })
    expect(
      r.medical_aid && 'new' in r.medical_aid && r.medical_aid.new.main_member,
    ).toEqual({ existing: 'p-9' })
  })

  it('family per file: the main member is added first as 00, then patient and dependants', () => {
    const r = buildFileRequest(
      values((v) => {
        v.payer = { who: 'new', person: person('Sipho'), existing: null }
        v.aid = {
          ...v.aid,
          ...aid,
          kind: 'medical_aid',
          main: 'payer',
          patient_code: '01',
        }
        v.dependants = [{ person: person('Lindi'), code: '02' }]
      }),
      'family_per_file',
    )
    expect(r.patients.map((p) => [p.person, p.dependant_code])).toEqual([
      [{ new: { first_name: 'Sipho', last_name: 'Dube' } }, '00'],
      [{ new: { first_name: 'Ayanda', last_name: 'Dube' } }, '01'],
      [{ new: { first_name: 'Lindi', last_name: 'Dube' } }, '02'],
    ])
    expect(r.guarantor).toEqual({ patient: 0 })
    expect(
      r.medical_aid && 'new' in r.medical_aid && r.medical_aid.new.main_member,
    ).toEqual({ patient: 0 })
  })

  it('family per file, private, patient pays: the patient is 00', () => {
    const r = buildFileRequest(
      values((v) => {
        v.dependants = [{ person: person('Lindi'), code: '01' }]
      }),
      'family_per_file',
    )
    expect(r.patients.map((p) => p.dependant_code)).toEqual(['00', '01'])
    expect(r.guarantor).toEqual({ patient: 0 })
  })

  it('family per file: patient pays but someone else is main member', () => {
    const r = buildFileRequest(
      values((v) => {
        v.aid = {
          ...v.aid,
          ...aid,
          kind: 'medical_aid',
          main: 'new',
          person: person('Gogo'),
          patient_code: '03',
        }
      }),
      'family_per_file',
    )
    expect(r.patients.map((p) => p.dependant_code)).toEqual(['00', '03'])
    expect(r.guarantor).toEqual({ patient: 1 })
    expect(
      r.medical_aid && 'new' in r.medical_aid && r.medical_aid.new.main_member,
    ).toEqual({ patient: 0 })
  })

  it('dependants are ignored outside family files', () => {
    const r = buildFileRequest(
      values((v) => {
        v.dependants = [{ person: person('Lindi'), code: '01' }]
      }),
      'patient_per_file',
    )
    expect(r.patients).toHaveLength(1)
  })

  it('person input sends only what was filled in, normalised', () => {
    expect(
      toPersonInput(
        person('A', {
          id_kind: 'sa_id',
          id_number: '800101 5009 087',
          date_of_birth: '1980-01-01',
          mobile: ' 082 555 1234 ',
        }),
      ),
    ).toEqual({
      first_name: 'A',
      last_name: 'Dube',
      id_type: 'sa_id',
      id_number: '8001015009087',
      date_of_birth: '1980-01-01',
      mobile: '082 555 1234',
    })
    expect(
      toPersonInput(
        person('A', {
          id_kind: 'passport',
          id_number: 'ab 12345',
          id_country: 'zw',
        }),
      ),
    ).toMatchObject({
      id_type: 'passport',
      id_number: 'AB12345',
      id_country: 'ZW',
    })
  })
})
