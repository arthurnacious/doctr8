import {
  CalendarDays,
  ClipboardCheck,
  LayoutDashboard,
  Landmark,
  ReceiptText,
  Settings,
  Users,
} from 'lucide-react'
import type { LucideIcon } from 'lucide-react'

import { satisfies } from './auth/permissions'
import type { Permission, PermissionRule } from './auth/permissions'

export interface NavItem {
  label: string
  to:
    | '/'
    | '/diary'
    | '/patients'
    | '/billing'
    | '/claims'
    | '/finance'
    | '/settings'
  icon: LucideIcon
  /** Hidden (and the route guarded) unless the user's permissions satisfy this. */
  requires?: PermissionRule
}

/**
 * Sidebar navigation. A section's route uses the same rule in its
 * beforeLoad, so hiding a link is never the only protection.
 */
export const NAV_ITEMS: ReadonlyArray<NavItem> = [
  { label: 'Dashboard', to: '/', icon: LayoutDashboard },
  {
    label: 'Diary',
    to: '/diary',
    icon: CalendarDays,
    // No booking permissions yet; the diary shows patients.
    requires: { anyOf: ['patients.view'] },
  },
  {
    label: 'Patients',
    to: '/patients',
    icon: Users,
    requires: { anyOf: ['patients.view'] },
  },
  {
    label: 'Billing',
    to: '/billing',
    icon: ReceiptText,
    requires: { anyOf: ['billing.view'] },
  },
  {
    label: 'Claims',
    to: '/claims',
    icon: ClipboardCheck,
    requires: { anyOf: ['billing.view'] },
  },
  {
    label: 'Finance',
    to: '/finance',
    icon: Landmark,
    requires: { anyOf: ['payments.manage', 'journals.manage'] },
  },
  {
    label: 'Settings',
    to: '/settings',
    icon: Settings,
    requires: {
      anyOf: [
        'practice.view',
        'members.view',
        'roles.view',
        'providers.manage',
      ],
    },
  },
]

export function visibleNav(
  permissions: ReadonlySet<Permission>,
  items: ReadonlyArray<NavItem> = NAV_ITEMS,
): Array<NavItem> {
  return items.filter((item) => satisfies(permissions, item.requires))
}

export function navRule(to: NavItem['to']): PermissionRule | undefined {
  return NAV_ITEMS.find((i) => i.to === to)?.requires
}
