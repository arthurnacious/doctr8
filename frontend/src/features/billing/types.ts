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
