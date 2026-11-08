/**
 * What staff can do to a posted invoice, each by permission: credit notes
 * (billing.manage), payments (payments.manage), liability moves and
 * journals (journals.manage). One form at a time; each posts a new
 * transaction.
 */
import { useRef, useState } from 'react'

import { Button } from '#/components/ui/Button'
import { useCan } from '#/lib/auth/session'
import {
  CreditNoteForm,
  JournalForm,
  PaymentForm,
  ReceiptSummary,
  TransferForm,
} from './CorrectionForms'
import { remaining } from './model'
import type { Transaction } from './api'
import type { Invoice } from '../types'

type Action = 'credit' | 'payment' | 'transfer' | 'journal'

export function InvoiceActions({
  invoice,
  today,
}: {
  invoice: Invoice
  today: string
}) {
  const canCredit = useCan('billing.manage')
  const canPay = useCan('payments.manage')
  const canJournal = useCan('journals.manage')
  const [open, setOpen] = useState<Action | null>(null)
  const [done, setDone] = useState<string | null>(null)
  const [receipt, setReceipt] = useState<Transaction | null>(null)
  const buttons = useRef(new Map<Action, HTMLButtonElement | null>())

  if (!canCredit && !canPay && !canJournal) return null

  const show = (a: Action) => {
    setDone(null)
    setReceipt(null)
    setOpen(a)
  }
  const close = (a: Action, message: string | null) => {
    setOpen(null)
    setDone(message)
    requestAnimationFrame(() => buttons.current.get(a)?.focus())
  }
  const creditable = invoice.lines.some((l) => remaining(l) > 0)

  const button = (a: Action, label: string, disabled = false) => (
    <Button
      variant="secondary"
      ref={(el) => {
        buttons.current.set(a, el)
      }}
      disabled={disabled}
      onClick={() => show(a)}
      aria-expanded={open === a}
    >
      {label}
    </Button>
  )

  return (
    <section
      aria-label="Invoice actions"
      className="no-print flex flex-col gap-4"
    >
      <div className="flex flex-wrap items-center gap-2">
        {canPay && button('payment', 'Record payment')}
        {canCredit && button('credit', 'Issue credit note', !creditable)}
        {canJournal &&
          invoice.medical_aid &&
          button('transfer', 'Move liability')}
        {canJournal && button('journal', 'Post journal')}
        {done && (
          <p role="status" className="text-sm text-muted">
            {done}
          </p>
        )}
      </div>
      {receipt && (
        <ReceiptSummary receipt={receipt} accountId={invoice.account_id} />
      )}
      {open === 'credit' && (
        <CreditNoteForm invoice={invoice} onDone={(m) => close('credit', m)} />
      )}
      {open === 'payment' && (
        <PaymentForm
          accountId={invoice.account_id}
          invoice={invoice}
          today={today}
          onDone={(r) => {
            close('payment', null)
            setReceipt(r)
          }}
        />
      )}
      {open === 'transfer' && (
        <TransferForm invoice={invoice} onDone={(m) => close('transfer', m)} />
      )}
      {open === 'journal' && (
        <JournalForm invoice={invoice} onDone={(m) => close('journal', m)} />
      )}
    </section>
  )
}

/** Record a payment from the file (allocated oldest invoices first). */
export function AccountPayment({
  accountId,
  today,
}: {
  accountId: string
  today: string
}) {
  const canPay = useCan('payments.manage')
  const [open, setOpen] = useState(false)
  const [receipt, setReceipt] = useState<Transaction | null>(null)
  if (!canPay) return null
  return (
    <div className="flex flex-col gap-3">
      {!open && (
        <div>
          <Button
            variant="secondary"
            onClick={() => {
              setReceipt(null)
              setOpen(true)
            }}
          >
            Record payment
          </Button>
        </div>
      )}
      {open && (
        <PaymentForm
          accountId={accountId}
          today={today}
          onDone={(r) => {
            setOpen(false)
            setReceipt(r)
          }}
        />
      )}
      {receipt && <ReceiptSummary receipt={receipt} accountId={accountId} />}
    </div>
  )
}
