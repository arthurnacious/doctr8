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

export type FileMode =
  'patient_per_file' | 'family_per_file' | 'debtor_per_visit'

/** GET /patient-files/{id}: the file (account) as patient staff see it. */
export interface PatientFile {
  id: string
  account_no: number
  file_mode: FileMode
  status: 'open' | 'closed'
  guarantor: { id: string; first_name: string; last_name: string }
  medical_aid: {
    membership_id: string
    scheme_name: string
    plan_name: string
    membership_number: string
    main_member_person_id: string
  } | null
  patients: Array<{
    patient_id: string
    person_id: string
    first_name: string
    last_name: string
    dependant_code: string
    status: PatientStatus
  }>
  created_at: string
  updated_at: string
}

export type Sex = 'female' | 'male' | 'other' | 'unknown'
export type IdType = 'sa_id' | 'passport' | 'other'

/** GET /persons/{id}: full details, including the unmasked ID number. */
export interface PersonDetails {
  id: string
  title: string | null
  first_name: string
  last_name: string
  preferred_name: string | null
  date_of_birth: string | null
  sex: Sex | null
  id_type: IdType | null
  id_number: string | null
  id_country: string | null
  email: string | null
  mobile: string | null
  home_phone: string | null
  address_line1: string | null
  address_line2: string | null
  suburb: string | null
  city: string | null
  postal_code: string | null
  created_at: string
  updated_at: string
}
