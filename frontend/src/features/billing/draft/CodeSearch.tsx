/**
 * Code search without a mouse: an ARIA combobox. Type a code or words from
 * the description; Arrow keys move, Enter picks, Escape closes. Each option
 * shows the description so the user can confirm the right code.
 *
 * - ItemSearch: tariff or NAPPI code for a line (one).
 * - CodeChips: ICD-10 codes or modifiers (several, in order).
 */
import { forwardRef, useId, useState } from 'react'
import { X } from 'lucide-react'

import { useDebouncedValue } from '#/lib/hooks/useDebouncedValue'
import { useCodeSearch, useModifiers } from './api'
import type { KeyboardEvent, ReactNode } from 'react'
import type { CodeRef, Icd10Ref, LineItem } from './model'

interface Option<T> {
  key: string
  code: string
  description: string
  tag?: string
  value: T
}

interface ComboboxProps<T> {
  label: string
  /** Visually hidden label (the column header says it). */
  hideLabel?: boolean
  query: string
  onQuery: (q: string) => void
  options: ReadonlyArray<Option<T>>
  loading: boolean
  onPick: (value: T) => void
  placeholder?: string
  error?: string
  /** Backspace on an empty input (remove the last chip). */
  onBackspaceEmpty?: () => void
  before?: ReactNode
}

function ComboboxInner<T>(
  {
    label,
    hideLabel = false,
    query,
    onQuery,
    options,
    loading,
    onPick,
    placeholder,
    error,
    onBackspaceEmpty,
    before,
  }: ComboboxProps<T>,
  ref: React.ForwardedRef<HTMLInputElement>,
) {
  const id = useId()
  const [open, setOpen] = useState(false)
  const [active, setActive] = useState(0)
  const show = open && query.trim().length >= 2
  const listId = `${id}-list`
  const optId = (i: number) => `${id}-opt-${i}`

  const pick = (o: Option<T>) => {
    onPick(o.value)
    onQuery('')
    setOpen(false)
    setActive(0)
  }

  const onKeyDown = (e: KeyboardEvent<HTMLInputElement>) => {
    if (e.key === 'ArrowDown') {
      e.preventDefault()
      setOpen(true)
      setActive((a) => Math.min(a + 1, Math.max(options.length - 1, 0)))
    } else if (e.key === 'ArrowUp') {
      e.preventDefault()
      setActive((a) => Math.max(a - 1, 0))
    } else if (e.key === 'Enter') {
      // Enter picks while the list is open; otherwise it bubbles up (the
      // line editor uses it to add a new line).
      if (show && options[active]) {
        e.preventDefault()
        e.stopPropagation()
        pick(options[active])
      } else if (query.trim()) {
        e.preventDefault()
        e.stopPropagation()
      }
    } else if (e.key === 'Escape' && show) {
      e.preventDefault()
      e.stopPropagation()
      setOpen(false)
    } else if (e.key === 'Backspace' && !query && onBackspaceEmpty) {
      onBackspaceEmpty()
    }
  }

  return (
    <div className="relative min-w-0">
      <label
        htmlFor={id}
        className={
          hideLabel ? 'sr-only' : 'mb-1.5 block text-sm font-medium text-text'
        }
      >
        {label}
      </label>
      <div
        className={`flex min-h-11 flex-wrap items-center gap-1 rounded-lg border bg-card px-2 py-1 focus-within:border-border-strong ${
          error ? 'border-danger' : 'border-border'
        }`}
      >
        {before}
        <input
          ref={ref}
          id={id}
          role="combobox"
          aria-expanded={show}
          aria-controls={listId}
          aria-autocomplete="list"
          aria-activedescendant={
            show && options[active] ? optId(active) : undefined
          }
          aria-invalid={error ? true : undefined}
          aria-describedby={error ? `${id}-error` : undefined}
          autoComplete="off"
          spellCheck={false}
          placeholder={placeholder}
          value={query}
          onChange={(e) => {
            onQuery(e.target.value)
            setOpen(true)
            setActive(0)
          }}
          onFocus={() => setOpen(true)}
          onBlur={() => setOpen(false)}
          onKeyDown={onKeyDown}
          className="min-w-[6rem] flex-1 bg-transparent px-1 py-1.5 text-base text-text outline-none placeholder:text-subtle"
        />
      </div>
      <ul
        id={listId}
        role="listbox"
        aria-label={`${label} results`}
        hidden={!show}
        className="absolute z-20 mt-1 max-h-72 w-full min-w-[20rem] overflow-y-auto rounded-lg border border-border-strong bg-card py-1"
      >
        {options.length === 0 ? (
          <li className="px-3 py-2 text-sm text-muted">
            {loading ? 'Searching…' : 'No matches'}
          </li>
        ) : (
          options.map((o, i) => (
            <li
              key={o.key}
              id={optId(i)}
              role="option"
              aria-selected={i === active}
              // mousedown so the pick happens before the input blurs
              onMouseDown={(e) => {
                e.preventDefault()
                pick(o)
              }}
              onMouseEnter={() => setActive(i)}
              className={`flex cursor-pointer gap-3 px-3 py-2 text-sm ${
                i === active ? 'bg-raised' : ''
              }`}
            >
              <span className="shrink-0 font-mono font-semibold text-text">
                {o.code}
              </span>
              <span className="min-w-0 flex-1 text-text">{o.description}</span>
              {o.tag && (
                <span className="shrink-0 text-xs text-muted">{o.tag}</span>
              )}
            </li>
          ))
        )}
      </ul>
      {error && (
        <p id={`${id}-error`} className="mt-1 text-sm text-danger">
          {error}
        </p>
      )}
    </div>
  )
}

