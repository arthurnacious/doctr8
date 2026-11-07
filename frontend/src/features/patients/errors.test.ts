import { describe, expect, it } from 'vitest'

import { friendlyError, friendlyMessage } from './errors'
import { ApiError } from '#/lib/api/errors'

describe('friendly patient errors', () => {
  it('names file modes the way staff do', () => {
    expect(
      friendlyMessage(
        'Accounts opened in patient_per_file mode must have exactly one patient',
      ),
    ).toBe(
      'Accounts opened in "patient per file" mode must have exactly one patient. This practice keeps one patient per file, so add them on a file of their own.',
    )
  })

  it('suggests what to do next', () => {
    expect(
      friendlyMessage(
        'A person with this identity document already exists in this practice',
      ),
    ).toMatch(/Someone already in the practice/)
    expect(
      friendlyMessage(
        'This account was opened in new debtor per visit mode: open a new account for each visit instead of adding patients',
      ),
    ).toMatch(/Open a new file for this visit/)
    expect(
      friendlyMessage('The principal member must have dependant code 00'),
    ).toMatch(/main member/)
  })

  it('passes other messages through', () => {
    expect(friendlyMessage('Something else')).toBe('Something else')
    expect(
      friendlyError(
        new ApiError(422, 'validation_failed', 'first_name is required'),
      ),
    ).toBe('first_name is required')
    expect(friendlyError(new TypeError('Failed to fetch'))).toMatch(
      /Can't reach doctr8/,
    )
  })
})
