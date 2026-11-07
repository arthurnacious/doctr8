import { Outlet, createFileRoute } from '@tanstack/react-router'

import { requirePermission } from '#/lib/auth/guards'
import { navRule } from '#/lib/nav'

/** Everything under /billing needs billing.view. */
export const Route = createFileRoute('/_app/billing')({
  beforeLoad: () => requirePermission(navRule('/billing')),
  component: Outlet,
})
