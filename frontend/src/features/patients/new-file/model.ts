/**
 * The new-file flow: form values, which steps a file mode has, validation
 * per step (zod), and turning it all into one POST /patient-files request so
 * a half-finished file is never saved.
 */
import { z } from 'zod'

import { checkSaId, dobMatchesSaId, normaliseSaId } from '#/lib/sa-id'
import type { FileMode } from '../types'

export type IdKind = 'sa_id' | 'passport' | 'none'

export interface PersonDraft {
  title: string
  first_name: string
  last_name: string
  date_of_birth: string
  sex: '' | 'female' | 'male' | 'other' | 'unknown'
  id_kind: IdKind
  id_number: string
  id_country: string
  mobile: string
  email: string
  address_line1: string
  suburb: string
  city: string
  postal_code: string
}

export interface ExistingPerson {
  id: string
  label: string
}

export interface NewFileValues {
  patient: PersonDraft
  payer: {
    who: 'patient' | 'new' | 'existing'
    person: PersonDraft
    existing: ExistingPerson | null
  }
  aid: {
    kind: 'private' | 'medical_aid'
    scheme_id: string
    plan_id: string
    membership_number: string
    main: 'patient' | 'payer' | 'new' | 'existing'
    person: PersonDraft
    existing: ExistingPerson | null
    /** The patient's code from the card, when they aren't the main member. */
    patient_code: string
  }
  dependants: Array<{ person: PersonDraft; code: string }>
}

export const emptyPerson = (): PersonDraft => ({
  title: '',
  first_name: '',
  last_name: '',
  date_of_birth: '',
  sex: '',
  id_kind: 'sa_id',
  id_number: '',
  id_country: '',
  mobile: '',
  email: '',
  address_line1: '',
  suburb: '',
  city: '',
  postal_code: '',
})

export const defaultValues = (): NewFileValues => ({
  patient: emptyPerson(),
  payer: { who: 'patient', person: emptyPerson(), existing: null },
  aid: {
    kind: 'private',
    scheme_id: '',
    plan_id: '',
    membership_number: '',
    main: 'patient',
    person: emptyPerson(),
    existing: null,
    patient_code: '',
  },
  dependants: [],
})

// -----------------------------------------------------------------------------
// Steps
// -----------------------------------------------------------------------------

export type Step = 'patient' | 'payer' | 'aid' | 'dependants' | 'review'

export const STEP_LABELS: Record<Step, string> = {
  patient: 'Patient',
  payer: 'Who pays',
  aid: 'Medical aid',
  dependants: 'Dependants',
  review: 'Review',
}

/** Dependants only exist on family files. */
export function stepsFor(mode: FileMode): Array<Step> {
  return mode === 'family_per_file'
    ? ['patient', 'payer', 'aid', 'dependants', 'review']
    : ['patient', 'payer', 'aid', 'review']
}

// -----------------------------------------------------------------------------
// Who is who
// -----------------------------------------------------------------------------

type Ref =
  | { kind: 'patient' }
  | { kind: 'new'; person: PersonDraft }
  | { kind: 'existing'; id: string }

export function payerRef(v: NewFileValues): Ref {
  if (v.payer.who === 'patient') return { kind: 'patient' }
  if (v.payer.who === 'existing')
    return { kind: 'existing', id: v.payer.existing?.id ?? '' }
  return { kind: 'new', person: v.payer.person }
}

/**
 * The principal: the medical aid main member, or whoever pays on a private
 * file. They are dependant 00.
 */
export function principalRef(v: NewFileValues): Ref {
  if (v.aid.kind === 'private') return payerRef(v)
  switch (v.aid.main) {
    case 'patient':
      return { kind: 'patient' }
    case 'payer':
      return payerRef(v)
    case 'existing':
      return { kind: 'existing', id: v.aid.existing?.id ?? '' }
    case 'new':
      return { kind: 'new', person: v.aid.person }
  }
}

export function patientIsPrincipal(v: NewFileValues): boolean {
  return principalRef(v).kind === 'patient'
}

