import type { ReactNode } from 'react'

/** Plain empty state for sections that aren't built yet. No fake data. */
export function EmptyState({
  title,
  children,
}: {
  title: string
  children?: ReactNode
}) {
  return (
    <section className="mx-auto max-w-2xl py-16">
      <h1 className="text-2xl font-semibold tracking-tight text-text">
        {title}
      </h1>
      {children && <div className="mt-3 text-base text-muted">{children}</div>}
    </section>
  )
}
