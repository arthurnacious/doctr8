import { useEffect, useRef } from 'react'
import { X } from 'lucide-react'

import { IconButton } from '../ui/Button'
import type { ReactNode } from 'react'

/**
 * The sidebar as a slide-over menu on small screens. Modal: Escape or the
 * backdrop closes it, and focus moves into it on open and back on close.
 */
export function MobileNav({
  open,
  onClose,
  children,
}: {
  open: boolean
  onClose: () => void
  children: ReactNode
}) {
  const closeRef = useRef<HTMLButtonElement>(null)

  useEffect(() => {
    if (!open) return
    const previous = document.activeElement as HTMLElement | null
    closeRef.current?.focus()
    const onKey = (e: KeyboardEvent) => e.key === 'Escape' && onClose()
    document.addEventListener('keydown', onKey)
    document.body.style.overflow = 'hidden'
    return () => {
      document.removeEventListener('keydown', onKey)
      document.body.style.overflow = ''
      previous?.focus()
    }
  }, [open, onClose])

  if (!open) return null

  return (
    <div className="fixed inset-0 z-40 lg:hidden">
      <button
        type="button"
        aria-label="Close menu"
        tabIndex={-1}
        onClick={onClose}
        className="absolute inset-0 bg-page/80"
      />
      <div
        id="mobile-nav"
        role="dialog"
        aria-modal="true"
        aria-label="Navigation"
        className="absolute inset-y-0 left-0 flex w-72 max-w-[85vw] flex-col border-r border-border bg-panel"
      >
        <div className="absolute right-2 top-2">
          <IconButton ref={closeRef} label="Close menu" onClick={onClose}>
            <X className="size-5" aria-hidden="true" />
          </IconButton>
        </div>
        <div className="h-full pt-12">{children}</div>
      </div>
    </div>
  )
}
