import {
  keepPreviousData,
  useMutation,
  useQuery,
  useQueryClient,
} from '@tanstack/react-query'

import { api } from '#/lib/api/client'
import { invoiceKeys } from '../api'
import type { Invoice } from '../types'
import type { BillingContext, CodeRef, Icd10Ref, PriceRef } from './model'

export interface Draft {
  id: string
  account_id: string
  content: Record<string, unknown>
  created_by: string | null
  created_at: string
  updated_at: string
}

export const draftKeys = {
  context: (accountId: string) => ['billing', 'context', accountId] as const,
  draft: (id: string) => ['billing', 'draft', id] as const,
  account: (accountId: string) => ['billing', 'drafts', accountId] as const,
  codes: (kind: string, q: string) => ['coding', kind, q] as const,
  modifiers: () => ['coding', 'modifiers'] as const,
}

export function useBillingContext(accountId: string | undefined) {
  return useQuery({
    queryKey: draftKeys.context(accountId ?? ''),
    queryFn: () =>
      api<BillingContext>(`/accounts/${accountId}/billing-context`),
    enabled: Boolean(accountId),
  })
}

export function useDraft(id: string) {
  return useQuery({
    queryKey: draftKeys.draft(id),
    queryFn: () => api<Draft>(`/invoice-drafts/${id}`),
    // A draft being edited mustn't be swapped under the user.
    staleTime: Infinity,
    refetchOnWindowFocus: false,
  })
}

export function useAccountDrafts(accountId: string) {
  return useQuery({
    queryKey: draftKeys.account(accountId),
    queryFn: () => api<Array<Draft>>(`/accounts/${accountId}/invoice-drafts`),
  })
}

export function useCreateDraft() {
  const qc = useQueryClient()
  return useMutation({
    mutationFn: (accountId: string) =>
      api<Draft>('/invoice-drafts', {
        method: 'POST',
        json: { account_id: accountId },
      }),
    onSuccess: (d) => {
      qc.setQueryData(draftKeys.draft(d.id), d)
      void qc.invalidateQueries({ queryKey: draftKeys.account(d.account_id) })
    },
  })
}

export function useSaveDraft(id: string) {
  const qc = useQueryClient()
  return useMutation({
    mutationFn: (v: {
      content: Record<string, unknown>
      expectedUpdatedAt: string
    }) =>
      api<Draft>(`/invoice-drafts/${id}`, {
        method: 'PUT',
        json: { content: v.content, expected_updated_at: v.expectedUpdatedAt },
      }),
    onSuccess: (d) => {
      qc.setQueryData(draftKeys.draft(id), d)
      void qc.invalidateQueries({ queryKey: draftKeys.account(d.account_id) })
    },
  })
}

export function useDiscardDraft(id: string, accountId: string) {
  const qc = useQueryClient()
  return useMutation({
    mutationFn: () =>
      api<undefined>(`/invoice-drafts/${id}`, { method: 'DELETE' }),
    onSuccess: () => {
      qc.removeQueries({ queryKey: draftKeys.draft(id) })
      void qc.invalidateQueries({ queryKey: draftKeys.account(accountId) })
    },
  })
}

export function useFinaliseDraft(id: string, accountId: string) {
  const qc = useQueryClient()
  return useMutation({
    mutationFn: () =>
      api<Invoice>(`/invoice-drafts/${id}/finalise`, { method: 'POST' }),
    onSuccess: (inv) => {
      qc.setQueryData(invoiceKeys.invoice(inv.id), inv)
      qc.removeQueries({ queryKey: draftKeys.draft(id) })
      void qc.invalidateQueries({ queryKey: draftKeys.account(accountId) })
      void qc.invalidateQueries({ queryKey: invoiceKeys.lists() })
    },
  })
}

// -----------------------------------------------------------------------------
// Code lists (not personal data, so GET query strings are fine)
// -----------------------------------------------------------------------------

export type CodeKind = 'tariffs' | 'nappi' | 'icd10'

export function useCodeSearch<T extends CodeRef>(kind: CodeKind, q: string) {
  const query = q.trim()
  return useQuery({
    queryKey: draftKeys.codes(kind, query),
    queryFn: () =>
      api<Array<T>>(
        `/coding/${kind}?${new URLSearchParams({ q: query, limit: '12' })}`,
      ),
    enabled: query.length >= 2,
    placeholderData: keepPreviousData,
    staleTime: 10 * 60_000,
  })
}

export const searchIcd10 = (q: string) =>
  api<Array<Icd10Ref>>(
    `/coding/icd10?${new URLSearchParams({ q, limit: '12' })}`,
  )

export function useModifiers() {
  return useQuery({
    queryKey: draftKeys.modifiers(),
    queryFn: () => api<Array<CodeRef>>('/coding/modifiers?limit=100'),
    staleTime: 60 * 60_000,
  })
}

/** The tariff rate for the plan and year, or null when there's none. */
export async function fetchTariffPrice(
  code: string,
  planId: string | undefined,
  date: string,
): Promise<PriceRef | null> {
  const q = new URLSearchParams({ code, date })
  if (planId) q.set('plan_id', planId)
  try {
    const r = await api<{
      price_cents: number
      year: number
      source: 'plan' | 'default'
    }>(`/coding/tariff-price?${q}`)
    return { cents: r.price_cents, year: r.year, source: r.source }
  } catch {
    return null
  }
}
