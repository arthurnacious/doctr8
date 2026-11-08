import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'

import { api } from '#/lib/api/client'
import type { Invoice } from '../types'

export interface Transaction {
  id: string
  account_id: string
  kind: string
  txn_date: string
  amount_cents: number
  payment_method: string | null
  reference: string | null
  receipt_no: number | null
  entries: Array<{
    invoice_id: string | null
    bucket: 'medical_aid' | 'patient'
    amount_cents: number
  }>
}

export interface ReasonType {
  id: string
  code: string
  name: string
  is_active: boolean
}

/** After any posting: the invoice, statements and lists are out of date. */
function useRefresh() {
  const qc = useQueryClient()
  return () => {
    void qc.invalidateQueries({ queryKey: ['billing'] })
  }
}

export function useCreditNote(invoiceId: string) {
  const refresh = useRefresh()
  return useMutation({
    mutationFn: (body: object) =>
      api<Invoice>(`/invoices/${invoiceId}/credit-notes`, {
        method: 'POST',
        json: body,
      }),
    onSuccess: refresh,
  })
}

export function useRecordPayment() {
  const refresh = useRefresh()
  return useMutation({
    mutationFn: (body: object) =>
      api<Transaction>('/payments', { method: 'POST', json: body }),
    onSuccess: refresh,
  })
}

export function useTransfer() {
  const refresh = useRefresh()
  return useMutation({
    mutationFn: (body: object) =>
      api<Transaction>('/liability-transfers', { method: 'POST', json: body }),
    onSuccess: refresh,
  })
}

export function useJournal() {
  const refresh = useRefresh()
  return useMutation({
    mutationFn: (body: object) =>
      api<Transaction>('/journals', { method: 'POST', json: body }),
    onSuccess: refresh,
  })
}

export function useReasonTypes() {
  return useQuery({
    queryKey: ['billing', 'journal-reasons'],
    queryFn: () => api<Array<ReasonType>>('/journal-reason-types'),
    staleTime: 10 * 60_000,
  })
}
