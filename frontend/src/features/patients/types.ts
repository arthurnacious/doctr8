/** Shapes returned by the people API (backend/src/modules/people). */

export type PatientStatus = 'active' | 'inactive'
export type StatusFilter = PatientStatus | 'all'

export interface MainMemberRef {
  person_id: string
  first_name: string
  last_name: string
}

/** One row of POST /patients/search. ID numbers arrive already masked. */
export interface PatientListItem {
  patient_id: string
  person_id: string
  account_id: string
  account_no: number
  dependant_code: string
  status: PatientStatus
  title: string | null
  first_name: string
  last_name: string
  date_of_birth: string | null
  id_number_masked: string | null
  scheme_name: string | null
  plan_name: string | null
  membership_number: string | null
  main_member: MainMemberRef | null
}

export interface PatientPage {
  items: Array<PatientListItem>
  total: number
  page: number
  page_size: number
}

export interface PatientSearchParams {
  query: string
  status: StatusFilter
  page: number
}
