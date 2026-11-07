import { useEffect, useRef, useState } from 'react'
import { Link, createFileRoute, useNavigate } from '@tanstack/react-router'
import { revalidateLogic, useStore } from '@tanstack/react-form'
import { ArrowLeft, Plus, Trash2 } from 'lucide-react'

import { firstError, useAppForm, withForm } from '#/components/form/form'
import { Button } from '#/components/ui/Button'
import { Alert } from '#/components/ui/Field'
import {
  useCreateFile,
  useFileSettings,
  useSchemes,
} from '#/features/patients/api'
import { friendlyError } from '#/features/patients/errors'
import { FILE_MODE_LABELS, formatDate } from '#/features/patients/format'
import { PersonFields } from '#/features/patients/new-file/PersonFields'
import { PersonPicker } from '#/features/patients/new-file/PersonPicker'
import {
  STEP_LABELS,
  buildFileRequest,
  defaultValues,
  emptyPerson,
  nextCode,
  patientCode,
  patientIsPrincipal,
  stepsFor,
  validateStep,
} from '#/features/patients/new-file/model'
import { requirePermission } from '#/lib/auth/guards'
import { errorMessage } from '#/lib/api/errors'
import type {
  NewFileValues,
  PersonDraft,
  Step,
} from '#/features/patients/new-file/model'
import type { FileMode, Scheme } from '#/features/patients/types'

export const Route = createFileRoute('/_app/patients/new')({
  beforeLoad: () => requirePermission({ anyOf: ['patients.manage'] }),
  head: () => ({ meta: [{ title: 'New file · doctr8' }] }),
  component: NewFilePage,
})

function NewFilePage() {
  const settings = useFileSettings()
  const schemes = useSchemes()
  const failed = settings.error ?? schemes.error

  return (
    <div className="mx-auto flex max-w-3xl flex-col gap-6">
      <Link
        to="/patients"
        className="inline-flex min-h-11 items-center gap-2 self-start rounded-lg pr-3 text-sm font-medium text-muted hover:text-text"
      >
        <ArrowLeft className="size-4" aria-hidden="true" />
        Patients
      </Link>
      <h1 className="text-2xl font-semibold tracking-tight text-text">
        New file
      </h1>
      {failed ? (
        <Alert>{errorMessage(failed)}</Alert>
      ) : settings.data && schemes.data ? (
        <NewFileFlow mode={settings.data.file_mode} schemes={schemes.data} />
      ) : (
        <p className="text-sm text-muted">Loading…</p>
      )}
    </div>
  )
}

const MODE_NOTES: Record<FileMode, string> = {
  patient_per_file:
    'This practice keeps one patient per file. Each family member gets a file of their own.',
  family_per_file:
    'This practice keeps a family on one file: the main member is 00 and dependants are 01, 02 and so on. You can add dependants in step 4.',
  debtor_per_visit:
    'This practice opens a new account at each visit. This creates the account for this visit; next time, a new one is created.',
}

