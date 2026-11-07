import { useId, useRef } from 'react'
import type { ReactNode } from 'react'

export interface TabDef<T extends string> {
  id: T
  label: string
}

/**
 * Accessible tabs (WAI-ARIA tabs pattern, automatic activation): one tab is
 * in the tab order; Left/Right/Home/End move between tabs.
 */
export function Tabs<T extends string>({
  label,
  tabs,
  selected,
  onSelect,
  children,
}: {
  label: string
  tabs: ReadonlyArray<TabDef<T>>
  selected: T
  onSelect: (id: T) => void
  children: ReactNode
}) {
  const base = useId()
  const refs = useRef<Array<HTMLButtonElement | null>>([])
  const index = Math.max(
    0,
    tabs.findIndex((t) => t.id === selected),
  )

  function onKeyDown(e: React.KeyboardEvent) {
    const last = tabs.length - 1
    const next =
      e.key === 'ArrowRight'
        ? index === last
          ? 0
          : index + 1
        : e.key === 'ArrowLeft'
          ? index === 0
            ? last
            : index - 1
          : e.key === 'Home'
            ? 0
            : e.key === 'End'
              ? last
              : null
    if (next === null) return
    e.preventDefault()
    onSelect(tabs[next].id)
    refs.current[next]?.focus()
  }

  return (
    <div>
      <div
        role="tablist"
        aria-label={label}
        onKeyDown={onKeyDown}
        className="flex gap-1 overflow-x-auto border-b border-border"
      >
        {tabs.map((t, i) => {
          const active = i === index
          return (
            <button
              key={t.id}
              ref={(el) => {
                refs.current[i] = el
              }}
              type="button"
              role="tab"
              id={`${base}-tab-${t.id}`}
              aria-selected={active}
              aria-controls={`${base}-panel`}
              tabIndex={active ? 0 : -1}
              onClick={() => onSelect(t.id)}
              className={`-mb-px min-h-11 shrink-0 border-b-2 px-4 text-sm font-semibold transition-colors ${
                active
                  ? 'border-accent text-text'
                  : 'border-transparent text-muted hover:text-text'
              }`}
            >
              {t.label}
            </button>
          )
        })}
      </div>
      <div
        role="tabpanel"
        id={`${base}-panel`}
        aria-labelledby={`${base}-tab-${tabs[index].id}`}
        tabIndex={0}
        className="pt-5"
      >
        {children}
      </div>
    </div>
  )
}
