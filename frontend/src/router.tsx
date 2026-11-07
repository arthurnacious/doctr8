import { createRouter as createTanStackRouter } from '@tanstack/react-router'
import { setupRouterSsrQueryIntegration } from '@tanstack/react-router-ssr-query'

import { routeTree } from './routeTree.gen'
import { getContext } from './integrations/tanstack-query/root-provider'
import { initTabSync } from './lib/auth/lifecycle'
import { getSession, subscribe } from './lib/auth/session'

export function getRouter() {
  const context = getContext()

  const router = createTanStackRouter({
    routeTree,
    context,
    scrollRestoration: true,
    defaultPreload: 'intent',
    defaultPreloadStaleTime: 0,
  })

  setupRouterSsrQueryIntegration({ router, queryClient: context.queryClient })

  if (typeof window !== 'undefined') {
    initTabSync()
    // Data belongs to one user in one practice. When either changes (sign
    // out, switching practice, another tab), drop every cached query and
    // re-run route guards so permissions apply immediately.
    let scope = scopeKey()
    subscribe(() => {
      const next = scopeKey()
      if (next === scope) return
      scope = next
      context.queryClient.clear()
      void router.invalidate()
    })
  }

  return router
}

function scopeKey(): string {
  const s = getSession()
  return s.status === 'signed_in'
    ? `${s.user.id}:${s.practice?.id ?? ''}`
    : s.status
}

declare module '@tanstack/react-router' {
  interface Register {
    router: ReturnType<typeof getRouter>
  }
}