function NewFileFlow({
  mode,
  schemes,
}: {
  mode: FileMode
  schemes: Array<Scheme>
}) {
  const steps = stepsFor(mode)
  const [index, setIndex] = useState(0)
  const step = steps[index]
  const stepRef = useRef<Step>(step)
  stepRef.current = step
  const headingRef = useRef<HTMLHeadingElement>(null)
  const navigate = useNavigate()
  const create = useCreateFile()

  const form = useAppForm({
    defaultValues: defaultValues(),
    validationLogic: revalidateLogic(),
    validators: {
      onDynamic: ({ value }) => {
        const fields = validateStep(stepRef.current, mode, value)
        return Object.keys(fields).length ? { fields } : undefined
      },
    },
    onSubmit: async ({ value, formApi }) => {
      if (stepRef.current !== 'review') {
        // Keep the values, forget this step's submit attempt so the next
        // step doesn't open full of errors.
        formApi.reset(value, { keepDefaultValues: true })
        setIndex((i) => i + 1)
        return
      }
      let file
      try {
        file = await create.mutateAsync(buildFileRequest(value, mode))
      } catch {
        // Shown on screen (create.error). Never rethrow: an unhandled
        // rejection would print to the browser console.
        return
      }
      await navigate({
        to: '/patients/file/$accountId',
        params: { accountId: file.id },
        search: {
          patient: file.patients.find((p) => p.dependant_code === '00')
            ?.patient_id,
        },
      })
    },
  })

  // Move focus to the step heading so screen readers announce the new step.
  const first = useRef(true)
  useEffect(() => {
    if (first.current) {
      first.current = false
      return
    }
    headingRef.current?.focus()
  }, [index])

  const values = useStore(form.store, (s) => s.values)

  return (
    <form
      noValidate
      onSubmit={(e) => {
        e.preventDefault()
        void form.handleSubmit()
      }}
      className="flex flex-col gap-6"
    >
      <Alert tone="info">
        <strong className="font-semibold text-text">
          {FILE_MODE_LABELS[mode]}.
        </strong>{' '}
        {MODE_NOTES[mode]}
      </Alert>

      <ol className="flex flex-wrap gap-2" aria-label="Steps">
        {steps.map((s, i) => (
          <li
            key={s}
            aria-current={i === index ? 'step' : undefined}
            className={`rounded-full border px-3 py-1 text-xs font-semibold ${
              i === index
                ? 'border-accent text-text'
                : i < index
                  ? 'border-border-strong text-muted'
                  : 'border-border text-subtle'
            }`}
          >
            {i + 1}. {STEP_LABELS[s]}
          </li>
        ))}
      </ol>

      <section className="rounded-lg border border-border bg-panel p-5">
        <h2
          ref={headingRef}
          tabIndex={-1}
          className="mb-5 text-lg font-semibold text-text"
        >
          {STEP_LABELS[step]}
        </h2>

        {step === 'patient' && (
          <PersonFields form={form} fields="patient" contact address />
        )}

        {step === 'payer' && (
          <div className="flex flex-col gap-5">
            <form.AppField name="payer.who">
              {(f) => (
                <f.RadioGroupInput
                  legend="Who pays the account?"
                  hint="The guarantor: statements are addressed to them."
                  options={[
                    { value: 'patient', label: 'The patient' },
                    {
                      value: 'new',
                      label: 'Someone else, new to the practice',
                      hint: 'e.g. a parent',
                    },
                    {
                      value: 'existing',
                      label: 'Someone already in the practice',
                    },
                  ]}
                />
              )}
            </form.AppField>
            {values.payer.who === 'new' && (
              <PersonFields form={form} fields="payer.person" contact address />
            )}
            {values.payer.who === 'existing' && (
              <form.Field name="payer.existing">
                {(f) => (
                  <PersonPicker
                    label="Who pays"
                    value={f.state.value}
                    onChange={(p) => f.handleChange(p)}
                    error={
                      f.form.state.submissionAttempts > 0
                        ? firstError(f.state.meta.errors)
                        : undefined
                    }
                  />
                )}
              </form.Field>
            )}
          </div>
        )}

        {step === 'aid' && (
          <MedicalAidStep
            form={form}
            values={values}
            schemes={schemes}
            mode={mode}
          />
        )}

        {step === 'dependants' && (
          <DependantsStep form={form} values={values} mode={mode} />
        )}

        {step === 'review' && (
          <Review values={values} mode={mode} schemes={schemes} />
        )}
      </section>

      {create.isError && <Alert>{friendlyError(create.error)}</Alert>}

      <div className="flex flex-wrap justify-between gap-3">
        <Button
          variant="secondary"
          disabled={index === 0 || create.isPending}
          onClick={() => setIndex((i) => Math.max(0, i - 1))}
        >
          Back
        </Button>
        <form.Subscribe selector={(s) => s.isSubmitting}>
          {(submitting) => (
            <Button type="submit" busy={submitting}>
              {step === 'review'
                ? submitting
                  ? 'Creating…'
                  : 'Create file'
                : 'Next'}
            </Button>
          )}
        </form.Subscribe>
      </div>
    </form>
  )
}

