/** Display helpers for patient data. Pure functions; never log their input. */
import type { FileMode, IdType, Sex } from './types'

const dateFormat = new Intl.DateTimeFormat('en-ZA', {
  day: '2-digit',
  month: 'short',
  year: 'numeric',
  timeZone: 'UTC',
})

/** "1979-03-02" → "02 Mar 1979". Dates are calendar dates, so format in UTC. */
export function formatDate(iso: string | null | undefined): string {
  if (!iso) return '—'
  const d = new Date(`${iso.slice(0, 10)}T00:00:00Z`)
  return Number.isNaN(d.getTime()) ? '—' : dateFormat.format(d)
}

/** "Mokoena, Naledi" — surname first for scanning a sorted list. */
export function listName(p: { last_name: string; first_name: string }): string {
  return `${p.last_name}, ${p.first_name}`
}

/** "Dr Naledi Mokoena". */
export function fullName(p: {
  title?: string | null
  first_name: string
  last_name: string
}): string {
  return [p.title, p.first_name, p.last_name].filter(Boolean).join(' ')
}

/** "Discovery Health · Classic", or "Private" when there's no medical aid. */
export function medicalAidLabel(
  scheme: string | null | undefined,
  plan: string | null | undefined,
): string {
  if (!scheme) return 'Private'
  return plan ? `${scheme} · ${plan}` : scheme
}

/** "Showing 26–50 of 132". */
export function pageSummary(
  page: number,
  pageSize: number,
  total: number,
): string {
  if (total === 0) return 'No patients'
  const from = (page - 1) * pageSize + 1
  const to = Math.min(total, page * pageSize)
  return `Showing ${from}–${to} of ${total}`
}

export function pageCount(pageSize: number, total: number): number {
  return Math.max(1, Math.ceil(total / pageSize))
}

export const FILE_MODE_LABELS: Record<FileMode, string> = {
  patient_per_file: 'Patient per file',
  family_per_file: 'Family per file',
  debtor_per_visit: 'New debtor per visit',
}

export const FILE_MODE_HELP: Record<FileMode, string> = {
  patient_per_file: 'One patient per file.',
  family_per_file:
    'A family shares one file: the main member is 00 and dependants are 01, 02 and so on.',
  debtor_per_visit: 'A new file (account) is opened for every visit.',
}

export const SEX_LABELS: Record<Sex, string> = {
  female: 'Female',
  male: 'Male',
  other: 'Other',
  unknown: 'Not recorded',
}

export const ID_TYPE_LABELS: Record<IdType, string> = {
  sa_id: 'South African ID',
  passport: 'Passport',
  other: 'Other document',
}

/** Address lines joined, skipping blanks; null when there's nothing. */
export function formatAddress(p: {
  address_line1: string | null
  address_line2: string | null
  suburb: string | null
  city: string | null
  postal_code: string | null
}): string | null {
  const parts = [
    p.address_line1,
    p.address_line2,
    p.suburb,
    p.city,
    p.postal_code,
  ].filter((x): x is string => Boolean(x && x.trim()))
  return parts.length ? parts.join(', ') : null
}
