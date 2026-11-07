/** An error response from the API: `{"error":{"code":"…","message":"…"}}`. */
export class ApiError extends Error {
  constructor(
    readonly status: number,
    readonly code: string,
    message: string,
  ) {
    super(message)
    this.name = 'ApiError'
  }
}

export const NETWORK_ERROR_MESSAGE =
  "Can't reach doctr8 right now. Check your connection and try again."

export async function toApiError(res: Response): Promise<ApiError> {
  try {
    const body = (await res.json()) as {
      error?: { code?: unknown; message?: unknown }
    }
    const code = body.error?.code
    const message = body.error?.message
    if (typeof code === 'string' && typeof message === 'string') {
      return new ApiError(res.status, code, message)
    }
  } catch {
    // Not JSON (e.g. a proxy error page).
  }
  return new ApiError(
    res.status,
    'http_' + res.status,
    res.status >= 500
      ? 'Something went wrong. Please try again.'
      : res.statusText || 'Request failed',
  )
}

/** A message safe to show the user for any thrown value. */
export function errorMessage(err: unknown): string {
  if (err instanceof ApiError) return err.message
  if (err instanceof TypeError) return NETWORK_ERROR_MESSAGE // fetch network failure
  return 'Something went wrong. Please try again.'
}