const MedicalAidStep = withForm({
  defaultValues: defaultValues(),
  props: {} as {
    values: NewFileValues
    schemes: Array<Scheme>
    mode: FileMode
  },
  render: function Render({ form, values, schemes, mode }) {
    const plans =
      schemes.find((s) => s.id === values.aid.scheme_id)?.plans ?? []
    const someoneElsePays = values.payer.who !== 'patient'

    return (
      <div className="flex flex-col gap-5">
        <form.AppField name="aid.kind">
          {(f) => (
            <f.RadioGroupInput
              legend="How is this file billed?"
              options={[
                {
                  value: 'private',
                  label: 'Private',
                  hint: 'No medical aid: the person who pays is billed.',
                },
                { value: 'medical_aid', label: 'Medical aid' },
              ]}
            />
          )}
        </form.AppField>

        {values.aid.kind === 'medical_aid' && (
          <>
            {schemes.length === 0 && (
              <Alert>
                No medical schemes are set up yet, so this file can only be
                private for now.
              </Alert>
            )}
            <div className="grid gap-4 sm:grid-cols-2">
              <form.AppField
                name="aid.scheme_id"
                listeners={{
                  onChange: () => form.setFieldValue('aid.plan_id', ''),
                }}
              >
                {(f) => (
                  <f.SelectInput
                    label="Medical scheme"
                    placeholder="Choose…"
                    options={schemes.map((s) => ({
                      value: s.id,
                      label: s.name,
                    }))}
                  />
                )}
              </form.AppField>
              <form.AppField name="aid.plan_id">
                {(f) => (
                  <f.SelectInput
                    label="Plan"
                    placeholder={
                      values.aid.scheme_id ? 'Choose…' : 'Choose a scheme first'
                    }
                    disabled={!values.aid.scheme_id}
                    options={plans.map((p) => ({ value: p.id, label: p.name }))}
                  />
                )}
              </form.AppField>
            </div>
            <form.AppField name="aid.membership_number">
              {(f) => (
                <f.TextInput
                  label="Membership number"
                  autoComplete="off"
                  spellCheck={false}
                />
              )}
            </form.AppField>

            <form.AppField name="aid.main">
              {(f) => (
                <f.RadioGroupInput
                  legend="Main member"
                  hint={
                    mode === 'family_per_file'
                      ? 'The main member is dependant 00 and is added to the file.'
                      : 'The main member is dependant 00 on the card.'
                  }
                  options={[
                    { value: 'patient', label: 'The patient' },
                    ...(someoneElsePays
                      ? [
                          {
                            value: 'payer' as const,
                            label: 'The person who pays',
                          },
                        ]
                      : []),
                    {
                      value: 'new',
                      label: 'Someone else, new to the practice',
                    },
                    {
                      value: 'existing',
                      label: 'Someone already in the practice',
                    },
                  ]}
                />
              )}
            </form.AppField>

            {values.aid.main === 'new' && (
              <PersonFields form={form} fields="aid.person" contact={false} />
            )}
            {values.aid.main === 'existing' && (
              <form.Field name="aid.existing">
                {(f) => (
                  <PersonPicker
                    label="Main member"
                    value={f.state.value}
                    onChange={(p) => f.handleChange(p)}
                    error={
                      f.form.state.submissionAttempts > 0
                        ? firstError(f.state.meta.errors)
                        : undefined
                    }
                  />
                )}
              </form.Field>
            )}

            {!patientIsPrincipal(values) && (
              <form.AppField name="aid.patient_code">
                {(f) => (
                  <f.TextInput
                    label="Patient's dependant code"
                    hint="From the medical aid card, e.g. 01"
                    inputMode="numeric"
                    maxLength={2}
                    autoComplete="off"
                  />
                )}
              </form.AppField>
            )}
          </>
        )}
      </div>
    )
  },
})

