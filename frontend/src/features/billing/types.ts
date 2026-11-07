/** GET /accounts/{id}/statement (backend/src/modules/billing/statements). */

export type TxnKind =
  | 'invoice'
  | 'claim_submitted'
  | 'remittance'
  | 'patient_payment'
  | 'refund'
  | 'credit_note'
  | 'journal'
  | 'liability_transfer'
  | 'reversal'

export interface StatementLine {
  transaction_id: string
  date: string
  kind: TxnKind
  description: string
  medical_aid_cents: number
  patient_cents: number
  /** Reversed on or before the statement date. */
  reversed: boolean
  reverses_transaction_id: string | null
}

export interface InvoiceLine {
  id: string
  line_no: number
  service_date: string
  tariff_code: string | null
  nappi_code: string | null
  description: string
  /** Decimal as a string, e.g. "1" or "2.5". */
  quantity: string
  unit_price_cents: number
  amount_cents: number
  credited_cents: number
  place_of_service: string
  icd10_codes: Array<string>
  modifiers: Array<string>
  assistant_provider_id: string | null
}

export interface StatementInvoice {
  id: string
  invoice_no: number
  invoice_date: string
  patient_id: string
  dependant_code: string | null
  treating_provider_id: string
  referring_provider_id: string | null
  treating_provider: string
  referring_provider: string | null
  service_centre: string
  authorisation_number: string | null
  total_cents: number
  lines: Array<InvoiceLine>
  transactions: Array<StatementLine>
  medical_aid_due_cents: number
  patient_due_cents: number
}

export interface StatementPatient {
  patient_id: string
  person_id: string
  dependant_code: string
  title: string | null
  first_name: string
  last_name: string
  date_of_birth: string | null
}

export interface StatementProvider {
  id: string
  name: string
  practice_number: string | null
  hpcsa_number: string | null
}

export interface Statement {
  as_of: string
  generated_at: string
  practice: {
    name: string
    pcns_number: string | null
    address_line1: string | null
    address_line2: string | null
    suburb: string | null
    city: string | null
    postal_code: string | null
    phone: string | null
    email: string | null
    bank_name: string | null
    bank_account_name: string | null
    bank_account_number: string | null
    bank_branch_code: string | null
  }
  account: {
    id: string
    account_no: number
    status: 'open' | 'closed'
    file_mode: 'patient_per_file' | 'family_per_file' | 'debtor_per_visit'
  }
  addressee: {
    person_id: string
    title: string | null
    first_name: string
    last_name: string
    address_line1: string | null
    address_line2: string | null
    suburb: string | null
    city: string | null
    postal_code: string | null
  }
  medical_aid: {
    scheme_name: string
    plan_name: string
    membership_number: string
    main_member_person_id: string
    main_member_name: string
  } | null
  patients: Array<StatementPatient>
  providers: Array<StatementProvider>
  invoices: Array<StatementInvoice>
  /** Money on the account not allocated to an invoice. */
  unallocated: Array<StatementLine>
  totals: {
    medical_aid_due_cents: number
    patient_due_cents: number
    total_due_cents: number
  }
  ageing: {
    bands: Array<string>
    medical_aid: Array<number>
    patient: Array<number>
    total: Array<number>
  }
}

// -----------------------------------------------------------------------------
// Invoices (backend/src/modules/billing/invoices)
// -----------------------------------------------------------------------------

export type ClaimStatus = 'submitted' | 'accepted' | 'rejected' | 'paid'

/** One row of POST /invoices/search and GET /accounts/{id}/invoices. */
export interface InvoiceListItem {
  id: string
  invoice_no: number
  invoice_date: string
  account_id: string
  account_no: number
  patient: {
    patient_id: string
    dependant_code: string
    first_name: string
    last_name: string
  }
  treating_provider_id: string
  treating_provider: string
  total_cents: number
  credited_cents: number
  medical_aid_due_cents: number
  patient_due_cents: number
  claim_status: ClaimStatus | null
}

export interface InvoicePage {
  items: Array<InvoiceListItem>
  total: number
  page: number
  page_size: number
}

export interface ProviderRef {
  id: string
  name: string
  practice_number: string | null
  hpcsa_number: string | null
}

export interface Claim {
  id: string
  invoice_id: string
  invoice_no: number
  account_id: string
  status: ClaimStatus
  claimed_cents: number
  submitted_date: string
  status_reason: string | null
  updated_at: string
}

export interface CreditNote {
  id: string
  credit_note_no: number
  credit_date: string
  reason: string
  total_cents: number
  lines: Array<{ invoice_line_id: string; amount_cents: number }>
}

/** GET /invoices/{id}: everything on the printed invoice. */
export interface Invoice {
  id: string
  invoice_no: number
  account_id: string
  account_no: number
  patient_id: string
  patient: {
    patient_id: string
    dependant_code: string
    title: string | null
    first_name: string
    last_name: string
    date_of_birth: string | null
    sex: string | null
  }
  medical_aid: Statement['medical_aid']
  practice: Statement['practice']
  transactions: Array<StatementLine>
  invoice_date: string
  treating_provider: ProviderRef
  referring_provider: ProviderRef | null
  service_centre: { id: string; kind: string; name: string }
  authorisation_number: string | null
  total_cents: number
  credited_cents: number
  medical_aid_due_cents: number
  patient_due_cents: number
  lines: Array<InvoiceLine>
  credit_notes: Array<CreditNote>
  claims: Array<Claim>
  created_at: string
}

export interface Provider {
  id: string
  kind: 'practice' | 'external'
  title: string | null
  first_name: string | null
  last_name: string
  practice_number: string | null
  hpcsa_number: string | null
  speciality: string | null
  is_active: boolean
}

export interface ServiceCentre {
  id: string
  kind: string
  name: string
  practice_number: string | null
  default_place_of_service: string | null
  is_active: boolean
}
