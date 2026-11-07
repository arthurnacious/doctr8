import { safeRedirect } from './guards'
import type { NextStep } from './actions'

/** Where to go after a sign-in step, carrying the original destination. */
export function destinationFor(
  step: NextStep,
  redirect: string | undefined,
): { href: string } {
  const search = redirect ? `?redirect=${encodeURIComponent(redirect)}` : ''
  switch (step) {
    case 'two_step':
      return { href: `/two-step${search}` }
    case 'choose_practice':
    case 'no_practice':
      return { href: `/choose-practice${search}` }
    case 'app':
      return { href: safeRedirect(redirect) }
  }
}
