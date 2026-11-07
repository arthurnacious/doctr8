import { forwardRef } from 'react'
import type { ButtonHTMLAttributes } from 'react'

type Variant = 'primary' | 'secondary' | 'ghost' | 'danger'

const base =
  'inline-flex min-h-11 items-center justify-center gap-2 rounded-lg px-4 text-sm font-semibold transition-colors disabled:cursor-not-allowed disabled:opacity-60'

const variants: Record<Variant, string> = {
  primary: 'bg-accent text-on-accent hover:bg-accent-hover',
  secondary:
    'border border-border bg-card text-text hover:border-border-strong hover:bg-raised',
  ghost: 'text-text hover:bg-raised',
  danger:
    'border border-border bg-card text-danger hover:border-border-strong hover:bg-raised',
}

export interface ButtonProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  variant?: Variant
  /** Shows a busy state and disables the button. */
  busy?: boolean
  fullWidth?: boolean
}

export const Button = forwardRef<HTMLButtonElement, ButtonProps>(
  function Button(
    {
      variant = 'primary',
      busy = false,
      fullWidth = false,
      className = '',
      disabled,
      children,
      type = 'button',
      ...rest
    },
    ref,
  ) {
    return (
      <button
        ref={ref}
        type={type}
        disabled={disabled || busy}
        aria-busy={busy || undefined}
        className={`${base} ${variants[variant]} ${fullWidth ? 'w-full' : ''} ${className}`}
        {...rest}
      >
        {children}
      </button>
    )
  },
)

/** Square 44px button for a single icon. `label` is required (screen readers). */
export const IconButton = forwardRef<
  HTMLButtonElement,
  Omit<ButtonHTMLAttributes<HTMLButtonElement>, 'aria-label'> & {
    label: string
  }
>(function IconButton({ label, className = '', children, ...rest }, ref) {
  return (
    <button
      ref={ref}
      type="button"
      aria-label={label}
      title={label}
      className={`inline-flex size-11 shrink-0 items-center justify-center rounded-lg text-muted transition-colors hover:bg-raised hover:text-text ${className}`}
      {...rest}
    >
      {children}
    </button>
  )
})
