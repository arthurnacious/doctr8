/**
 * Forms for editing an open file: person details, medical aid, adding a
 * dependant. TanStack Form + zod; errors appear once a field is touched or
 * the user tries to save, and backend errors are shown in plain language.
 */
import { useEffect, useRef } from 'react'
import { revalidateLogic, useStore } from '@tanstack/react-form'

import { firstError, useAppForm } from '#/components/form/form'
import { Button } from '#/components/ui/Button'
import { Alert } from '#/components/ui/Field'
import {
  useAddDependant,
  useChangeMedicalAid,
  useMembership,
  useSchemes,
  useUpdateMembership,
  useUpdatePerson,
} from '../api'
import { friendlyError } from '../errors'
import { fullName } from '../format'
import { PersonFields } from '../new-file/PersonFields'
import { PersonPicker } from '../new-file/PersonPicker'
import { emptyPerson, nextCode } from '../new-file/model'
import {
  addDependantRequest,
  addDependantSchema,
  changeAidRequest,
  changeAidSchema,
  cleanMembershipNumber,
  editMembershipSchema,
  editPersonSchema,
  personToDraft,
  toPersonUpdate,
} from './model'
import type { ReactNode, RefObject } from 'react'
import type { AddDependantValues, ChangeAidValues } from './model'
import type { PatientFile, PersonDetails } from '../types'

/** Run a mutation; failures are shown from `mutation.error`, never thrown. */
async function attempt(run: () => Promise<unknown>): Promise<boolean> {
  try {
    await run()
    return true
  } catch {
    return false
  }
}

/** Puts focus in the first field when an edit form opens. */
function useFocusFirstField() {
  const ref = useRef<HTMLFormElement>(null)
  useEffect(() => {
    ref.current
      ?.querySelector<HTMLElement>('input:not([type=hidden]), select')
      ?.focus()
  }, [])
  return ref
}

function EditForm({
  label,
  formRef,
  onSubmit,
  error,
  submitting,
  submitLabel,
  onCancel,
  children,
}: {
  label: string
  formRef: RefObject<HTMLFormElement | null>
  onSubmit: () => void
  error: unknown
  submitting: boolean
  submitLabel: string
  onCancel: () => void
  children: ReactNode
}) {
  return (
    <form
      ref={formRef}
      aria-label={label}
      noValidate
      onSubmit={(e) => {
        e.preventDefault()
        onSubmit()
      }}
      className="flex flex-col gap-5"
    >
      {children}
      {error ? <Alert>{friendlyError(error, 'edit')}</Alert> : null}
      <div className="flex flex-wrap gap-3">
        <Button type="submit" busy={submitting}>
          {submitLabel}
        </Button>
        <Button variant="secondary" onClick={onCancel}>
          Cancel
        </Button>
      </div>
    </form>
  )
}

// -----------------------------------------------------------------------------
// Person details
// -----------------------------------------------------------------------------

export function EditPersonForm({
  person,
  accountId,
  onDone,
}: {
  person: PersonDetails
  accountId: string
  onDone: (saved: boolean) => void
}) {
  const save = useUpdatePerson(person.id, accountId)
  const ref = useFocusFirstField()
  const form = useAppForm({
    defaultValues: { person: personToDraft(person) },
    validationLogic: revalidateLogic(),
    validators: { onDynamic: editPersonSchema },
    onSubmit: async ({ value }) => {
      const ok = await attempt(() =>
        save.mutateAsync(toPersonUpdate(person, value.person)),
      )
      if (ok) onDone(true)
    },
  })
  const submitting = useStore(form.store, (s) => s.isSubmitting)

  return (
    <EditForm
      label={`Edit details for ${fullName(person)}`}
      formRef={ref}
      onSubmit={() => void form.handleSubmit()}
      error={save.error}
      submitting={submitting}
      submitLabel="Save details"
      onCancel={() => onDone(false)}
    >
      <PersonFields form={form} fields="person" contact address />
    </EditForm>
  )
}

// -----------------------------------------------------------------------------
// Medical aid: plan and number on the current membership
// -----------------------------------------------------------------------------

