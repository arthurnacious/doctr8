import { keepPreviousData, useQuery } from '@tanstack/react-query'

import { api } from '#/lib/api/client'
import type {
  ClaimStatus,
  Invoice,
  InvoiceListItem,
  InvoicePage,
  Provider,
  ServiceCentre,
  Statement,
} from './types'

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

// -----------------------------------------------------------------------------
// Invoices
// -----------------------------------------------------------------------------

export type DueFilter = 'outstanding' | 'settled'
export type ClaimFilter = 'none' | ClaimStatus

export interface InvoiceSearchParams {
  from?: string
  to?: string
  providerId?: string
  patientId?: string
  status?: DueFilter
  claimStatus?: ClaimFilter
  page: number
}

export const INVOICE_PAGE_SIZE = 25

export const invoiceKeys = {
  all: ['billing', 'invoice'] as const,
  invoice: (id: string) => ['billing', 'invoice', id] as const,
  lists: () => ['billing', 'invoices'] as const,
  account: (accountId: string) =>
    ['billing', 'invoices', 'account', accountId] as const,
  search: (p: InvoiceSearchParams) =>
    ['billing', 'invoices', 'search', p] as const,
  providers: () => ['billing', 'providers'] as const,
  centres: () => ['billing', 'service-centres'] as const,
}

export function useAccountInvoices(accountId: string) {
  return useQuery({
    queryKey: invoiceKeys.account(accountId),
    queryFn: () =>
      api<Array<InvoiceListItem>>(`/accounts/${accountId}/invoices`),
  })
}

/** POST: filters (including patient ids) stay out of request URLs. */
export function useInvoiceSearch(p: InvoiceSearchParams) {
  return useQuery({
    queryKey: invoiceKeys.search(p),
    queryFn: () =>
      api<InvoicePage>('/invoices/search', {
        method: 'POST',
        json: {
          from: p.from,
          to: p.to,
          provider_id: p.providerId,
          patient_id: p.patientId,
          status: p.status,
          claim_status: p.claimStatus,
          page: p.page,
          page_size: INVOICE_PAGE_SIZE,
        },
      }),
    placeholderData: keepPreviousData,
  })
}

export function useInvoice(id: string) {
  return useQuery({
    queryKey: invoiceKeys.invoice(id),
    queryFn: () => api<Invoice>(`/invoices/${id}`),
  })
}

export function useProviders() {
  return useQuery({
    queryKey: invoiceKeys.providers(),
    queryFn: () => api<Array<Provider>>('/providers'),
    staleTime: 5 * 60_000,
  })
}

export function useServiceCentres() {
  return useQuery({
    queryKey: invoiceKeys.centres(),
    queryFn: () => api<Array<ServiceCentre>>('/service-centres'),
    staleTime: 5 * 60_000,
  })
}