const DependantsStep = withForm({
  defaultValues: defaultValues(),
  props: {} as { values: NewFileValues; mode: FileMode },
  render: function Render({ form, values, mode }) {
    const used = [
      '00',
      patientCode(values, mode) ?? '',
      ...values.dependants.map((d) => d.code),
    ]
    return (
      <form.Field name="dependants" mode="array">
        {(list) => (
          <div className="flex flex-col gap-5">
            <p className="text-sm text-muted">
              Add anyone else on this family file. Codes continue from the
              patient's: check them against the medical aid card.
            </p>
            {list.state.value.length === 0 && (
              <p className="text-sm text-subtle">No dependants added.</p>
            )}
            {list.state.value.map((_, i) => (
              <fieldset key={i} className="rounded-lg border border-border p-4">
                <legend className="px-1 text-sm font-semibold text-text">
                  Dependant {i + 1}
                </legend>
                <div className="flex flex-col gap-4">
                  <div className="w-32">
                    <form.AppField name={`dependants[${i}].code`}>
                      {(f) => (
                        <f.TextInput
                          label="Dependant code"
                          inputMode="numeric"
                          maxLength={2}
                          autoComplete="off"
                        />
                      )}
                    </form.AppField>
                  </div>
                  <PersonFields
                    form={form}
                    fields={`dependants[${i}].person`}
                    contact={false}
                  />
                  <Button
                    variant="danger"
                    className="self-start"
                    onClick={() => list.removeValue(i)}
                  >
                    <Trash2 className="size-4" aria-hidden="true" />
                    Remove dependant {i + 1}
                  </Button>
                </div>
              </fieldset>
            ))}
            <Button
              variant="secondary"
              className="self-start"
              onClick={() =>
                list.pushValue({ person: emptyPerson(), code: nextCode(used) })
              }
            >
              <Plus className="size-4" aria-hidden="true" />
              Add dependant
            </Button>
          </div>
        )}
      </form.Field>
    )
  },
})

function personLine(p: PersonDraft): string {
  const id =
    p.id_kind === 'sa_id'
      ? `SA ID ${p.id_number}`
      : p.id_kind === 'passport'
        ? `Passport ${p.id_number}`
        : 'No ID document'
  return `${[p.title, p.first_name, p.last_name].filter(Boolean).join(' ')} · ${p.date_of_birth ? formatDate(p.date_of_birth) : 'No date of birth'} · ${id}`
}

function Review({
  values,
  mode,
  schemes,
}: {
  values: NewFileValues
  mode: FileMode
  schemes: Array<Scheme>
}) {
  const scheme = schemes.find((s) => s.id === values.aid.scheme_id)
  const plan = scheme?.plans.find((p) => p.id === values.aid.plan_id)
  const payer =
    values.payer.who === 'patient'
      ? 'The patient'
      : values.payer.who === 'existing'
        ? (values.payer.existing?.label ?? '')
        : personLine(values.payer.person)
  const main =
    values.aid.main === 'patient'
      ? 'The patient'
      : values.aid.main === 'payer'
        ? 'The person who pays'
        : values.aid.main === 'existing'
          ? (values.aid.existing?.label ?? '')
          : personLine(values.aid.person)

  return (
    <dl className="flex flex-col gap-4 text-sm">
      <ReviewRow label="Patient">
        {personLine(values.patient)}
        <span className="block text-muted">
          Dependant code {patientCode(values, mode) ?? 'chosen by doctr8'}
        </span>
      </ReviewRow>
      <ReviewRow label="Who pays">{payer}</ReviewRow>
      <ReviewRow label="Medical aid">
        {values.aid.kind === 'private' ? (
          'Private'
        ) : (
          <>
            {scheme?.name} · {plan?.name} ·{' '}
            {values.aid.membership_number.toUpperCase()}
            <span className="block text-muted">Main member: {main}</span>
          </>
        )}
      </ReviewRow>
      {mode === 'family_per_file' && (
        <ReviewRow label="Dependants">
          {values.dependants.length === 0
            ? 'None'
            : values.dependants.map((d, i) => (
                <span key={i} className="block">
                  {d.code} · {personLine(d.person)}
                </span>
              ))}
        </ReviewRow>
      )}
      <p className="text-muted">
        Nothing is saved until you choose Create file.
      </p>
    </dl>
  )
}

function ReviewRow({
  label,
  children,
}: {
  label: string
  children: React.ReactNode
}) {
  return (
    <div className="flex flex-col gap-1 sm:flex-row sm:gap-4">
      <dt className="w-32 shrink-0 font-medium text-muted">{label}</dt>
      <dd className="text-text">{children}</dd>
    </div>
  )
}
