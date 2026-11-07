/** The doctr8 mark: a flat green tile with "d8", optionally with the wordmark. */
export function Logo({ withWordmark = true }: { withWordmark?: boolean }) {
  return (
    <span className="inline-flex items-center gap-2.5">
      <svg
        viewBox="0 0 32 32"
        className="size-8 shrink-0"
        role="img"
        aria-label="doctr8"
      >
        <rect width="32" height="32" rx="8" fill="var(--color-accent)" />
        <text
          x="16"
          y="21.5"
          textAnchor="middle"
          fontFamily="Manrope Variable, system-ui, sans-serif"
          fontSize="15"
          fontWeight="800"
          fill="var(--color-on-accent)"
        >
          d8
        </text>
      </svg>
      {withWordmark && (
        <span
          aria-hidden="true"
          className="text-lg font-bold tracking-tight text-text"
        >
          doctr8
        </span>
      )}
    </span>
  )
}