export function EditMembershipForm({
  file,
  onDone,
}: {
  file: PatientFile
  onDone: (saved: boolean) => void
}) {
  const membershipId = file.medical_aid?.membership_id
  const membership = useMembership(membershipId)
  const schemes = useSchemes()

  if (membership.isError || schemes.isError)
    return <Alert>{friendlyError(membership.error ?? schemes.error)}</Alert>
  if (!membership.data || !schemes.data || !membershipId)
    return <p className="text-sm text-muted">Loading…</p>

  const plans =
    schemes.data.find((s) => s.id === membership.data.scheme_id)?.plans ?? []
  return (
    <EditMembershipFields
      accountId={file.id}
      membershipId={membershipId}
      schemeName={membership.data.scheme_name}
      plans={plans}
      initial={{
        plan_id: membership.data.plan_id,
        membership_number: membership.data.membership_number,
      }}
      onDone={onDone}
    />
  )
}

function EditMembershipFields({
  accountId,
  membershipId,
  schemeName,
  plans,
  initial,
  onDone,
}: {
  accountId: string
  membershipId: string
  schemeName: string
  plans: Array<{ id: string; name: string }>
  initial: { plan_id: string; membership_number: string }
  onDone: (saved: boolean) => void
}) {
  const save = useUpdateMembership(membershipId, accountId)
  const ref = useFocusFirstField()
  const form = useAppForm({
    defaultValues: initial,
    validationLogic: revalidateLogic(),
    validators: { onDynamic: editMembershipSchema },
    onSubmit: async ({ value }) => {
      const ok = await attempt(() =>
        save.mutateAsync({
          plan_id: value.plan_id,
          membership_number: cleanMembershipNumber(value.membership_number),
        }),
      )
      if (ok) onDone(true)
    },
  })
  const submitting = useStore(form.store, (s) => s.isSubmitting)

  return (
    <EditForm
      label="Edit medical aid"
      formRef={ref}
      onSubmit={() => void form.handleSubmit()}
      error={save.error}
      submitting={submitting}
      submitLabel="Save medical aid"
      onCancel={() => onDone(false)}
    >
      <p className="text-sm text-muted">
        Scheme: <span className="font-semibold text-text">{schemeName}</span>.
        To move to another scheme, use Change medical aid instead.
      </p>
      <div className="grid gap-4 sm:grid-cols-2">
        <form.AppField name="plan_id">
          {(f) => (
            <f.SelectInput
              label="Plan"
              placeholder="Choose…"
              options={plans.map((p) => ({ value: p.id, label: p.name }))}
            />
          )}
        </form.AppField>
        <form.AppField name="membership_number">
          {(f) => (
            <f.TextInput
              label="Membership number"
              autoComplete="off"
              spellCheck={false}
            />
          )}
        </form.AppField>
      </div>
    </EditForm>
  )
}

// -----------------------------------------------------------------------------
// Medical aid: another scheme, or private
// -----------------------------------------------------------------------------

