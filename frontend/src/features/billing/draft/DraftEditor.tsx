/**
 * Preparing an invoice. The draft saves itself as you type (no number, no
 * money yet). Finalising checks everything, asks for confirmation, then
 * posts the invoice, which can't be edited afterwards.
 *
 * Keyboard: Tab moves through fields; Enter in a line adds a new line and
 * puts you in its code search; the code search works with the keyboard.
 */
import { useEffect, useRef, useState } from 'react'
import { useNavigate } from '@tanstack/react-router'
import { revalidateLogic, useStore } from '@tanstack/react-form'
import { AlertTriangle, Plus, Trash2 } from 'lucide-react'

import { firstError, useAppForm } from '#/components/form/form'
import { Button } from '#/components/ui/Button'
import { Alert } from '#/components/ui/Field'
import { ApiError, errorMessage } from '#/lib/api/errors'
import { formatDate, fullName } from '../../patients/format'
import { providerName } from '../invoice'
import { formatRand } from '../money'
import {
  fetchTariffPrice,
  useDiscardDraft,
  useFinaliseDraft,
  useSaveDraft,
} from './api'
import { Icd10Picker, ItemSearch, ModifierPicker } from './CodeSearch'
import {
  PLACES_OF_SERVICE,
  emptyLine,
  finaliseSchema,
  fromContent,
  isBlank,
  lineAmount,
  lineWarnings,
  mapBackendError,
  priceOverridden,
  randInput,
  toContent,
} from './model'
import type { KeyboardEvent } from 'react'
import type { Draft } from './api'
import type {
  BillingContext,
  DraftValues,
  LineValues,
  MappedError,
  PriceRef,
} from './model'
import type { Provider, ServiceCentre } from '../types'

const SAVE_DELAY_MS = 800

