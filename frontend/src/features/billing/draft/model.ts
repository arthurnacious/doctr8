/**
 * The invoice being prepared (a draft): form values, how they're stored as
 * draft content, the checks before finalising (zod), warnings that don't
 * block, and backend errors mapped to the line they belong to.
 */
import { z } from 'zod'

// -----------------------------------------------------------------------------
// Context and values
// -----------------------------------------------------------------------------

/** GET /accounts/{id}/billing-context */
export interface BillingContext {
  account_id: string
  account_no: number
  status: 'open' | 'closed'
  medical_aid: {
    plan_id: string
    scheme_name: string
    plan_name: string
    membership_number: string
  } | null
  patients: Array<ContextPatient>
}

export interface ContextPatient {
  patient_id: string
  dependant_code: string
  title: string | null
  first_name: string
  last_name: string
  date_of_birth: string | null
  sex: string | null
  status: 'active' | 'inactive'
}

export interface CodeRef {
  code: string
  description: string
}

export interface Icd10Ref extends CodeRef {
  valid_as_primary: boolean
  sex: 'female' | 'male' | null
  min_age_years: number | null
  max_age_years: number | null
}

/** The rate the price was filled in from. */
export interface PriceRef {
  cents: number
  year: number
  source: 'plan' | 'default'
}

export interface LineItem extends CodeRef {
  kind: 'tariff' | 'nappi'
}

export interface LineValues {
  /** Stable key for React lists (not sent to the backend). */
  key: string
  service_date: string
  item: LineItem | null
  quantity: string
  /** Rands as typed, e.g. "520.00". */
  unit_price: string
  tariff_price: PriceRef | null
  icd10: Array<Icd10Ref>
  modifiers: Array<CodeRef>
  assistant_provider_id: string
}

export interface DraftValues {
  patient_id: string
  invoice_date: string
  treating_provider_id: string
  referring_provider_id: string
  service_centre_id: string
  place_of_service: string
  authorisation_number: string
  lines: Array<LineValues>
}

let keySeq = 0
export const newKey = () =>
  `l${Date.now().toString(36)}${(keySeq++).toString(36)}`

export function emptyLine(serviceDate: string): LineValues {
  return {
    key: newKey(),
    service_date: serviceDate,
    item: null,
    quantity: '1',
    unit_price: '',
    tariff_price: null,
    icd10: [],
    modifiers: [],
    assistant_provider_id: '',
  }
}

export function defaultValues(ctx: BillingContext, today: string): DraftValues {
  const active = ctx.patients.filter((p) => p.status === 'active')
  return {
    patient_id: active.length === 1 ? active[0].patient_id : '',
    invoice_date: today,
    treating_provider_id: '',
    referring_provider_id: '',
    service_centre_id: '',
    place_of_service: '',
    authorisation_number: '',
    lines: [emptyLine(today)],
  }
}

/** Common places of service (2-digit codes on claims). */
export const PLACES_OF_SERVICE: Array<{ code: string; label: string }> = [
  { code: '11', label: '11 · Rooms (office)' },
  { code: '12', label: '12 · Home' },
  { code: '21', label: '21 · Hospital, inpatient' },
  { code: '22', label: '22 · Hospital, outpatient' },
  { code: '23', label: '23 · Emergency unit' },
  { code: '24', label: '24 · Day clinic' },
]

// -----------------------------------------------------------------------------
// Money and dates
// -----------------------------------------------------------------------------

/** "520", "520.5", "1 234,50" → cents; null when it isn't an amount. */
export function parseRand(input: string): number | null {
  const s = input.replace(/[\sR]/g, '').replace(',', '.')
  if (!/^\d+(\.\d{1,2})?$/.test(s)) return null
  const [r, c = ''] = s.split('.')
  return Number(r) * 100 + Number(c.padEnd(2, '0'))
}

/** Cents → "520.00" for an input field. */
export function randInput(cents: number): string {
  return (cents / 100).toFixed(2)
}

/** Full years between a date of birth and a date (both yyyy-mm-dd). */
export function ageOn(dob: string, on: string): number {
  const [by, bm, bd] = dob.split('-').map(Number)
  const [y, m, d] = on.split('-').map(Number)
  return y - by - (m < bm || (m === bm && d < bd) ? 1 : 0)
}

export function lineAmount(l: LineValues): number | null {
  const price = parseRand(l.unit_price)
  const qty = Number(l.quantity)
  if (price === null || !Number.isFinite(qty)) return null
  return Math.round(qty * price)
}

