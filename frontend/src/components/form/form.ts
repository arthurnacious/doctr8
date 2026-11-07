/**
 * The app's form hook (TanStack Form). Use `useAppForm` with zod schemas
 * for validation; fields render through the components registered here.
 */
import { createFormHook } from '@tanstack/react-form'

import { fieldContext, formContext } from './context'
import { RadioGroupInput, SelectInput, TextInput } from './fields'

export const { useAppForm, withForm, withFieldGroup } = createFormHook({
  fieldContext,
  formContext,
  fieldComponents: { TextInput, SelectInput, RadioGroupInput },
  formComponents: {},
})

/** First error message from TanStack Form field metadata (strings or issues). */
export function firstError(errors: ReadonlyArray<unknown>): string | undefined {
  const e = errors.at(0)
  if (!e) return undefined
  if (typeof e === 'string') return e
  if (typeof e === 'object' && 'message' in e) return String(e.message)
  return undefined
}