const Combobox = forwardRef(ComboboxInner) as <T>(
  p: ComboboxProps<T> & { ref?: React.Ref<HTMLInputElement> },
) => ReturnType<typeof ComboboxInner>

// -----------------------------------------------------------------------------
// Tariff or NAPPI code for a line
// -----------------------------------------------------------------------------

export const ItemSearch = forwardRef<
  HTMLInputElement,
  {
    label: string
    value: LineItem | null
    onPick: (item: LineItem) => void
    onClear: () => void
    error?: string
  }
>(function ItemSearch({ label, value, onPick, onClear, error }, ref) {
  const [query, setQuery] = useState('')
  const q = useDebouncedValue(query, 200)
  const tariffs = useCodeSearch<CodeRef>('tariffs', q)
  const nappi = useCodeSearch<CodeRef>('nappi', q)
  const options: Array<Option<LineItem>> = [
    ...(tariffs.data ?? []).map((t) => ({
      key: `t${t.code}`,
      code: t.code,
      description: t.description,
      tag: 'Tariff',
      value: {
        kind: 'tariff' as const,
        code: t.code,
        description: t.description,
      },
    })),
    ...(nappi.data ?? []).map((n) => ({
      key: `n${n.code}`,
      code: n.code,
      description: n.description,
      tag: 'NAPPI',
      value: {
        kind: 'nappi' as const,
        code: n.code,
        description: n.description,
      },
    })),
  ]
  if (value) {
    return (
      <div className="min-w-0">
        <p className="sr-only">{label}</p>
        <div className="flex min-h-11 items-start gap-2 rounded-lg border border-border bg-card px-3 py-2">
          <div className="min-w-0 flex-1">
            <p className="font-mono text-sm font-semibold text-text">
              {value.code}{' '}
              <span className="font-sans text-xs font-normal text-muted">
                {value.kind === 'tariff' ? 'Tariff' : 'NAPPI'}
              </span>
            </p>
            <p className="text-sm text-text">{value.description}</p>
          </div>
          <button
            type="button"
            onClick={onClear}
            aria-label={`Change code ${value.code}`}
            className="inline-flex size-9 shrink-0 items-center justify-center rounded-lg text-muted hover:bg-raised hover:text-text"
          >
            <X className="size-4" aria-hidden="true" />
          </button>
        </div>
      </div>
    )
  }
  return (
    <Combobox<LineItem>
      ref={ref}
      label={label}
      hideLabel
      query={query}
      onQuery={setQuery}
      options={options}
      loading={tariffs.isFetching || nappi.isFetching}
      onPick={onPick}
      placeholder="Tariff or NAPPI code, or words"
      error={error}
    />
  )
})

