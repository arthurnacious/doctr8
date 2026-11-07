/**
 * South African ID numbers: YYMMDD SSSS C A Z (13 digits).
 * Mirrors backend/src/common/validation.rs: a real date of birth, a
 * citizenship digit of 0, 1 or 2, and a Luhn check digit. Messages never
 * repeat the number.
 */

export type SaIdCheck =
  { ok: true; dateOfBirth: string } | { ok: false; error: string }

export function luhnValid(digits: string): boolean {
  let sum = 0
  for (let i = 0; i < digits.length; i++) {
    let d = Number(digits[digits.length - 1 - i])
    if (i % 2 === 1) {
      d *= 2
      if (d > 9) d -= 9
    }
    sum += d
  }
  return sum % 10 === 0
}

function isoDate(y: number, m: number, d: number): string | null {
  const dt = new Date(Date.UTC(y, m - 1, d))
  if (
    dt.getUTCFullYear() !== y ||
    dt.getUTCMonth() !== m - 1 ||
    dt.getUTCDate() !== d
  )
    return null
  return dt.toISOString().slice(0, 10)
}

export function normaliseSaId(input: string): string {
  return input.replace(/\s+/g, '')
}

/**
 * Check an SA ID number and work out the date of birth. The ID only holds
 * a two-digit year, so pick the most recent century that isn't in the
 * future; the user can correct it (see `dobMatchesSaId`).
 */
export function checkSaId(input: string, today: Date = new Date()): SaIdCheck {
  const id = normaliseSaId(input)
  if (!/^\d{13}$/.test(id))
    return { ok: false, error: 'An SA ID number has 13 digits' }
  const yy = Number(id.slice(0, 2))
  const mm = Number(id.slice(2, 4))
  const dd = Number(id.slice(4, 6))
  const todayIso = today.toISOString().slice(0, 10)
  const dob = [2000 + yy, 1900 + yy]
    .map((y) => isoDate(y, mm, dd))
    .find((d): d is string => d !== null && d <= todayIso)
  if (!dob)
    return {
      ok: false,
      error: "The ID number's date of birth isn't a real date",
    }
  if (Number(id[10]) > 2)
    return { ok: false, error: "The ID number's citizenship digit is invalid" }
  if (!luhnValid(id))
    return { ok: false, error: "That ID number isn't valid: check for a typo" }
  return { ok: true, dateOfBirth: dob }
}

/** Does a date of birth (YYYY-MM-DD) agree with the ID's YYMMDD? */
export function dobMatchesSaId(dob: string, id: string): boolean {
  const n = normaliseSaId(id)
  return (
    /^\d{4}-\d{2}-\d{2}$/.test(dob) &&
    dob.slice(2, 4) + dob.slice(5, 7) + dob.slice(8, 10) === n.slice(0, 6)
  )
}
