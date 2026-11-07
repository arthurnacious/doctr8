import { useEffect, useId, useRef, useState } from 'react'
import { Check, ChevronsUpDown } from 'lucide-react'

import type { PracticeSummary } from '#/lib/auth/session'

interface Props {
  current: PracticeSummary | null
  practices: ReadonlyArray<PracticeSummary>
  onSwitch: (practiceId: string) => void
  busy?: boolean
}

/**
 * Shows the current practice. With more than one practice it becomes a
 * menu: Escape or clicking outside closes it, focus returns to the button.
 */
export function PracticeSwitcher({
  current,
  practices,
  onSwitch,
  busy,
}: Props) {
  const [open, setOpen] = useState(false)
  const listId = useId()
  const buttonRef = useRef<HTMLButtonElement>(null)
  const panelRef = useRef<HTMLDivElement>(null)

  useEffect(() => {
    if (!open) return
    panelRef.current?.querySelector<HTMLButtonElement>('button')?.focus()
    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape') {
        setOpen(false)
        buttonRef.current?.focus()
      }
    }
    const onPointer = (e: PointerEvent) => {
      const target = e.target as Node
      if (
        !panelRef.current?.contains(target) &&
        !buttonRef.current?.contains(target)
      ) {
        setOpen(false)
      }
    }
    document.addEventListener('keydown', onKey)
    document.addEventListener('pointerdown', onPointer)
    return () => {
      document.removeEventListener('keydown', onKey)
      document.removeEventListener('pointerdown', onPointer)
    }
  }, [open])

  const name = current?.name ?? 'No practice selected'

  if (practices.length <= 1) {
    return (
      <div className="flex min-h-11 items-center rounded-lg border border-border bg-card px-3">
        <span className="truncate text-sm font-semibold text-text">{name}</span>
      </div>
    )
  }

  return (
    <div className="relative">
      <button
        ref={buttonRef}
        type="button"
        aria-haspopup="true"
        aria-expanded={open}
        aria-controls={listId}
        disabled={busy}
        onClick={() => setOpen((o) => !o)}
        className="flex min-h-11 w-full items-center justify-between gap-2 rounded-lg border border-border bg-card px-3 text-left transition-colors hover:border-border-strong hover:bg-raised disabled:opacity-60"
      >
        <span className="min-w-0">
          <span className="block text-xs text-subtle">Practice</span>
          <span className="block truncate text-sm font-semibold text-text">
            {name}
          </span>
        </span>
        <ChevronsUpDown
          className="size-4 shrink-0 text-muted"
          aria-hidden="true"
        />
      </button>

      {open && (
        <div
          ref={panelRef}
          id={listId}
          className="absolute inset-x-0 top-full z-30 mt-1 rounded-lg border border-border-strong bg-card p-1"
        >
          <p className="px-3 py-2 text-xs font-medium text-subtle">
            Switch practice
          </p>
          <ul>
            {practices.map((p) => {
              const active = p.id === current?.id
              return (
                <li key={p.id}>
                  <button
                    type="button"
                    aria-current={active || undefined}
                    onClick={() => {
                      setOpen(false)
                      if (!active) onSwitch(p.id)
                    }}
                    className="flex min-h-11 w-full items-center justify-between gap-2 rounded-md px-3 text-left text-sm text-text hover:bg-raised"
                  >
                    <span className="truncate">{p.name}</span>
                    {active && (
                      <Check
                        className="size-4 shrink-0 text-accent"
                        aria-hidden="true"
                      />
                    )}
                  </button>
                </li>
              )
            })}
          </ul>
        </div>
      )}
    </div>
  )
}
