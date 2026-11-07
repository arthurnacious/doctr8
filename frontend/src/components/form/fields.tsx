/**
 * Field components bound to TanStack Form (via `useFieldContext`). Every
 * input has a real label; errors show once the field has been touched or
 * the step was submitted, and are linked with aria-describedby.
 */
import { useId } from 'react'
import { useStore } from '@tanstack/react-form'

import { TextField } from '../ui/Field'
import { useFieldContext } from './context'
import type { InputHTMLAttributes, ReactNode } from 'react'

function useFieldError(): string | undefined {
  const field = useFieldContext<unknown>()
  const meta = useStore(field.store, (s) => s.meta)
  const submitted = useStore(field.form.store, (s) => s.submissionAttempts > 0)
  if (!meta.isTouched && !submitted) return undefined
  const first: unknown = meta.errors.at(0)
  if (!first) return undefined
  if (typeof first === 'string') return first
  if (typeof first === 'object' && 'message' in first)
    return String(first.message)
  return undefined
}

export function TextInput({
  label,
  hint,
  ...rest
}: Omit<
  InputHTMLAttributes<HTMLInputElement>,
  'value' | 'onChange' | 'onBlur' | 'name' | 'id'
> & {
  label: string
  hint?: ReactNode
}) {
  const field = useFieldContext<string>()
  const value = useStore(field.store, (s) => s.value)
  return (
    <TextField
      label={label}
      hint={hint}
      error={useFieldError()}
      name={field.name}
      value={value}
      onChange={(e) => field.handleChange(e.target.value)}
      onBlur={field.handleBlur}
      {...rest}
    />
  )
}

export function SelectInput({
  label,
  options,
  placeholder,
  hint,
  disabled,
}: {
  label: string
  options: ReadonlyArray<{ value: string; label: string }>
  placeholder?: string
  hint?: string
  disabled?: boolean
}) {
  const field = useFieldContext<string>()
  const value = useStore(field.store, (s) => s.value)
  const error = useFieldError()
  const id = useId()
  const describedBy =
    [hint && `${id}-hint`, error && `${id}-error`].filter(Boolean).join(' ') ||
    undefined
  return (
    <div>
      <label
        htmlFor={id}
        className="mb-1.5 block text-sm font-medium text-text"
      >
        {label}
      </label>
      <select
        id={id}
        name={field.name}
        value={value}
        disabled={disabled}
        aria-invalid={error ? true : undefined}
        aria-describedby={describedBy}
        onChange={(e) => field.handleChange(e.target.value)}
        onBlur={field.handleBlur}
        className="block min-h-11 w-full rounded-lg border border-border bg-card px-3 text-base text-text hover:border-border-strong disabled:opacity-60 aria-[invalid=true]:border-danger"
      >
        {placeholder !== undefined && <option value="">{placeholder}</option>}
        {options.map((o) => (
          <option key={o.value} value={o.value}>
            {o.label}
          </option>
        ))}
      </select>
      {hint && (
        <p id={`${id}-hint`} className="mt-1.5 text-sm text-muted">
          {hint}
        </p>
      )}
      {error && (
        <p id={`${id}-error`} className="mt-1.5 text-sm text-danger">
          {error}
        </p>
      )}
    </div>
  )
}

export function RadioGroupInput<T extends string>({
  legend,
  options,
  hint,
}: {
  legend: string
  options: ReadonlyArray<{ value: T; label: string; hint?: string }>
  hint?: string
}) {
  const field = useFieldContext<T>()
  const value = useStore(field.store, (s) => s.value)
  const error = useFieldError()
  const id = useId()
  return (
    <fieldset
      aria-describedby={error ? `${id}-error` : hint ? `${id}-hint` : undefined}
    >
      <legend className="mb-1.5 text-sm font-medium text-text">{legend}</legend>
      {hint && (
        <p id={`${id}-hint`} className="mb-2 text-sm text-muted">
          {hint}
        </p>
      )}
      <div className="flex flex-col gap-1.5">
        {options.map((o) => (
          <label
            key={o.value}
            className="flex min-h-11 cursor-pointer items-start gap-3 rounded-lg border border-border bg-card px-3 py-2.5 text-sm text-text hover:border-border-strong has-[:checked]:border-accent"
          >
            <input
              type="radio"
              name={field.name}
              value={o.value}
              checked={value === o.value}
              onChange={() => field.handleChange(o.value)}
              onBlur={field.handleBlur}
              className="mt-0.5 size-5 shrink-0 accent-[var(--color-accent)]"
            />
            <span>
              <span className="block font-medium">{o.label}</span>
              {o.hint && (
                <span className="mt-0.5 block text-muted">{o.hint}</span>
              )}
            </span>
          </label>
        ))}
      </div>
      {error && (
        <p id={`${id}-error`} className="mt-1.5 text-sm text-danger">
          {error}
        </p>
      )}
    </fieldset>
  )
}
