import { useCallback, useEffect, useState } from 'react'
import {
  Outlet,
  createFileRoute,
  useLocation,
  useNavigate,
} from '@tanstack/react-router'

import { MobileNav } from '#/components/layout/MobileNav'
import { Sidebar } from '#/components/layout/Sidebar'
import { TopBar } from '#/components/layout/TopBar'
import { signOut, switchPractice } from '#/lib/auth/actions'
import { requireSignedIn } from '#/lib/auth/guards'
import { useSession } from '#/lib/auth/session'
import { useRealtime } from '#/lib/ws/useRealtime'

/** Signed-in app: sidebar (a menu on small screens) and a top bar. */
export const Route = createFileRoute('/_app')({
  // The session only exists in the browser, so guard and render there.
  ssr: false,
  beforeLoad: ({ location }) => requireSignedIn(location),
  component: AppLayout,
})

function AppLayout() {
  const session = useSession()
  const navigate = useNavigate()
  const location = useLocation()
  const [menuOpen, setMenuOpen] = useState(false)
  const [switching, setSwitching] = useState(false)
  const closeMenu = useCallback(() => setMenuOpen(false), [])

  useRealtime(session)

  // The session ended underneath us (refresh failed, or another tab signed
  // out): back to sign in, returning here afterwards if it expired.
  useEffect(() => {
    if (session.status !== 'signed_out') return
    const expired = session.reason === 'expired'
    void navigate({
      to: '/sign-in',
      search: expired ? { redirect: location.href, expired: true } : {},
      replace: true,
    })
  }, [session, navigate, location.href])

  const onSwitchPractice = useCallback(
    (id: string) => {
      setSwitching(true)
      switchPractice(id)
        .then(() => navigate({ to: '/' }))
        .catch(() => {
          // A failed switch leaves the current practice in place; a dead
          // session is handled by the effect above.
        })
        .finally(() => setSwitching(false))
    },
    [navigate],
  )

  const onSignOut = useCallback(() => {
    void signOut()
  }, [])

  if (session.status !== 'signed_in') return null

  const sidebar = (
    <Sidebar
      session={session}
      onSwitchPractice={onSwitchPractice}
      onSignOut={onSignOut}
      switching={switching}
      onNavigate={closeMenu}
    />
  )

  return (
    <div className="min-h-dvh bg-page lg:grid lg:grid-cols-[16rem_1fr]">
      <aside className="sticky top-0 hidden h-dvh border-r border-border bg-panel lg:block">
        {sidebar}
      </aside>
      <MobileNav open={menuOpen} onClose={closeMenu}>
        {sidebar}
      </MobileNav>
      <div className="flex min-w-0 flex-col">
        <TopBar menuOpen={menuOpen} onOpenMenu={() => setMenuOpen(true)} />
        <main className="flex-1 px-4 py-6 sm:px-8">
          <Outlet />
        </main>
      </div>
    </div>
  )
}