/** The user changed the price from the tariff rate. */
export function priceOverridden(l: LineValues): boolean {
  if (!l.tariff_price) return false
  const p = parseRand(l.unit_price)
  return p !== null && p !== l.tariff_price.cents
}

/** A line nobody has started (e.g. the empty one Enter added). */
export function isBlank(l: LineValues): boolean {
  return !l.item && l.icd10.length === 0 && !l.unit_price.trim()
}

// -----------------------------------------------------------------------------
// Draft content (what's saved) and the invoice request
// -----------------------------------------------------------------------------

const blankToUndefined = (s: string) => s.trim() || undefined

/**
 * Draft content: the invoice request fields the backend finalises, plus the
 * form values under `ui` so descriptions and price notes survive a reload.
 */
export function toContent(v: DraftValues): Record<string, unknown> {
  return {
    patient_id: blankToUndefined(v.patient_id),
    invoice_date: blankToUndefined(v.invoice_date),
    treating_provider_id: blankToUndefined(v.treating_provider_id),
    referring_provider_id: blankToUndefined(v.referring_provider_id),
    service_centre_id: blankToUndefined(v.service_centre_id),
    authorisation_number: blankToUndefined(v.authorisation_number),
    lines: v.lines
      .filter((l) => !isBlank(l))
      .map((l) => ({
        service_date: blankToUndefined(l.service_date),
        tariff_code: l.item?.kind === 'tariff' ? l.item.code : undefined,
        nappi_code: l.item?.kind === 'nappi' ? l.item.code : undefined,
        quantity: l.quantity.trim() || '1',
        unit_price_cents: parseRand(l.unit_price) ?? undefined,
        place_of_service: blankToUndefined(v.place_of_service),
        icd10_codes: l.icd10.map((c) => c.code),
        modifiers: l.modifiers.map((m) => m.code),
        assistant_provider_id: blankToUndefined(l.assistant_provider_id),
      })),
    ui: { version: 1, values: v },
  }
}

/** Form values back from saved content (falls back to defaults). */
export function fromContent(
  content: Record<string, unknown>,
  ctx: BillingContext,
  today: string,
): DraftValues {
  const ui = content.ui as
    { version?: number; values?: DraftValues } | undefined
  const base = defaultValues(ctx, today)
  if (ui?.version !== 1 || !ui.values) return base
  const v = ui.values
  return {
    ...base,
    ...v,
    lines: v.lines.length
      ? v.lines.map((l) => ({ ...emptyLine(v.invoice_date || today), ...l }))
      : [emptyLine(v.invoice_date || today)],
  }
}

// -----------------------------------------------------------------------------
// Checks before finalising (blocking) and warnings (not blocking)
// -----------------------------------------------------------------------------

const AUTH = /^[A-Za-z0-9/-]{1,40}$/
const DATE = /^\d{4}-\d{2}-\d{2}$/

export function finaliseSchema(today: string) {
  return z
    .object({
      patient_id: z.string(),
      invoice_date: z.string(),
      treating_provider_id: z.string(),
      referring_provider_id: z.string(),
      service_centre_id: z.string(),
      place_of_service: z.string(),
      authorisation_number: z.string(),
      lines: z.array(z.any()),
    })
    .superRefine((v, ctx) => {
      const issue = (path: Array<string | number>, message: string) =>
        ctx.addIssue({ code: 'custom', path, message })
      if (!v.patient_id) issue(['patient_id'], 'Choose the patient')
      if (!DATE.test(v.invoice_date))
        issue(['invoice_date'], 'Enter the invoice date')
      else if (v.invoice_date > today)
        issue(['invoice_date'], "The invoice date can't be in the future")
      if (!v.treating_provider_id)
        issue(['treating_provider_id'], 'Choose the treating doctor')
      if (
        v.referring_provider_id &&
        v.referring_provider_id === v.treating_provider_id
      )
        issue(
          ['referring_provider_id'],
          "The referring doctor can't be the treating doctor",
        )
      if (!v.service_centre_id)
        issue(['service_centre_id'], 'Choose the service centre')
      if (!/^\d{2}$/.test(v.place_of_service))
        issue(['place_of_service'], 'Choose the place of service')
      const auth = v.authorisation_number.replace(/\s+/g, '')
      if (auth && !AUTH.test(auth))
        issue(['authorisation_number'], "Up to 40 letters, digits, '/' or '-'")

      const lines = v.lines as Array<LineValues>
      if (lines.every(isBlank)) issue(['lines'], 'Add at least one line')
      lines.forEach((l, i) => {
        if (isBlank(l)) return
        const at = (f: string, m: string) => issue(['lines', i, f], m)
        if (!l.item) at('item', 'Search for a tariff or NAPPI code')
        if (!DATE.test(l.service_date))
          at('service_date', 'Enter the service date')
        else if (DATE.test(v.invoice_date) && l.service_date > v.invoice_date)
          at('service_date', "Can't be after the invoice date")
        const q = Number(l.quantity)
        if (!/^\d+(\.\d{1,3})?$/.test(l.quantity.trim()) || !(q > 0))
          at('quantity', 'More than 0, up to 3 decimals')
        if (parseRand(l.unit_price) === null)
          at('unit_price', 'Enter the price, e.g. 520.00')
        if (l.icd10.length === 0) at('icd10', 'Add at least one ICD-10 code')
        else if (!l.icd10[0].valid_as_primary)
          at('icd10', `${l.icd10[0].code} can't be the primary (first) code`)
        if (
          l.assistant_provider_id &&
          l.assistant_provider_id === v.treating_provider_id
        )
          at(
            'assistant_provider_id',
            "The assistant can't be the treating doctor",
          )
      })
    })
}

