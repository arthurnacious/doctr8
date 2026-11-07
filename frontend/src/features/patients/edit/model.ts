/**
 * Editing an existing file: form values, zod schemas and the requests they
 * turn into. Pure functions so they can be tested without rendering.
 */
import { z } from 'zod'

import { normaliseSaId } from '#/lib/sa-id'
import { emptyPerson, personSchema, toPersonInput } from '../new-file/model'
import type { ExistingPerson, PersonDraft } from '../new-file/model'
import type { PersonDetails } from '../types'

const MEMBERSHIP = /^[A-Za-z0-9/-]{1,30}$/
const CODE = /^\d{2}$/

const membershipNumber = z
  .string()
  .refine(
    (n) => MEMBERSHIP.test(n.replace(/\s+/g, '')),
    'Enter the membership number from the card',
  )

export const cleanMembershipNumber = (n: string) =>
  n.replace(/\s+/g, '').toUpperCase()

// -----------------------------------------------------------------------------
// Person details
// -----------------------------------------------------------------------------

export interface EditPersonValues {
  person: PersonDraft
}

export const editPersonSchema = z.object({ person: personSchema })

/** The form's starting values from what the API returned. */
export function personToDraft(p: PersonDetails): PersonDraft {
  const kind =
    p.id_type === 'sa_id'
      ? 'sa_id'
      : p.id_type === 'passport'
        ? 'passport'
        : 'none'
  return {
    ...emptyPerson(),
    title: p.title ?? '',
    first_name: p.first_name,
    last_name: p.last_name,
    date_of_birth: p.date_of_birth ?? '',
    sex: p.sex ?? '',
    id_kind: kind,
    id_number: kind === 'none' ? '' : (p.id_number ?? ''),
    id_country: p.id_country ?? '',
    mobile: p.mobile ?? '',
    email: p.email ?? '',
    address_line1: p.address_line1 ?? '',
    suburb: p.suburb ?? '',
    city: p.city ?? '',
    postal_code: p.postal_code ?? '',
  }
}

/**
 * PUT /persons/{id} replaces the whole record, so fields this form doesn't
 * show (preferred name, home phone, address line 2, an "other" identity
 * document) are carried over from the original instead of being wiped.
 */
export function toPersonUpdate(
  original: PersonDetails,
  draft: PersonDraft,
): Record<string, string> {
  const out = toPersonInput(draft)
  const keep = (k: keyof PersonDetails) => {
    const v = original[k]
    if (typeof v === 'string' && v) out[k] = v
  }
  keep('preferred_name')
  keep('home_phone')
  keep('address_line2')
  if (original.id_type === 'other' && draft.id_kind === 'none') {
    keep('id_type')
    keep('id_number')
    keep('id_country')
  }
  if (out.id_type === 'sa_id') out.id_number = normaliseSaId(draft.id_number)
  return out
}

// -----------------------------------------------------------------------------
// Medical aid
// -----------------------------------------------------------------------------

/** Plan and number on the file's current membership. */
export interface EditMembershipValues {
  plan_id: string
  membership_number: string
}

export const editMembershipSchema = z.object({
  plan_id: z.string().min(1, 'Choose the plan'),
  membership_number: membershipNumber,
})

/** Switch to another medical aid, or make the file private. */
export interface ChangeAidValues {
  kind: 'medical_aid' | 'private'
  scheme_id: string
  plan_id: string
  membership_number: string
}

export const changeAidSchema = z
  .object({
    kind: z.enum(['medical_aid', 'private']),
    scheme_id: z.string(),
    plan_id: z.string(),
    membership_number: z.string(),
  })
  .superRefine((v, ctx) => {
    if (v.kind === 'private') return
    const issue = (path: string, message: string) =>
      ctx.addIssue({ code: 'custom', path: [path], message })
    if (!v.scheme_id) issue('scheme_id', 'Choose the medical scheme')
    if (!v.plan_id) issue('plan_id', 'Choose the plan')
    if (!MEMBERSHIP.test(v.membership_number.replace(/\s+/g, '')))
      issue('membership_number', 'Enter the membership number from the card')
  })

/** PATCH /accounts/{id} body. */
export function changeAidRequest(v: ChangeAidValues) {
  return v.kind === 'private'
    ? { membership_id: null }
    : {
        new_membership: {
          scheme_id: v.scheme_id,
          plan_id: v.plan_id,
          membership_number: cleanMembershipNumber(v.membership_number),
        },
      }
}

// -----------------------------------------------------------------------------
// Adding a dependant to a family file
// -----------------------------------------------------------------------------

export interface AddDependantValues {
  source: 'new' | 'existing'
  person: PersonDraft
  existing: ExistingPerson | null
  code: string
}

/**
 * On a medical aid file the code must come from the card; on a private file
 * the next free one is suggested. 00 is always the main member.
 */
export function addDependantSchema(takenCodes: ReadonlyArray<string>) {
  return z
    .object({
      source: z.enum(['new', 'existing']),
      person: z.any(),
      existing: z.any(),
      code: z.string(),
    })
    .superRefine((v, ctx) => {
      const issue = (path: Array<PropertyKey>, message: string) =>
        ctx.addIssue({ code: 'custom', path, message })
      if (v.source === 'new') {
        const r = personSchema.safeParse(v.person)
        if (!r.success)
          for (const i of r.error.issues)
            issue(['person', ...i.path], i.message)
      } else if (!v.existing) {
        issue(['existing'], 'Search for the person and choose them')
      }
      const code = v.code.trim()
      if (!CODE.test(code)) issue(['code'], 'Enter a 2-digit code, e.g. 01')
      else if (code === '00')
        issue(
          ['code'],
          '00 is the main member. Dependants are 01, 02 and so on.',
        )
      else if (takenCodes.includes(code))
        issue(['code'], `${code} is already used on this file`)
    })
}

/** POST /accounts/{id}/patients body. */
export function addDependantRequest(v: AddDependantValues) {
  const dependant_code = v.code.trim()
  return v.source === 'existing'
    ? { person_id: v.existing?.id ?? '', dependant_code }
    : { person: toPersonInput(v.person), dependant_code }
}
