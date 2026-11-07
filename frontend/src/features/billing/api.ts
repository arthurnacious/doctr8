import { keepPreviousData, useQuery } from '@tanstack/react-query'

import { api } from '#/lib/api/client'
import type { Statement } from './types'

export interface StatementParams {
  /** yyyy-mm-dd; omit for today. */
  asOf?: string
}

export const billingKeys = {
  all: ['billing'] as const,
  statement: (accountId: string, p: StatementParams) =>
    ['billing', 'statement', accountId, p] as const,
}

/**
 * The account statement. Reversed entries are always fetched; the screen
 * decides what to hide, so toggling doesn't refetch.
 */
export function useStatement(accountId: string, p: StatementParams) {
  return useQuery({
    queryKey: billingKeys.statement(accountId, p),
    queryFn: () => {
      const q = new URLSearchParams({ include_reversed: 'true' })
      if (p.asOf) q.set('as_of', p.asOf)
      return api<Statement>(`/accounts/${accountId}/statement?${q}`)
    },
    placeholderData: keepPreviousData,
  })
}
