import { keepPreviousData, useQuery } from '@tanstack/react-query'

import { api } from '#/lib/api/client'
import type {
  PatientFile,
  PatientPage,
  PatientSearchParams,
  PersonDetails,
} from './types'

export const PAGE_SIZE = 25

/**
 * Query keys for the people module. Everything starts with 'patients', so
 * `invalidateQueries({ queryKey: patientKeys.all })` refreshes it all.
 */
export const patientKeys = {
  all: ['patients'] as const,
  lists: () => ['patients', 'list'] as const,
  list: (p: PatientSearchParams) => ['patients', 'list', p] as const,
  file: (accountId: string) => ['patients', 'file', accountId] as const,
  people: () => ['patients', 'person'] as const,
  person: (personId: string) => ['patients', 'person', personId] as const,
}

/** The patient list. POST so names and ID numbers never go in a URL. */
export function searchPatients(p: PatientSearchParams): Promise<PatientPage> {
  return api<PatientPage>('/patients/search', {
    method: 'POST',
    json: {
      query: p.query.trim() || undefined,
      status: p.status,
      page: p.page,
      page_size: PAGE_SIZE,
    },
  })
}

export function usePatientList(p: PatientSearchParams) {
  return useQuery({
    queryKey: patientKeys.list(p),
    queryFn: () => searchPatients(p),
    // Keep showing the previous page while the next one loads.
    placeholderData: keepPreviousData,
  })
}

export function usePatientFile(accountId: string) {
  return useQuery({
    queryKey: patientKeys.file(accountId),
    queryFn: () => api<PatientFile>(`/patient-files/${accountId}`),
  })
}

export function usePerson(personId: string | undefined) {
  return useQuery({
    queryKey: patientKeys.person(personId ?? ''),
    queryFn: () => api<PersonDetails>(`/persons/${personId}`),
    enabled: Boolean(personId),
  })
}