export function ChangeMedicalAidForm({
  file,
  onDone,
}: {
  file: PatientFile
  onDone: (saved: boolean) => void
}) {
  const schemes = useSchemes()
  const save = useChangeMedicalAid(file.id)
  const ref = useFocusFirstField()
  const principal = file.patients.find((p) => p.dependant_code === '00')
  const initial: ChangeAidValues = {
    kind: 'medical_aid',
    scheme_id: '',
    plan_id: '',
    membership_number: '',
  }
  const form = useAppForm({
    defaultValues: initial,
    validationLogic: revalidateLogic(),
    validators: { onDynamic: changeAidSchema },
    onSubmit: async ({ value }) => {
      const ok = await attempt(() => save.mutateAsync(changeAidRequest(value)))
      if (ok) onDone(true)
    },
  })
  const values = useStore(form.store, (s) => s.values)
  const submitting = useStore(form.store, (s) => s.isSubmitting)
  const plans =
    schemes.data?.find((s) => s.id === values.scheme_id)?.plans ?? []

  return (
    <EditForm
      label={file.medical_aid ? 'Change medical aid' : 'Add medical aid'}
      formRef={ref}
      onSubmit={() => void form.handleSubmit()}
      error={save.error ?? schemes.error}
      submitting={submitting}
      submitLabel={
        values.kind === 'private' ? 'Make private' : 'Save medical aid'
      }
      onCancel={() => onDone(false)}
    >
      {file.medical_aid && (
        <form.AppField name="kind">
          {(f) => (
            <f.RadioGroupInput
              legend="What changes?"
              options={[
                {
                  value: 'medical_aid',
                  label: 'A different medical aid',
                  hint: 'A new scheme or a new membership number.',
                },
                {
                  value: 'private',
                  label: 'Private',
                  hint: 'No medical aid: the person who pays is billed.',
                },
              ]}
            />
          )}
        </form.AppField>
      )}

      {values.kind === 'medical_aid' && (
        <>
          <div className="grid gap-4 sm:grid-cols-2">
            <form.AppField
              name="scheme_id"
              listeners={{ onChange: () => form.setFieldValue('plan_id', '') }}
            >
              {(f) => (
                <f.SelectInput
                  label="Medical scheme"
                  placeholder={schemes.data ? 'Choose…' : 'Loading…'}
                  options={(schemes.data ?? []).map((s) => ({
                    value: s.id,
                    label: s.name,
                  }))}
                />
              )}
            </form.AppField>
            <form.AppField name="plan_id">
              {(f) => (
                <f.SelectInput
                  label="Plan"
                  placeholder={
                    values.scheme_id ? 'Choose…' : 'Choose a scheme first'
                  }
                  disabled={!values.scheme_id}
                  options={plans.map((p) => ({ value: p.id, label: p.name }))}
                />
              )}
            </form.AppField>
          </div>
          <form.AppField name="membership_number">
            {(f) => (
              <f.TextInput
                label="Membership number"
                autoComplete="off"
                spellCheck={false}
              />
            )}
          </form.AppField>
          {principal && (
            <p className="text-sm text-muted">
              The main member will be{' '}
              <span className="font-semibold text-text">
                {fullName(principal)}
              </span>{' '}
              (dependant 00 on this file).
            </p>
          )}
        </>
      )}
      {values.kind === 'private' && (
        <p className="text-sm text-muted">
          {fullName(file.guarantor)} will be billed. They must be dependant 00
          on this file.
        </p>
      )}
    </EditForm>
  )
}

// -----------------------------------------------------------------------------
// Adding a dependant to a family file
// -----------------------------------------------------------------------------

export function AddDependantForm({
  file,
  onDone,
}: {
  file: PatientFile
  /** The new patient's id, or null when cancelled. */
  onDone: (addedPatientId: string | null) => void
}) {
  const save = useAddDependant(file.id)
  const ref = useFocusFirstField()
  const taken = file.patients.map((p) => p.dependant_code)
  const onAid = Boolean(file.medical_aid)
  const initial: AddDependantValues = {
    source: 'new',
    person: { ...emptyPerson(), last_name: file.guarantor.last_name },
    existing: null,
    // Private files: suggest the next free code. Medical aid: from the card.
    code: onAid ? '' : nextCode(taken),
  }
  const form = useAppForm({
    defaultValues: initial,
    validationLogic: revalidateLogic(),
    validators: { onDynamic: addDependantSchema(taken) },
    onSubmit: async ({ value }) => {
      let updated: PatientFile
      try {
        updated = await save.mutateAsync(addDependantRequest(value))
      } catch {
        return // shown from save.error
      }
      const added = updated.patients.find(
        (p) => !taken.includes(p.dependant_code),
      )
      onDone(added?.patient_id ?? null)
    },
  })
  const source = useStore(form.store, (s) => s.values.source)
  const submitting = useStore(form.store, (s) => s.isSubmitting)

  return (
    <EditForm
      label="Add a dependant"
      formRef={ref}
      onSubmit={() => void form.handleSubmit()}
      error={save.error}
      submitting={submitting}
      submitLabel="Add dependant"
      onCancel={() => onDone(null)}
    >
      <form.AppField name="source">
        {(f) => (
          <f.RadioGroupInput
            legend="Who is the dependant?"
            options={[
              { value: 'new', label: 'Someone new to the practice' },
              { value: 'existing', label: 'Someone already in the practice' },
            ]}
          />
        )}
      </form.AppField>

      {source === 'new' ? (
        <PersonFields form={form} fields="person" contact={false} />
      ) : (
        <form.Field name="existing">
          {(f) => (
            <PersonPicker
              label="Dependant"
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

      <div className="max-w-48">
        <form.AppField name="code">
          {(f) => (
            <f.TextInput
              label="Dependant code"
              hint={
                onAid
                  ? 'From the medical aid card.'
                  : `Used on this file: ${taken.join(', ')}.`
              }
              inputMode="numeric"
              maxLength={2}
              autoComplete="off"
            />
          )}
        </form.AppField>
      </div>
    </EditForm>
  )
}