// -----------------------------------------------------------------------------
// Several codes: ICD-10 or modifiers
// -----------------------------------------------------------------------------

function Chips<T extends CodeRef>({
  values,
  onRemove,
  noun,
}: {
  values: ReadonlyArray<T>
  onRemove: (code: string) => void
  noun: string
}) {
  return (
    <>
      {values.map((v, i) => (
        <span
          key={v.code}
          title={v.description}
          className="inline-flex items-center gap-1 rounded border border-border-strong bg-raised py-0.5 pl-2 pr-0.5 font-mono text-xs text-text"
        >
          {v.code}
          {i === 0 && noun === 'ICD-10' && (
            <span className="sr-only"> (primary)</span>
          )}
          <button
            type="button"
            onClick={() => onRemove(v.code)}
            aria-label={`Remove ${noun} ${v.code} ${v.description}`}
            className="inline-flex size-6 items-center justify-center rounded text-muted hover:text-text"
          >
            <X className="size-3" aria-hidden="true" />
          </button>
        </span>
      ))}
    </>
  )
}

export function Icd10Picker({
  label,
  values,
  onChange,
  error,
}: {
  label: string
  values: ReadonlyArray<Icd10Ref>
  onChange: (v: Array<Icd10Ref>) => void
  error?: string
}) {
  const [query, setQuery] = useState('')
  const q = useDebouncedValue(query, 200)
  const found = useCodeSearch<Icd10Ref>('icd10', q)
  const taken = new Set(values.map((v) => v.code))
  const options = (found.data ?? [])
    .filter((c) => !taken.has(c.code))
    .map((c) => ({
      key: c.code,
      code: c.code,
      description: c.description,
      tag: c.valid_as_primary ? undefined : 'Not primary',
      value: c,
    }))
  return (
    <div className="min-w-0">
      <Combobox<Icd10Ref>
        label={label}
        hideLabel
        query={query}
        onQuery={setQuery}
        options={options}
        loading={found.isFetching}
        onPick={(c) => onChange([...values, c])}
        onBackspaceEmpty={() => onChange(values.slice(0, -1))}
        placeholder={values.length ? 'Add' : 'ICD-10 code or words'}
        error={error}
        before={
          <Chips
            values={values}
            noun="ICD-10"
            onRemove={(code) => onChange(values.filter((v) => v.code !== code))}
          />
        }
      />
      {values.length > 0 && (
        <p className="mt-1 text-xs text-muted">
          {values.map((v) => `${v.code} ${v.description}`).join(' · ')}
        </p>
      )}
    </div>
  )
}

export function ModifierPicker({
  label,
  values,
  onChange,
}: {
  label: string
  values: ReadonlyArray<CodeRef>
  onChange: (v: Array<CodeRef>) => void
}) {
  const [query, setQuery] = useState('')
  const all = useModifiers()
  const taken = new Set(values.map((v) => v.code))
  const q = query.trim().toLowerCase()
  const options = (all.data ?? [])
    .filter(
      (m) =>
        !taken.has(m.code) &&
        (m.code.startsWith(q) || m.description.toLowerCase().includes(q)),
    )
    .slice(0, 12)
    .map((m) => ({
      key: m.code,
      code: m.code,
      description: m.description,
      value: m,
    }))
  return (
    <Combobox<CodeRef>
      label={label}
      hideLabel
      query={query}
      onQuery={setQuery}
      options={options}
      loading={all.isFetching}
      onPick={(m) => onChange([...values, m])}
      onBackspaceEmpty={() => onChange(values.slice(0, -1))}
      placeholder={values.length ? 'Add' : 'Modifier'}
      before={
        <Chips
          values={values}
          noun="modifier"
          onRemove={(code) => onChange(values.filter((v) => v.code !== code))}
        />
      }
    />
  )
}