/** Next free dependant code ("01".."99") not in `taken`. */
export function nextCode(taken: ReadonlyArray<string>): string {
  for (let n = 1; n <= 99; n++) {
    const c = String(n).padStart(2, '0')
    if (!taken.includes(c)) return c
  }
  return ''
}

// -----------------------------------------------------------------------------
// Validation (zod), per step
// -----------------------------------------------------------------------------

const PHONE = /^\+?[\d\s()-]{7,20}$/
const EMAIL = /^[^\s@]+@[^\s@]+\.[^\s@]+$/
const CODE = /^\d{2}$/

function today(): string {
  return new Date().toISOString().slice(0, 10)
}

/** A person, as entered. `full` adds contact and address checks. */
export const personSchema = z
  .object({
    title: z.string().max(20, 'Keep the title under 20 characters'),
    first_name: z
      .string()
      .trim()
      .min(1, 'Enter a first name')
      .max(100, 'Too long'),
    last_name: z.string().trim().min(1, 'Enter a surname').max(100, 'Too long'),
    date_of_birth: z.string(),
    sex: z.enum(['', 'female', 'male', 'other', 'unknown']),
    id_kind: z.enum(['sa_id', 'passport', 'none']),
    id_number: z.string(),
    id_country: z.string(),
    mobile: z.string(),
    email: z.string(),
    address_line1: z.string().max(200, 'Too long'),
    suburb: z.string().max(100, 'Too long'),
    city: z.string().max(100, 'Too long'),
    postal_code: z.string(),
  })
  .superRefine((p, ctx) => {
    const issue = (path: keyof PersonDraft, message: string) =>
      ctx.addIssue({ code: 'custom', path: [path], message })

    if (p.date_of_birth) {
      if (!/^\d{4}-\d{2}-\d{2}$/.test(p.date_of_birth))
        issue('date_of_birth', 'Enter a valid date')
      else if (p.date_of_birth > today())
        issue('date_of_birth', "Date of birth can't be in the future")
      else if (p.date_of_birth < '1900-01-01')
        issue('date_of_birth', 'Check the year')
    }

    if (p.id_kind === 'sa_id') {
      if (!p.id_number.trim()) {
        issue(
          'id_number',
          'Enter the SA ID number, or choose passport or no document',
        )
      } else {
        const r = checkSaId(p.id_number)
        if (!r.ok) issue('id_number', r.error)
        else if (
          p.date_of_birth &&
          !dobMatchesSaId(p.date_of_birth, p.id_number)
        ) {
          issue('date_of_birth', "This doesn't match the date in the ID number")
        }
      }
    }
    if (p.id_kind === 'passport') {
      const n = p.id_number.replace(/\s+/g, '')
      if (!/^[A-Za-z0-9]{5,20}$/.test(n))
        issue('id_number', 'A passport number is 5-20 letters or digits')
      if (!/^[A-Za-z]{2}$/.test(p.id_country.trim())) {
        issue('id_country', 'Choose the country that issued the passport')
      }
    }

    if (p.mobile.trim() && !PHONE.test(p.mobile.trim()))
      issue('mobile', 'Enter a valid phone number')
    if (p.email.trim() && !EMAIL.test(p.email.trim()))
      issue('email', 'Enter a valid email address')
    if (p.postal_code.trim() && !/^\d{4}$/.test(p.postal_code.trim())) {
      issue('postal_code', 'A postal code has 4 digits')
    }
  })

const existingSchema = z
  .object({ id: z.string(), label: z.string() })
  .nullable()