export function DraftEditor({
  draft,
  context,
  providers,
  centres,
  today,
}: {
  draft: Draft
  context: BillingContext
  providers: ReadonlyArray<Provider>
  centres: ReadonlyArray<ServiceCentre>
  today: string
}) {
  const navigate = useNavigate()
  const save = useSaveDraft(draft.id)
  const discard = useDiscardDraft(draft.id, draft.account_id)
  const finalise = useFinaliseDraft(draft.id, draft.account_id)
  const [confirming, setConfirming] = useState(false)
  const [serverError, setServerError] = useState<MappedError | null>(null)
  const [conflict, setConflict] = useState(false)
  const updatedAt = useRef(draft.updated_at)
  const lastSaved = useRef(JSON.stringify(draft.content))
  const codeInputs = useRef(new Map<string, HTMLInputElement | null>())
  const [focusLine, setFocusLine] = useState<string | null>(null)

  // Built once: new default values on every render would reset the form.
  const [initial] = useState(() => fromContent(draft.content, context, today))
  const form = useDraftForm(initial, today, () => {
    // Submitting = asking to finalise (after the checks pass).
    setServerError(null)
    setConfirming(true)
  })
  const values = useStore(form.store, (s) => s.values)

  /** Save now if anything changed since the last save. */
  const flush = async (v: DraftValues): Promise<boolean> => {
    const content = toContent(v)
    const json = JSON.stringify(content)
    if (json === lastSaved.current) return true
    try {
      const d = await save.mutateAsync({
        content,
        expectedUpdatedAt: updatedAt.current,
      })
      updatedAt.current = d.updated_at
      lastSaved.current = json
      return true
    } catch (e) {
      if (e instanceof ApiError && e.status === 409) setConflict(true)
      return false
    }
  }

  // Autosave a moment after typing stops.
  useEffect(() => {
    if (conflict) return
    const t = setTimeout(() => void flush(values), SAVE_DELAY_MS)
    return () => clearTimeout(t)
  }, [values, conflict])

  useEffect(() => {
    if (!focusLine) return
    codeInputs.current.get(focusLine)?.focus()
    setFocusLine(null)
  }, [focusLine, values.lines.length])

  const patient = context.patients.find(
    (p) => p.patient_id === values.patient_id,
  )
  const practiceDoctors = providers.filter(
    (p) => p.is_active && p.kind === 'practice',
  )
  const otherDoctors = providers.filter(
    (p) => p.is_active && p.id !== values.treating_provider_id,
  )
  const total = values.lines.reduce((n, l) => n + (lineAmount(l) ?? 0), 0)

  const addLine = (after?: number) => {
    const lines = form.getFieldValue('lines')
    const prev = after !== undefined ? lines[after] : lines.at(-1)
    const l = emptyLine(prev?.service_date || values.invoice_date || today)
    const at = after !== undefined ? after + 1 : lines.length
    form.insertFieldValue('lines', at, l)
    setFocusLine(l.key)
  }

  const setPrice = (i: number, price: PriceRef | null) => {
    form.setFieldValue(`lines[${i}].tariff_price`, price)
    if (price)
      form.setFieldValue(`lines[${i}].unit_price`, randInput(price.cents))
  }

  const lookUpPrice = async (i: number, code: string, date: string) => {
    setPrice(
      i,
      await fetchTariffPrice(code, context.medical_aid?.plan_id, date),
    )
  }

  const doFinalise = async () => {
    setServerError(null)
    if (!(await flush(form.state.values))) return
    try {
      const inv = await finalise.mutateAsync()
      await navigate({
        to: '/billing/invoices/$invoiceId',
        params: { invoiceId: inv.id },
      })
    } catch (e) {
      setConfirming(false)
      setServerError(
        e instanceof ApiError
          ? mapBackendError(e.message, form.state.values.lines)
          : { message: errorMessage(e) },
      )
    }
  }

  const onLineKeyDown = (i: number) => (e: KeyboardEvent<HTMLElement>) => {
    const t = e.target as HTMLElement
    if (e.key !== 'Enter' || e.defaultPrevented) return
    if (t.tagName === 'BUTTON' || t.tagName === 'TEXTAREA') return
    e.preventDefault()
    addLine(i)
  }

  const submitted = useStore(form.store, (s) => s.submissionAttempts > 0)
  const status = conflict
    ? null
    : save.isPending
      ? 'Saving draft…'
      : save.isError
        ? "Couldn't save the draft. Changes will be saved when you edit again."
        : 'Draft saved'

  return (
    <form
      noValidate
      aria-label="Invoice draft"
      onSubmit={(e) => {
        e.preventDefault()
        void form.handleSubmit()
      }}
      // Enter in the header fields mustn't finalise.
      onKeyDown={(e) => {
        if (
          e.key === 'Enter' &&
          (e.target as HTMLElement).tagName === 'INPUT' &&
          !e.defaultPrevented
        )
          e.preventDefault()
      }}
      className="flex flex-col gap-6"
    >
      {conflict && (
        <Alert>
          Someone else changed this draft while you were editing. Reload the
          page to see their changes; yours since then weren&rsquo;t saved.
        </Alert>
      )}

      <section
        aria-label="Invoice details"
        className="grid gap-4 rounded-lg border border-border bg-panel p-5 sm:grid-cols-2 lg:grid-cols-3"
      >
        <form.AppField name="patient_id">
          {(f) => (
            <f.SelectInput
              label="Patient"
              placeholder="Choose…"
              options={context.patients
                .filter(
                  (p) =>
                    p.status === 'active' || p.patient_id === values.patient_id,
                )
                .map((p) => ({
                  value: p.patient_id,
                  label: `${p.dependant_code} · ${fullName(p)}${
                    p.date_of_birth ? ` · ${formatDate(p.date_of_birth)}` : ''
                  }`,
                }))}
            />
          )}
        </form.AppField>
        <form.AppField
          name="invoice_date"
          listeners={{
            onChange: ({ value }) => {
              // Lines that followed the invoice date keep following it.
              const prev = form.state.values.invoice_date
              form.state.values.lines.forEach((l, i) => {
                if (l.service_date === prev || !l.service_date)
                  form.setFieldValue(`lines[${i}].service_date`, value)
              })
            },
          }}
        >
          {(f) => <f.TextInput label="Invoice date" type="date" max={today} />}
        </form.AppField>
        <form.AppField name="treating_provider_id">
          {(f) => (
            <f.SelectInput
              label="Treating doctor"
              placeholder="Choose…"
              options={practiceDoctors.map((p) => ({
                value: p.id,
                label: providerName(p),
              }))}
            />
          )}
        </form.AppField>
        <form.AppField
          name="service_centre_id"
          listeners={{
            onChange: ({ value }) => {
              const c = centres.find((x) => x.id === value)
              if (c?.default_place_of_service)
                form.setFieldValue(
                  'place_of_service',
                  c.default_place_of_service,
                )
            },
          }}
        >
          {(f) => (
            <f.SelectInput
              label="Service centre"
              placeholder="Choose…"
              options={centres
                .filter((c) => c.is_active)
                .map((c) => ({ value: c.id, label: c.name }))}
            />
          )}
        </form.AppField>
        <form.AppField name="place_of_service">
          {(f) => (
            <f.SelectInput
              label="Place of service"
              placeholder="Choose…"
              options={[
                ...PLACES_OF_SERVICE,
                ...(values.place_of_service &&
                !PLACES_OF_SERVICE.some(
                  (p) => p.code === values.place_of_service,
                )
                  ? [
                      {
                        code: values.place_of_service,
                        label: values.place_of_service,
                      },
                    ]
                  : []),
              ].map((p) => ({ value: p.code, label: p.label }))}
            />
          )}
        </form.AppField>
        <form.AppField name="referring_provider_id">
          {(f) => (
            <f.SelectInput
              label="Referring doctor (optional)"
              placeholder="None"
              options={otherDoctors.map((p) => ({
                value: p.id,
                label: providerName(p),
              }))}
            />
          )}
        </form.AppField>
        <form.AppField name="authorisation_number">
          {(f) => (
            <f.TextInput
              label="Authorisation number (optional)"
              autoComplete="off"
              spellCheck={false}
            />
          )}
        </form.AppField>
        <div className="text-sm text-muted sm:col-span-2 lg:col-span-2 lg:self-end">
          {context.medical_aid ? (
            <>
              Prices from the{' '}
              <span className="text-text">
                {context.medical_aid.scheme_name} ·{' '}
                {context.medical_aid.plan_name}
              </span>{' '}
              rates.
            </>
          ) : (
            'Private file: prices from the default rates.'
          )}
        </div>
      </section>

      <section aria-labelledby="lines-heading" className="flex flex-col gap-3">
        <div className="flex flex-wrap items-baseline justify-between gap-2">
          <h2 id="lines-heading" className="text-base font-semibold text-text">
            Lines
          </h2>
          <p className="text-sm text-muted">
            Enter in a line adds the next one.
          </p>
        </div>
        <form.Field name="lines" mode="array">
          {(linesField) => (
            <>
              {linesField.state.value.map((l, i) => (
                <LineEditor
                  key={l.key}
                  index={i}
                  line={l}
                  count={linesField.state.value.length}
                  form={form}
                  submitted={submitted}
                  patient={patient}
                  assistants={otherDoctors}
                  serverError={
                    serverError?.line === i ? serverError.message : undefined
                  }
                  codeInputRef={(el) => codeInputs.current.set(l.key, el)}
                  onKeyDown={onLineKeyDown(i)}
                  onRemove={() => linesField.removeValue(i)}
                  onPickTariff={(code) =>
                    void lookUpPrice(
                      i,
                      code,
                      l.service_date || values.invoice_date,
                    )
                  }
                  onServiceDate={(date) => {
                    if (
                      l.item?.kind === 'tariff' &&
                      !priceOverridden(l) &&
                      date.slice(0, 4) !== String(l.tariff_price?.year ?? '')
                    )
                      void lookUpPrice(i, l.item.code, date)
                  }}
                  onUseTariff={() => setPrice(i, l.tariff_price)}
                />
              ))}
            </>
          )}
        </form.Field>
        <form.Subscribe selector={(s) => s.fieldMeta.lines?.errors ?? []}>
          {(errs) =>
            submitted && firstError(errs) ? (
              <p className="text-sm text-danger">{firstError(errs)}</p>
            ) : null
          }
        </form.Subscribe>
        <div>
          <Button variant="secondary" onClick={() => addLine()}>
            <Plus className="size-4" aria-hidden="true" />
            Add line
          </Button>
        </div>
      </section>

      <section
        aria-label="Finalise"
        className="flex flex-col gap-4 rounded-lg border border-border bg-panel p-5"
      >
        <div className="flex flex-wrap items-baseline justify-between gap-3">
          <p role="status" className="text-sm text-muted">
            {status}
          </p>
          <p className="text-lg font-semibold text-text">
            Total{' '}
            <span className="tabular-nums" aria-label="Invoice total">
              {formatRand(total)}
            </span>
          </p>
        </div>

        {serverError && serverError.line === undefined && (
          <Alert>{serverError.message}</Alert>
        )}
        {serverError?.line !== undefined && (
          <Alert>
            Line {serverError.line + 1}: {serverError.message}
          </Alert>
        )}

        {confirming ? (
          <div
            role="group"
            aria-label="Confirm finalising"
            className="flex flex-col gap-3 rounded-lg border border-border-strong bg-card p-4"
          >
            <p className="text-sm text-text">
              Finalise and post this invoice for {formatRand(total)}? It gets an
              invoice number and can&rsquo;t be edited afterwards: corrections
              are made with credit notes.
            </p>
            <div className="flex flex-wrap gap-3">
              <Button
                busy={finalise.isPending}
                onClick={() => void doFinalise()}
                autoFocus
              >
                Post invoice
              </Button>
              <Button variant="secondary" onClick={() => setConfirming(false)}>
                Keep editing
              </Button>
            </div>
          </div>
        ) : (
          <div className="flex flex-wrap justify-between gap-3">
            <Button
              variant="danger"
              busy={discard.isPending}
              onClick={() => {
                if (
                  window.confirm('Discard this draft? Nothing has been posted.')
                ) {
                  discard.mutate(undefined, {
                    onSuccess: () =>
                      void navigate({
                        to: '/billing',
                      }),
                  })
                }
              }}
            >
              <Trash2 className="size-4" aria-hidden="true" />
              Discard draft
            </Button>
            <Button type="submit" disabled={conflict}>
              Finalise invoice
            </Button>
          </div>
        )}
        {discard.isError && <Alert>{errorMessage(discard.error)}</Alert>}
      </section>
    </form>
  )
}

