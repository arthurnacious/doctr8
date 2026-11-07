import { forwardRef, useId, useState } from 'react'
import { Eye, EyeOff } from 'lucide-react'
import type { InputHTMLAttributes, ReactNode } from 'react'

const inputClass =
  'block min-h-11 w-full rounded-lg border border-border bg-card px-3 text-base text-text placeholder:text-subtle transition-colors hover:border-border-strong aria-[invalid=true]:border-danger'

interface FieldProps extends Omit<InputHTMLAttributes<HTMLInputElement>, 'id'> {
  /** Visible label (required: every input has a real label). */
  label: string
  hint?: ReactNode
  error?: string
  /** Element rendered inside the input, on the right (e.g. a toggle). */
  trailing?: ReactNode
}

export const TextField = forwardRef<HTMLInputElement, FieldProps>(
  function TextField(
    { label, hint, error, trailing, className = '', ...rest },
    ref,
  ) {
    const id = useId()
    const hintId = hint ? `${id}-hint` : undefined
    const errorId = error ? `${id}-error` : undefined
    const describedBy = [hintId, errorId].filter(Boolean).join(' ') || undefined

    return (
      <div className={className}>
        <label
          htmlFor={id}
          className="mb-1.5 block text-sm font-medium text-text"
        >
          {label}
        </label>
        <div className="relative">
          <input
            ref={ref}
            id={id}
            aria-invalid={error ? true : undefined}
            aria-describedby={describedBy}
            className={`${inputClass} ${trailing ? 'pr-12' : ''}`}
            {...rest}
          />
          {trailing && (
            <div className="absolute inset-y-0 right-0 flex items-center">
              {trailing}
            </div>
          )}
        </div>
        {hint && (
          <p id={hintId} className="mt-1.5 text-sm text-muted">
            {hint}
          </p>
        )}
        {error && (
          <p id={errorId} className="mt-1.5 text-sm text-danger">
            {error}
          </p>
        )}
      </div>
    )
  },
)

export const PasswordField = forwardRef<
  HTMLInputElement,
  Omit<FieldProps, 'type' | 'trailing'>
>(function PasswordField(props, ref) {
  const [visible, setVisible] = useState(false)
  return (
    <TextField
      ref={ref}
      {...props}
      type={visible ? 'text' : 'password'}
      trailing={
        <button
          type="button"
          aria-label={visible ? 'Hide password' : 'Show password'}
          aria-pressed={visible}
          onClick={() => setVisible((v) => !v)}
          className="inline-flex size-11 items-center justify-center rounded-lg text-muted hover:text-text"
        >
          {visible ? (
            <EyeOff className="size-5" aria-hidden="true" />
          ) : (
            <Eye className="size-5" aria-hidden="true" />
          )}
        </button>
      }
    />
  )
})

export function Checkbox({
  label,
  hint,
  ...rest
}: Omit<InputHTMLAttributes<HTMLInputElement>, 'type' | 'id'> & {
  label: string
  hint?: string
}) {
  const id = useId()
  return (
    <div>
      <label
        htmlFor={id}
        className="flex min-h-11 cursor-pointer items-center gap-3 text-sm text-text"
      >
        <input
          id={id}
          type="checkbox"
          aria-describedby={hint ? `${id}-hint` : undefined}
          className="size-5 shrink-0 cursor-pointer accent-[var(--color-accent)]"
          {...rest}
        />
        {label}
      </label>
      {hint && (
        <p id={`${id}-hint`} className="-mt-1 pl-8 text-sm text-muted">
          {hint}
        </p>
      )}
    </div>
  )
}

/** Errors are text (announced), not colour alone. */
export function Alert({
  tone = 'danger',
  children,
}: {
  tone?: 'danger' | 'info'
  children: ReactNode
}) {
  return tone === 'danger' ? (
    <div
      role="alert"
      className="rounded-lg border border-danger bg-danger-surface px-3 py-2.5 text-sm text-danger"
    >
      {children}
    </div>
  ) : (
    <div
      role="status"
      className="rounded-lg border border-border bg-card px-3 py-2.5 text-sm text-muted"
    >
      {children}
    </div>
  )
}
