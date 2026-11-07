/**
 * ============================================================================
 *  DEVELOPMENT STUB: TWO-STEP VERIFICATION
 *  This is NOT security. It exists so the two-step screen can be built and
 *  tested before the API supports two-step verification.
 * ============================================================================
 *
 * What's wrong with it (on purpose):
 * - The password step already returns real tokens and sets the refresh
 *   cookie; the stub only withholds them from the app until a code is
 *   entered. Reloading the page signs the user in without a code.
 * - The code is checked in the browser against a constant.
 * - "Don't ask again for 30 days" is a localStorage flag, not a signed,
 *   httpOnly device cookie.
 *
 * Replace with the endpoints listed in frontend/README.md ("Missing backend
 * endpoints") and delete this file.
 */

export const TWO_STEP_STUB_ENABLED =
  import.meta.env.VITE_TWO_STEP_STUB === 'true'

/** The only code the stub accepts. */
export const STUB_CODE = '000000'

const TRUST_KEY_PREFIX = 'doctr8.stub.trusted-device.'
const TRUST_DAYS = 30

export type SecondFactor =
  { kind: 'code'; value: string } | { kind: 'recovery'; value: string }

/** Six digits, ignoring spaces. */
export function normaliseCode(input: string): string {
  return input.replace(/\s+/g, '')
}

/** Recovery codes look like ABCD-1234 (case-insensitive, spaces ignored). */
export function normaliseRecoveryCode(input: string): string {
  return input.replace(/\s+/g, '').toUpperCase()
}

export function isWellFormed(factor: SecondFactor): boolean {
  return factor.kind === 'code'
    ? /^\d{6}$/.test(normaliseCode(factor.value))
    : /^[A-Z0-9]{4}-[A-Z0-9]{4}$/.test(normaliseRecoveryCode(factor.value))
}

export function stubVerify(factor: SecondFactor): boolean {
  if (!isWellFormed(factor)) return false
  return factor.kind === 'code'
    ? normaliseCode(factor.value) === STUB_CODE
    : true
}

export function stubDeviceTrusted(
  userId: string,
  now: number = Date.now(),
): boolean {
  try {
    const until = Number(localStorage.getItem(TRUST_KEY_PREFIX + userId))
    return Number.isFinite(until) && until > now
  } catch {
    return false
  }
}

export function stubTrustDevice(
  userId: string,
  now: number = Date.now(),
): void {
  try {
    localStorage.setItem(
      TRUST_KEY_PREFIX + userId,
      String(now + TRUST_DAYS * 86_400_000),
    )
  } catch {
    // Storage blocked: the user will be asked again next time.
  }
}
