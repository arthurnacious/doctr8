/**
 * Permission keys issued by the API (backend/src/common/permission.rs).
 * Keep this list in sync with the Rust `Permission` enum.
 */
export const PERMISSIONS = [
  'practice.view',
  'practice.manage',
  'members.view',
  'members.invite',
  'members.manage',
  'roles.view',
  'roles.manage',
  'roles.assign',
  'patients.view',
  'patients.manage',
  'accounts.view',
  'accounts.manage',
  'billing.view',
  'billing.manage',
  'payments.manage',
  'journals.manage',
  'providers.manage',
] as const

export type Permission = (typeof PERMISSIONS)[number]

const known = new Set<string>(PERMISSIONS)

/** Keep only keys this build knows about (the API may be newer). */
export function parsePermissions(keys: ReadonlyArray<string>): Set<Permission> {
  return new Set(keys.filter((k): k is Permission => known.has(k)))
}

/** A rule a screen or nav item can require. */
export type PermissionRule =
  { anyOf: ReadonlyArray<Permission> } | { allOf: ReadonlyArray<Permission> }

export function satisfies(
  granted: ReadonlySet<Permission>,
  rule: PermissionRule | undefined,
): boolean {
  if (!rule) return true
  if ('anyOf' in rule) return rule.anyOf.some((p) => granted.has(p))
  return rule.allOf.every((p) => granted.has(p))
}
