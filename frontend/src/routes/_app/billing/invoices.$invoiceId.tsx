import { Link, createFileRoute } from '@tanstack/react-router'
import { ArrowLeft } from 'lucide-react'

import { Button } from '#/components/ui/Button'
import { Alert } from '#/components/ui/Field'
import { InvoiceView } from '#/features/billing/InvoiceView'
import { useInvoice } from '#/features/billing/api'
import { ApiError, errorMessage } from '#/lib/api/errors'
import { useFollowAccount } from '#/lib/ws/useRealtime'

export const Route = createFileRoute('/_app/billing/invoices/$invoiceId')({
  head: () => ({ meta: [{ title: 'Invoice · doctr8' }] }),
  component: InvoicePage,
})

function InvoicePage() {
  const { invoiceId } = Route.useParams()
  const invoice = useInvoice(invoiceId)
  // Claims, remittances and payments on this account refresh the invoice.
  useFollowAccount(invoice.data?.account_id)

  return (
    <div className="invoice-page mx-auto flex max-w-5xl flex-col gap-6">
      <Link
        to="/billing"
        className="no-print inline-flex min-h-11 items-center gap-2 self-start rounded-lg pr-3 text-sm font-medium text-muted hover:text-text"
      >
        <ArrowLeft className="size-4" aria-hidden="true" />
        Billing
      </Link>
      <h1 className="text-2xl font-semibold tracking-tight text-text">
        Invoice{' '}
        {invoice.data && (
          <span className="invoice-key tabular-nums text-accent">
            {invoice.data.invoice_no}
          </span>
        )}
      </h1>
      {invoice.isError ? (
        <div className="flex flex-col items-start gap-4">
          <Alert>
            {invoice.error instanceof ApiError && invoice.error.status === 404
              ? "This invoice doesn't exist in this practice."
              : errorMessage(invoice.error)}
          </Alert>
          <Button variant="secondary" onClick={() => void invoice.refetch()}>
            Try again
          </Button>
        </div>
      ) : !invoice.data ? (
        <p className="py-12 text-center text-sm text-muted">Loading invoice…</p>
      ) : (
        <InvoiceView invoice={invoice.data} />
      )}
    </div>
  )
}
