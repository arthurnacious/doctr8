import { describe, expect, it } from 'vitest'

import { shouldRetry } from './root-provider'
import { ApiError } from '#/lib/api/errors'

describe('query retry policy', () => {
  it('retries server and network errors once', () => {
    expect(shouldRetry(0, new ApiError(503, 'x', 'x'))).toBe(true)
    expect(shouldRetry(1, new ApiError(503, 'x', 'x'))).toBe(false)
    expect(shouldRetry(0, new TypeError('Failed to fetch'))).toBe(true)
  })

  it('never retries client errors', () => {
    expect(shouldRetry(0, new ApiError(404, 'not_found', 'x'))).toBe(false)
    expect(shouldRetry(0, new ApiError(403, 'forbidden', 'x'))).toBe(false)
  })
})
