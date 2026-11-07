/**
 * API errors in plain language for patient screens. The backend's messages
 * are already user-facing; this adds context for the common cases and swaps
 * internal names (file modes) for the words staff use.
 */
import { ApiError, errorMessage } from '#/lib/api/errors'

import { FILE_MODE_LABELS } from './format'

const MODE_KEYS: Array<[string, string]> = Object.entries(FILE_MODE_LABELS)

/** Where the error happened: hints differ between creating and editing. */
export type ErrorContext = 'create' | 'edit'

export function friendlyMessage(
  message: string,
  context: ErrorContext = 'create',
): string {
  let m = message
  for (const [key, label] of MODE_KEYS)
    m = m.replaceAll(key, `"${label.toLowerCase()}"`)

  if (/identity document already exists/i.test(m)) {
    if (context === 'edit')
      return `${m}. Check the number: it belongs to someone else on record.`
    return `${m}. Search for them and choose "Someone already in the practice" instead.`
  }
  if (/membership number is already recorded/i.test(m)) {
    return `${m}. The family is probably already on a file: search for the membership number.`
  }
  if (/can have only one patient|must have exactly one patient/i.test(m)) {
    return `${m}. This practice keeps one patient per file, so add them on a file of their own.`
  }
  if (
    /new account for each visit|only be added when the account is opened/i.test(
      m,
    )
  ) {
    return `${m}. Open a new file for this visit instead.`
  }
  if (
    /reserved for the principal|principal member must have dependant code 00/i.test(
      m,
    )
  ) {
    return `${m}. Code 00 always belongs to the medical aid main member (or the person who pays on a private file).`
  }
  if (
    /family file must include the main member|exactly one main member/i.test(m)
  ) {
    return `${m}. Add the main member to the file as 00 first.`
  }
  return m
}

export function friendlyError(
  err: unknown,
  context: ErrorContext = 'create',
): string {
  if (err instanceof ApiError) return friendlyMessage(err.message, context)
  return errorMessage(err)
}
