import { useEffect, useState } from 'react'
import { Menu, Search } from 'lucide-react'

import { IconButton } from '../ui/Button'
import { openCommandPalette, paletteShortcutLabel } from '#/lib/command-palette'

export function TopBar({
  onOpenMenu,
  menuOpen,
}: {
  onOpenMenu: () => void
  menuOpen: boolean
}) {
  // The label depends on the platform, which is only known in the browser.
  const [shortcut, setShortcut] = useState('Ctrl K')
  useEffect(() => setShortcut(paletteShortcutLabel()), [])

  return (
    <header className="no-print sticky top-0 z-20 flex min-h-16 items-center gap-2 border-b border-border bg-page px-3 sm:px-6">
      <IconButton
        label="Open menu"
        aria-expanded={menuOpen}
        aria-controls="mobile-nav"
        onClick={onOpenMenu}
        className="lg:hidden"
      >
        <Menu className="size-5" aria-hidden="true" />
      </IconButton>

      <button
        type="button"
        onClick={openCommandPalette}
        aria-keyshortcuts="Control+K Meta+K"
        className="flex min-h-11 w-full max-w-md items-center gap-3 rounded-lg border border-border bg-card px-3 text-left text-sm text-muted transition-colors hover:border-border-strong hover:bg-raised"
      >
        <Search className="size-4 shrink-0" aria-hidden="true" />
        <span className="flex-1">Search</span>
        <kbd className="rounded border border-border-strong px-1.5 py-0.5 font-sans text-xs text-subtle">
          {shortcut}
        </kbd>
      </button>
    </header>
  )
}