function stepSchema(step: Step, mode: FileMode) {
  switch (step) {
    case 'patient':
      return z.object({ patient: personSchema })
    case 'payer':
      return z
        .object({
          payer: z.object({
            who: z.enum(['patient', 'new', 'existing']),
            person: z.any(),
            existing: existingSchema,
          }),
        })
        .superRefine((v, ctx) => {
          if (v.payer.who === 'new') {
            const r = personSchema.safeParse(v.payer.person)
            if (!r.success) {
              for (const i of r.error.issues) {
                ctx.addIssue({
                  code: 'custom',
                  path: ['payer', 'person', ...i.path],
                  message: i.message,
                })
              }
            }
          }
          if (v.payer.who === 'existing' && !v.payer.existing) {
            ctx.addIssue({
              code: 'custom',
              path: ['payer', 'existing'],
              message: 'Choose who pays',
            })
          }
        })
    case 'aid':
      return z
        .object({
          aid: z.any(),
          payer: z.any(),
          patient: z.any(),
          dependants: z.any(),
        })
        .superRefine((raw, ctx) => {
          const v = raw
          const issue = (path: Array<PropertyKey>, message: string) =>
            ctx.addIssue({ code: 'custom', path: ['aid', ...path], message })
          if (v.aid.kind === 'private') return
          if (!v.aid.scheme_id)
            issue(['scheme_id'], 'Choose the medical scheme')
          if (!v.aid.plan_id) issue(['plan_id'], 'Choose the plan')
          const n = v.aid.membership_number.replace(/\s+/g, '')
          if (!/^[A-Za-z0-9/-]{1,30}$/.test(n))
            issue(
              ['membership_number'],
              'Enter the membership number from the card',
            )
          if (v.aid.main === 'payer' && v.payer.who === 'patient') {
            issue(
              ['main'],
              'The patient pays, so choose "The patient" as main member',
            )
          }
          if (v.aid.main === 'new') {
            const r = personSchema.safeParse(v.aid.person)
            if (!r.success)
              for (const i of r.error.issues)
                issue(['person', ...i.path], i.message)
          }
          if (v.aid.main === 'existing' && !v.aid.existing)
            issue(['existing'], 'Choose the main member')
          if (!patientIsPrincipal(v)) {
            if (
              !CODE.test(v.aid.patient_code.trim()) ||
              v.aid.patient_code.trim() === '00'
            ) {
              issue(
                ['patient_code'],
                "Enter the patient's dependant code from the card, e.g. 01",
              )
            }
          }
        })
    case 'dependants':
      // looseObject: the code check needs the other steps' values too.
      return z
        .looseObject({
          dependants: z.array(
            z.object({ person: personSchema, code: z.string() }),
          ),
        })
        .superRefine((v, ctx) => {
          if (mode !== 'family_per_file') return
          const used = new Set(fixedCodes(v as unknown as NewFileValues))
          v.dependants.forEach((d, i) => {
            const c = d.code.trim()
            if (!CODE.test(c) || c === '00') {
              ctx.addIssue({
                code: 'custom',
                path: ['dependants', i, 'code'],
                message: 'Use 01 to 99',
              })
            } else if (used.has(c)) {
              ctx.addIssue({
                code: 'custom',
                path: ['dependants', i, 'code'],
                message: 'Another person already has this code',
              })
            }
            used.add(c)
          })
        })
    case 'review':
      return z.object({})
  }
}

/** Codes already used by the principal and the patient on a family file. */
function fixedCodes(v: NewFileValues): Array<string> {
  return ['00', patientCode(v, 'family_per_file')].filter((c): c is string =>
    Boolean(c),
  )
}

/**
 * Validate one step. Returns field errors keyed by TanStack Form path
 * (`dependants[0].person.first_name`), empty when the step is fine.
 */
export function validateStep(
  step: Step,
  mode: FileMode,
  values: NewFileValues,
): Record<string, string> {
  const r = stepSchema(step, mode).safeParse(values)
  if (r.success) return {}
  const out: Record<string, string> = {}
  for (const issue of r.error.issues) {
    const path = issue.path
      .map((p, i) =>
        typeof p === 'number'
          ? `[${p}]`
          : i === 0
            ? String(p)
            : `.${String(p)}`,
      )
      .join('')
    out[path] ??= issue.message
  }
  return out
}

// -----------------------------------------------------------------------------
// The request
// -----------------------------------------------------------------------------

type PersonInput = Record<string, string>