/** Things to double-check that don't stop finalising. */
export function lineWarnings(
  l: LineValues,
  patient: ContextPatient | undefined,
): Array<string> {
  const out: Array<string> = []
  for (const [i, c] of l.icd10.entries()) {
    if (i > 0 && !c.valid_as_primary) continue // fine as a secondary code
    if (c.sex) {
      if (!patient?.sex || patient.sex === 'unknown' || patient.sex === 'other')
        out.push(
          `${c.code} is for ${c.sex} patients; the patient's sex isn't recorded.`,
        )
      else if (patient.sex !== c.sex)
        out.push(`${c.code} is for ${c.sex} patients only.`)
    }
    if (c.min_age_years !== null || c.max_age_years !== null) {
      if (!patient?.date_of_birth) {
        out.push(
          `${c.code} has an age range; the patient's date of birth isn't recorded.`,
        )
      } else if (DATE.test(l.service_date)) {
        const age = ageOn(patient.date_of_birth, l.service_date)
        if (
          (c.min_age_years !== null && age < c.min_age_years) ||
          (c.max_age_years !== null && age > c.max_age_years)
        )
          out.push(
            `${c.code} is for ages ${c.min_age_years ?? 0}–${c.max_age_years ?? '…'}; the patient is ${age}.`,
          )
      }
    }
  }
  return out
}

// -----------------------------------------------------------------------------
// Backend errors in plain language, on the right line
// -----------------------------------------------------------------------------

const PLAIN: Array<[RegExp, string]> = [
  [/unknown tariff code/i, "This tariff code isn't in the code list."],
  [/unknown NAPPI code/i, "This NAPPI code isn't in the product list."],
  [
    /unknown or inactive ICD-10 code/i,
    'One of the ICD-10 codes is unknown or no longer valid.',
  ],
  [
    /first ICD-10 code can't be used as a primary/i,
    "The first ICD-10 code can't be used as the primary diagnosis. Put a primary code first.",
  ],
  [
    /unknown or inactive modifier/i,
    'One of the modifiers is unknown or no longer valid.',
  ],
  [
    /assistant can't be the treating provider/i,
    "The assistant can't be the treating doctor.",
  ],
  [
    /place_of_service is required/i,
    'Choose the place of service: this service centre has no default.',
  ],
  [
    /service_date can't be after the invoice date/i,
    "The service date can't be after the invoice date.",
  ],
  [/not on this account/i, 'That patient is not on this file.'],
  [
    /closed account/i,
    "This file's account is closed, so it can't be invoiced.",
  ],
]

export interface MappedError {
  /** Index into the form's lines, when the error is about one line. */
  line?: number
  message: string
}

/**
 * "lines[2]: unknown tariff code" → line index (in the form, skipping blank
 * lines that weren't sent) and plain wording.
 */
export function mapBackendError(
  message: string,
  lines: ReadonlyArray<LineValues>,
): MappedError {
  const m = /^lines\[(\d+)\]:\s*(.*)$/s.exec(message)
  const text = m ? m[2] : message
  const plain =
    PLAIN.find(([re]) => re.test(text))?.[1] ??
    text.replace(/^./, (c) => c.toUpperCase())
  if (!m) return { message: plain }
  const sent = lines.map((l, i) => [l, i] as const).filter(([l]) => !isBlank(l))
  return { line: sent.at(Number(m[1]))?.[1], message: plain }
}
