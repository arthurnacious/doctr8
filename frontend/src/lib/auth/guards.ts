/**
 * Route guards for `beforeLoad`. They run in the browser only (the auth and
 * app layouts are `ssr: false`), so signed-out users never render app pages.
 */
import { redirect } from '@tanstack/react-router'

import { ensureSession } from './lifecycle'
import { can, getSession } from './session'
import type { PermissionRule } from './permissions'
import type { SignedIn } from './session'

/** Only same-site paths: never redirect to another origin after sign-in. */
export function safeRedirect(target: unknown): string {
  if (typeof target !== 'string') return '/'
  if (
    !target.startsWith('/') ||
    target.startsWith('//') ||
    target.startsWith('/\\')
  )
    return '/'
  return target
}

export interface RedirectSearch {
  redirect?: string
  expired?: boolean
}

export function parseRedirectSearch(
  search: Record<string, unknown>,
): RedirectSearch {
  return {
    redirect: typeof search.redirect === 'string' ? search.redirect : undefined,
    expired:
      search.expired === true || search.expired === 'true' ? true : undefined,
  }
}

/** App routes: signed in, with a practice selected. */
export async function requireSignedIn(location: {
  href: string
}): Promise<SignedIn> {
  const s = await ensureSession()
  if (s.status !== 'signed_in') {
    throw redirect({ to: '/sign-in', search: { redirect: location.href } })
  }
  if (!s.practice) {
    throw redirect({
      to: '/choose-practice',
      search: { redirect: location.href },
    })
  }
  return s
}

/** Sections: the user's permissions must satisfy `rule`, else go home. */
export function requirePermission(rule: PermissionRule | undefined): void {
  if (rule && !can(getSession(), rule)) throw redirect({ to: '/' })
}

/** Sign-in screen: already signed in with a practice? Go to the app. */
export async function redirectIfSignedIn(
  search: RedirectSearch,
): Promise<void> {
  const s = await ensureSession()
  if (s.status === 'signed_in' && s.practice) {
    throw redirect({ href: safeRedirect(search.redirect) })
  }
}
