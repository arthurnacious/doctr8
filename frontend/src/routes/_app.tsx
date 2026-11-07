import { useCallback, useState } from 'react'
import { Outlet, createFileRoute } from '@tanstack/react-router'

import { MobileNav } from '#/components/layout/MobileNav'
import { Sidebar } from '#/components/layout/Sidebar'
import { TopBar } from '#/components/layout/TopBar'
import { signOut, switchPractice } from '#/lib/auth/actions'
import { useSession } from '#/lib/auth/session'

/** Signed-in app: sidebar (a menu on small screens) and a top bar. */
export const Route = createFileRoute('/_app')({
  ssr: false,
  component: AppLayout,
})

function AppLayout() {
  const session = useSession()
  const [menuOpen, setMenuOpen] = useState(false)
  const [switching, setSwitching] = useState(false)
  const closeMenu = useCallback(() => setMenuOpen(false), [])

  const onSwitchPractice = useCallback((id: string) => {
    setSwitching(true)
    switchPractice(id).finally(() => setSwitching(false))
  }, [])

  const sidebar = (
    <Sidebar
      session={session}
      onSwitchPractice={onSwitchPractice}
      onSignOut={signOut}
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