/** Only the fields that were filled in, normalised for the API. */
export function toPersonInput(p: PersonDraft): PersonInput {
  const out: PersonInput = {
    first_name: p.first_name.trim(),
    last_name: p.last_name.trim(),
  }
  const set = (k: string, v: string) => {
    if (v.trim()) out[k] = v.trim()
  }
  set('title', p.title)
  set('date_of_birth', p.date_of_birth)
  set('sex', p.sex)
  if (p.id_kind === 'sa_id') {
    out.id_type = 'sa_id'
    out.id_number = normaliseSaId(p.id_number)
  } else if (p.id_kind === 'passport') {
    out.id_type = 'passport'
    out.id_number = p.id_number.replace(/\s+/g, '').toUpperCase()
    out.id_country = p.id_country.trim().toUpperCase()
  }
  set('mobile', p.mobile)
  set('email', p.email)
  set('address_line1', p.address_line1)
  set('suburb', p.suburb)
  set('city', p.city)
  set('postal_code', p.postal_code)
  return out
}

/** The patient's dependant code on this file (undefined: let the API choose). */
export function patientCode(
  v: NewFileValues,
  mode: FileMode,
): string | undefined {
  if (patientIsPrincipal(v)) return '00'
  if (v.aid.kind === 'medical_aid') return v.aid.patient_code.trim()
  return mode === 'family_per_file' ? '01' : undefined
}

type Source = { new: PersonInput } | { existing: string }
type Party = { patient: number } | 'guarantor' | Source

function source(ref: Ref): Source {
  if (ref.kind === 'existing') return { existing: ref.id }
  if (ref.kind === 'new') return { new: toPersonInput(ref.person) }
  throw new Error('the patient is not a separate source')
}

export interface CreateFileRequest {
  patients: Array<{ person: Source; dependant_code?: string }>
  guarantor: Party
  medical_aid?:
    | { existing: string }
    | {
        new: {
          scheme_id: string
          plan_id: string
          membership_number: string
          main_member: Party
        }
      }
}

/**
 * Build the one-step create request.
 *
 * - Patient per file / new debtor per visit: one patient. If someone else
 *   pays or is the main member, they're named on the file but not a patient.
 * - Family per file: the principal (main member, or the payer when private)
 *   must be patient 00, so they're added first when they aren't the patient;
 *   then the patient, then dependants.
 */
export function buildFileRequest(
  v: NewFileValues,
  mode: FileMode,
): CreateFileRequest {
  const family = mode === 'family_per_file'
  const principal = principalRef(v)
  const payer = payerRef(v)
  const patients: CreateFileRequest['patients'] = []

  const principalIsSeparatePatient = family && principal.kind !== 'patient'
  if (principalIsSeparatePatient)
    patients.push({ person: source(principal), dependant_code: '00' })

  const patientIndex = patients.length
  const code = patientCode(v, mode)
  patients.push({
    person: { new: toPersonInput(v.patient) },
    ...(code ? { dependant_code: code } : {}),
  })

  if (family) {
    for (const d of v.dependants) {
      patients.push({
        person: { new: toPersonInput(d.person) },
        dependant_code: d.code.trim(),
      })
    }
  }

  const samePerson = (a: Ref, b: Ref) =>
    a.kind === b.kind &&
    (a.kind !== 'new' || (b.kind === 'new' && a.person === b.person)) &&
    (a.kind !== 'existing' || (b.kind === 'existing' && a.id === b.id))

  const partyFor = (ref: Ref): Party => {
    if (ref.kind === 'patient') return { patient: patientIndex }
    if (principalIsSeparatePatient && samePerson(ref, principal))
      return { patient: 0 }
    return source(ref)
  }

  const request: CreateFileRequest = { patients, guarantor: partyFor(payer) }
  if (v.aid.kind === 'medical_aid') {
    const main: Party =
      v.aid.main === 'payer' &&
      payer.kind !== 'patient' &&
      !principalIsSeparatePatient
        ? 'guarantor'
        : partyFor(principal)
    request.medical_aid = {
      new: {
        scheme_id: v.aid.scheme_id,
        plan_id: v.aid.plan_id,
        membership_number: v.aid.membership_number
          .replace(/\s+/g, '')
          .toUpperCase(),
        main_member: main,
      },
    }
  }
  return request
}
