import { Link } from '@tanstack/react-router'
import { LogOut } from 'lucide-react'

import { IconButton } from '../ui/Button'
import { Logo } from './Logo'
import { PracticeSwitcher } from './PracticeSwitcher'
import { ThemeToggle } from './ThemeToggle'
import { initials } from '#/lib/auth/session'
import { visibleNav } from '#/lib/nav'
import type { SessionState } from '#/lib/auth/session'

interface Props {
  session: SessionState
  onSwitchPractice: (practiceId: string) => void
  onSignOut: () => void
  switching?: boolean
  /** Called after a navigation link is followed (closes the mobile menu). */
  onNavigate?: () => void
}

/** Sidebar contents, shared by the desktop column and the mobile menu. */
export function Sidebar({
  session,
  onSwitchPractice,
  onSignOut,
  switching,
  onNavigate,
}: Props) {
  const signedIn = session.status === 'signed_in' ? session : null
  const items = signedIn?.practice ? visibleNav(signedIn.permissions) : []

  return (
    <div className="flex h-full flex-col gap-4 p-3">
      <div className="flex min-h-11 items-center px-2">
        <Link to="/" onClick={onNavigate} className="rounded-lg">
          <Logo />
        </Link>
      </div>

      {signedIn && (
        <PracticeSwitcher
          current={signedIn.practice}
          practices={signedIn.practices}
          onSwitch={onSwitchPractice}
          busy={switching}
        />
      )}

      <nav aria-label="Main" className="flex-1 overflow-y-auto">
        <ul className="flex flex-col gap-0.5">
          {items.map(({ to, label, icon: Icon }) => (
            <li key={to}>
              <Link
                to={to}
                onClick={onNavigate}
                activeOptions={{ exact: to === '/' }}
                className="flex min-h-11 items-center gap-3 rounded-lg px-3 text-sm font-medium text-muted transition-colors hover:bg-raised hover:text-text data-[status=active]:bg-raised data-[status=active]:text-text"
              >
                <Icon className="size-5 shrink-0" aria-hidden="true" />
                {label}
              </Link>
            </li>
          ))}
        </ul>
      </nav>

      {signedIn && (
        <div className="flex items-center gap-2 border-t border-border pt-3">
          <span
            aria-hidden="true"
            className="inline-flex size-9 shrink-0 items-center justify-center rounded-full bg-raised text-xs font-bold text-text"
          >
            {initials(signedIn.user.full_name)}
          </span>
          <span className="min-w-0 flex-1">
            <span className="block truncate text-sm font-semibold text-text">
              {signedIn.user.full_name}
            </span>
            <span className="block truncate text-xs text-subtle">
              {signedIn.user.email}
            </span>
          </span>
          <ThemeToggle />
          <IconButton label="Sign out" onClick={onSignOut}>
            <LogOut className="size-5" aria-hidden="true" />
          </IconButton>
        </div>
      )}
    </div>
  )
}
