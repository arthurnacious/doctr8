import { Outlet, createFileRoute } from '@tanstack/react-router'

import { Logo } from '#/components/layout/Logo'
import { ThemeToggle } from '#/components/layout/ThemeToggle'

/** Sign-in screens: centred content, no sidebar, the mark in the corner. */
export const Route = createFileRoute('/_auth')({
  // Auth state only exists in the browser (the token is in memory).
  ssr: false,
  component: AuthLayout,
})

function AuthLayout() {
  return (
    <div className="relative min-h-dvh bg-page">
      <header className="absolute inset-x-0 top-0 flex items-center justify-between p-4 sm:p-6">
        <Logo />
        <ThemeToggle />
      </header>
      <main className="flex min-h-dvh items-center justify-center px-4 py-24">
        <div className="w-full max-w-sm">
          <Outlet />
        </div>
      </main>
    </div>
  )
}
