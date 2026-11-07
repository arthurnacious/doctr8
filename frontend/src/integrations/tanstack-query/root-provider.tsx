import { QueryClient } from '@tanstack/react-query'

import { ApiError } from '#/lib/api/errors'

/**
 * Retry a failed query once, and only for server or network errors: a 4xx
 * won't fix itself, and three slow retries leave users staring at a spinner.
 */
export function shouldRetry(failureCount: number, error: unknown): boolean {
  if (error instanceof ApiError && error.status < 500) return false
  return failureCount < 1
}

export function getContext() {
  const queryClient = new QueryClient({
    defaultOptions: {
      queries: { retry: shouldRetry, staleTime: 15_000 },
      mutations: { retry: false },
    },
  })

  return {
    queryClient,
  }
}
export default function TanstackQueryProvider() {}