// -----------------------------------------------------------------------------
// One line
// -----------------------------------------------------------------------------

/** The draft form: checks run when finalising (submit), then live. */
function useDraftForm(
  defaultValues: DraftValues,
  today: string,
  onSubmit: () => void,
) {
  return useAppForm({
    defaultValues,
    validationLogic: revalidateLogic(),
    validators: { onDynamic: finaliseSchema(today) },
    onSubmit,
  })
}
type EditorForm = ReturnType<typeof useDraftForm>

function LineEditor({
  index: i,
  line: l,
  count,
  form,
  submitted,
  patient,
  assistants,
  serverError,
  codeInputRef,
  onKeyDown,
  onRemove,
  onPickTariff,
  onServiceDate,
  onUseTariff,
}: {
  index: number
  line: LineValues
  count: number
  form: EditorForm
  submitted: boolean
  patient: BillingContext['patients'][number] | undefined
  assistants: ReadonlyArray<Provider>
  serverError?: string
  codeInputRef: (el: HTMLInputElement | null) => void
  onKeyDown: (e: KeyboardEvent<HTMLElement>) => void
  onRemove: () => void
  onPickTariff: (code: string) => void
  onServiceDate: (date: string) => void
  onUseTariff: () => void
}) {
  const n = i + 1
  const warnings = isBlank(l) ? [] : lineWarnings(l, patient)
  const amount = lineAmount(l)
  const err = (meta: { errors: ReadonlyArray<unknown> }) =>
    submitted ? firstError(meta.errors) : undefined
  const label = 'mb-1 block text-xs font-medium text-muted'
  const input =
    'block min-h-11 w-full rounded-lg border bg-card px-3 text-base text-text hover:border-border-strong'

  return (
    <fieldset
      onKeyDown={onKeyDown}
      className={`rounded-lg border bg-panel p-4 ${serverError ? 'border-danger' : 'border-border'}`}
    >
      <legend className="sr-only">Line {n}</legend>
      <div className="grid gap-3 md:grid-cols-[9.5rem_minmax(0,1fr)_5rem_8rem_7.5rem_auto]">
        <form.Field
          name={`lines[${i}].service_date`}
          listeners={{ onChange: ({ value }) => onServiceDate(value) }}
        >
          {(f) => (
            <div>
              <label htmlFor={`l${l.key}-date`} className={label}>
                Service date
              </label>
              <input
                id={`l${l.key}-date`}
                type="date"
                value={f.state.value}
                onChange={(e) => f.handleChange(e.target.value)}
                onBlur={f.handleBlur}
                aria-invalid={err(f.state.meta) ? true : undefined}
                className={`${input} ${err(f.state.meta) ? 'border-danger' : 'border-border'}`}
              />
              {err(f.state.meta) && (
                <p className="mt-1 text-xs text-danger">{err(f.state.meta)}</p>
              )}
            </div>
          )}
        </form.Field>

        <form.Field name={`lines[${i}].item`}>
          {(f) => (
            <div className="min-w-0">
              <span className={label} aria-hidden="true">
                Code
              </span>
              <ItemSearch
                ref={codeInputRef}
                label={`Line ${n} code`}
                value={f.state.value}
                error={err(f.state.meta)}
                onClear={() => {
                  f.handleChange(null)
                  form.setFieldValue(`lines[${i}].tariff_price`, null)
                }}
                onPick={(item) => {
                  f.handleChange(item)
                  if (item.kind === 'tariff') onPickTariff(item.code)
                  else form.setFieldValue(`lines[${i}].tariff_price`, null)
                }}
              />
            </div>
          )}
        </form.Field>

        <form.Field name={`lines[${i}].quantity`}>
          {(f) => (
            <div>
              <label htmlFor={`l${l.key}-qty`} className={label}>
                Qty
              </label>
              <input
                id={`l${l.key}-qty`}
                inputMode="decimal"
                value={f.state.value}
                onChange={(e) => f.handleChange(e.target.value)}
                onBlur={f.handleBlur}
                aria-invalid={err(f.state.meta) ? true : undefined}
                aria-label={`Line ${n} quantity`}
                className={`${input} text-right tabular-nums ${err(f.state.meta) ? 'border-danger' : 'border-border'}`}
              />
              {err(f.state.meta) && (
                <p className="mt-1 text-xs text-danger">{err(f.state.meta)}</p>
              )}
            </div>
          )}
        </form.Field>

        <form.Field name={`lines[${i}].unit_price`}>
          {(f) => (
            <div>
              <label htmlFor={`l${l.key}-price`} className={label}>
                Unit price (R)
              </label>
              <input
                id={`l${l.key}-price`}
                inputMode="decimal"
                value={f.state.value}
                onChange={(e) => f.handleChange(e.target.value)}
                onBlur={f.handleBlur}
                aria-invalid={err(f.state.meta) ? true : undefined}
                aria-label={`Line ${n} unit price in rand`}
                className={`${input} text-right tabular-nums ${err(f.state.meta) ? 'border-danger' : 'border-border'}`}
              />
              {err(f.state.meta) && (
                <p className="mt-1 text-xs text-danger">{err(f.state.meta)}</p>
              )}
            </div>
          )}
        </form.Field>

        <div>
          <span className={label}>Amount</span>
          <p
            className="flex min-h-11 items-center justify-end font-semibold tabular-nums text-text"
            aria-label={`Line ${n} amount`}
          >
            {amount === null ? '—' : formatRand(amount)}
          </p>
        </div>

        <div className="flex items-end">
          <button
            type="button"
            onClick={onRemove}
            disabled={count === 1}
            aria-label={`Remove line ${n}`}
            className="inline-flex size-11 items-center justify-center rounded-lg text-muted hover:bg-raised hover:text-text disabled:opacity-40"
          >
            <Trash2 className="size-4" aria-hidden="true" />
          </button>
        </div>
      </div>

      <PriceNote line={l} onUseTariff={onUseTariff} />

      <div className="mt-3 grid gap-3 md:grid-cols-[minmax(0,2fr)_minmax(0,1fr)_minmax(0,1fr)]">
        <form.Field name={`lines[${i}].icd10`}>
          {(f) => (
            <div className="min-w-0">
              <span className={label} aria-hidden="true">
                ICD-10 (first is primary)
              </span>
              <Icd10Picker
                label={`Line ${n} ICD-10 codes`}
                values={f.state.value}
                onChange={(v) => f.handleChange(v)}
                error={err(f.state.meta)}
              />
            </div>
          )}
        </form.Field>
        <form.Field name={`lines[${i}].modifiers`}>
          {(f) => (
            <div className="min-w-0">
              <span className={label} aria-hidden="true">
                Modifiers
              </span>
              <ModifierPicker
                label={`Line ${n} modifiers`}
                values={f.state.value}
                onChange={(v) => f.handleChange(v)}
              />
            </div>
          )}
        </form.Field>
        <form.Field name={`lines[${i}].assistant_provider_id`}>
          {(f) => (
            <div>
              <label htmlFor={`l${l.key}-assist`} className={label}>
                Assistant (optional)
              </label>
              <select
                id={`l${l.key}-assist`}
                value={f.state.value}
                onChange={(e) => f.handleChange(e.target.value)}
                aria-label={`Line ${n} assistant`}
                className={`${input} ${err(f.state.meta) ? 'border-danger' : 'border-border'}`}
              >
                <option value="">None</option>
                {assistants.map((p) => (
                  <option key={p.id} value={p.id}>
                    {providerName(p)}
                  </option>
                ))}
              </select>
              {err(f.state.meta) && (
                <p className="mt-1 text-xs text-danger">{err(f.state.meta)}</p>
              )}
            </div>
          )}
        </form.Field>
      </div>

      {warnings.length > 0 && (
        <ul
          aria-label={`Line ${n} warnings`}
          className="mt-3 flex flex-col gap-1"
        >
          {warnings.map((w) => (
            <li key={w} className="flex items-start gap-2 text-sm text-text">
              <AlertTriangle
                className="mt-0.5 size-4 shrink-0 text-muted"
                aria-hidden="true"
              />
              <span>
                <span className="font-semibold">Check: </span>
                {w}
              </span>
            </li>
          ))}
        </ul>
      )}
      {serverError && (
        <p role="alert" className="mt-3 text-sm font-medium text-danger">
          {serverError}
        </p>
      )}
    </fieldset>
  )
}

function PriceNote({
  line: l,
  onUseTariff,
}: {
  line: LineValues
  onUseTariff: () => void
}) {
  if (l.item?.kind !== 'tariff') return null
  const year = (l.service_date || '').slice(0, 4)
  if (!l.tariff_price) {
    return (
      <p className="mt-2 text-sm text-muted">
        No tariff rate for this plan in {year || 'this year'}: enter the price.
      </p>
    )
  }
  const rate = `${formatRand(l.tariff_price.cents)} (${
    l.tariff_price.source === 'plan' ? 'plan rate' : 'default rate'
  }, ${l.tariff_price.year})`
  if (!priceOverridden(l))
    return (
      <p className="mt-2 text-sm text-muted">Price from the tariff: {rate}.</p>
    )
  return (
    <p className="mt-2 flex flex-wrap items-center gap-x-3 text-sm text-text">
      <span>
        <span className="font-semibold">Price changed</span> from the tariff
        rate of {rate}.
      </span>
      <button
        type="button"
        onClick={onUseTariff}
        className="min-h-11 font-semibold text-text underline underline-offset-4"
      >
        Use the tariff rate
      </button>
    </p>
  )
}
