import { useRef } from 'react'

import { withFieldGroup } from '#/components/form/form'
import { checkSaId } from '#/lib/sa-id'
import { emptyPerson } from './model'
import type { IdKind } from './model'

const ID_KINDS: Array<{ value: IdKind; label: string }> = [
  { value: 'sa_id', label: 'South African ID' },
  { value: 'passport', label: 'Passport' },
  { value: 'none', label: 'No document' },
]

const SEXES = [
  { value: 'female', label: 'Female' },
  { value: 'male', label: 'Male' },
  { value: 'other', label: 'Other' },
  { value: 'unknown', label: 'Not recorded' },
]

/**
 * A person's details, reusable anywhere in a form (patient, payer, main
 * member, dependants). A valid SA ID number fills in the date of birth,
 * which the user can still correct (e.g. the century).
 */
export const PersonFields = withFieldGroup({
  defaultValues: emptyPerson(),
  // Widened on purpose: both props are optional.
  // eslint-disable-next-line @typescript-eslint/no-unnecessary-type-assertion
  props: {} as { contact?: boolean; address?: boolean },
  render: function Render({ group, contact = true, address = false }) {
    const autoDob = useRef<string | null>(null)

    return (
      <div className="grid gap-4 sm:grid-cols-6">
        <div className="sm:col-span-1">
          <group.AppField name="title">
            {(f) => (
              <f.TextInput
                label="Title"
                autoComplete="off"
                placeholder="Mr, Ms, Dr"
              />
            )}
          </group.AppField>
        </div>
        <div className="sm:col-span-2">
          <group.AppField name="first_name">
            {(f) => <f.TextInput label="First name" autoComplete="off" />}
          </group.AppField>
        </div>
        <div className="sm:col-span-3">
          <group.AppField name="last_name">
            {(f) => <f.TextInput label="Surname" autoComplete="off" />}
          </group.AppField>
        </div>

        <div className="sm:col-span-6">
          <group.AppField name="id_kind">
            {(f) => (
              <f.RadioGroupInput
                legend="Identity document"
                options={ID_KINDS}
              />
            )}
          </group.AppField>
        </div>

        <group.Subscribe selector={(s) => s.values.id_kind}>
          {(kind) =>
            kind === 'none' ? null : (
              <>
                <div
                  className={
                    kind === 'passport' ? 'sm:col-span-4' : 'sm:col-span-6'
                  }
                >
                  <group.AppField
                    name="id_number"
                    listeners={{
                      onChange: ({ value }) => {
                        if (group.getFieldValue('id_kind') !== 'sa_id') return
                        const r = checkSaId(value)
                        if (!r.ok) return
                        const current = group.getFieldValue('date_of_birth')
                        // Fill in, or replace a date we filled in earlier;
                        // never overwrite one the user typed.
                        if (!current || current === autoDob.current) {
                          autoDob.current = r.dateOfBirth
                          group.setFieldValue('date_of_birth', r.dateOfBirth)
                        }
                      },
                    }}
                  >
                    {(f) => (
                      <f.TextInput
                        label={
                          kind === 'sa_id' ? 'SA ID number' : 'Passport number'
                        }
                        inputMode={kind === 'sa_id' ? 'numeric' : 'text'}
                        autoComplete="off"
                        spellCheck={false}
                      />
                    )}
                  </group.AppField>
                </div>
                {kind === 'passport' && (
                  <div className="sm:col-span-2">
                    <group.AppField name="id_country">
                      {(f) => (
                        <f.TextInput
                          label="Issuing country"
                          hint="2-letter code, e.g. ZW"
                          maxLength={2}
                          autoComplete="off"
                        />
                      )}
                    </group.AppField>
                  </div>
                )}
              </>
            )
          }
        </group.Subscribe>

        <div className="sm:col-span-3">
          <group.Subscribe
            selector={(s) =>
              s.values.date_of_birth === autoDob.current &&
              Boolean(autoDob.current)
            }
          >
            {(fromId) => (
              <group.AppField name="date_of_birth">
                {(f) => (
                  <f.TextInput
                    label="Date of birth"
                    type="date"
                    hint={
                      fromId
                        ? 'Filled in from the ID number. Change it if the century is wrong.'
                        : undefined
                    }
                  />
                )}
              </group.AppField>
            )}
          </group.Subscribe>
        </div>
        <div className="sm:col-span-3">
          <group.AppField name="sex">
            {(f) => (
              <f.SelectInput
                label="Sex"
                placeholder="Choose…"
                options={SEXES}
              />
            )}
          </group.AppField>
        </div>

        {contact && (
          <>
            <div className="sm:col-span-3">
              <group.AppField name="mobile">
                {(f) => (
                  <f.TextInput
                    label="Mobile"
                    type="tel"
                    inputMode="tel"
                    autoComplete="off"
                  />
                )}
              </group.AppField>
            </div>
            <div className="sm:col-span-3">
              <group.AppField name="email">
                {(f) => (
                  <f.TextInput label="Email" type="email" autoComplete="off" />
                )}
              </group.AppField>
            </div>
          </>
        )}

        {address && (
          <>
            <div className="sm:col-span-6">
              <group.AppField name="address_line1">
                {(f) => (
                  <f.TextInput label="Street address" autoComplete="off" />
                )}
              </group.AppField>
            </div>
            <div className="sm:col-span-2">
              <group.AppField name="suburb">
                {(f) => <f.TextInput label="Suburb" autoComplete="off" />}
              </group.AppField>
            </div>
            <div className="sm:col-span-2">
              <group.AppField name="city">
                {(f) => <f.TextInput label="City" autoComplete="off" />}
              </group.AppField>
            </div>
            <div className="sm:col-span-2">
              <group.AppField name="postal_code">
                {(f) => (
                  <f.TextInput
                    label="Postal code"
                    inputMode="numeric"
                    maxLength={4}
                    autoComplete="off"
                  />
                )}
              </group.AppField>
            </div>
          </>
        )}
      </div>
    )
  },
})
