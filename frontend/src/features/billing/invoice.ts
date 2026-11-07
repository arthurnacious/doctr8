/** Invoice wording and small calculations. Pure; unit-tested. */
import type { ClaimStatus, InvoiceLine, Provider } from './types'

export const CLAIM_LABELS: Record<ClaimStatus, string> = {
  submitted: 'Claim submitted',
  accepted: 'Claim accepted',
  rejected: 'Claim rejected',
  paid: 'Claim paid',
}

export function claimLabel(status: ClaimStatus | null | undefined): string {
  return status ? CLAIM_LABELS[status] : 'Not claimed'
}

/** "Settled", "Outstanding" or "In credit", from what's still owed. */
export function dueLabel(medicalAid: number, patient: number): string {
  const total = medicalAid + patient
  if (medicalAid === 0 && patient === 0) return 'Settled'
  return total < 0 ? 'In credit' : 'Outstanding'
}

export type LineState = 'open' | 'part-credited' | 'credited'

export function lineState(
  l: Pick<InvoiceLine, 'amount_cents' | 'credited_cents'>,
): LineState {
  if (l.credited_cents <= 0) return 'open'
  return l.credited_cents >= l.amount_cents ? 'credited' : 'part-credited'
}

/** "Dr A Nkosi", as the backend prints doctors on invoices. */
export function providerName(
  p: Pick<Provider, 'title' | 'first_name' | 'last_name'>,
): string {
  const initial = p.first_name?.trim().charAt(0).toUpperCase()
  return [p.title, initial, p.last_name].filter(Boolean).join(' ')
}

/** "Female" / "Male" for the invoice; anything else isn't printed. */
export function sexLabel(sex: string | null | undefined): string | null {
  if (sex === 'female') return 'Female'
  if (sex === 'male') return 'Male'
  return null
}
