import {
  keepPreviousData,
  useMutation,
  useQuery,
  useQueryClient,
} from '@tanstack/react-query'

import { api } from '#/lib/api/client'
import type { CreateFileRequest } from './new-file/model'
import type {
  FileMode,
  PatientFile,
  PatientPage,
  PatientSearchParams,
  PersonDetails,
  PersonSummary,
  Scheme,
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
  settings: () => ['patients', 'settings'] as const,
  schemes: () => ['medical-schemes'] as const,
  personSearch: (q: string) => ['patients', 'person-search', q] as const,
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

/** The practice's file mode, for the new-file flow. */
export function useFileSettings() {
  return useQuery({
    queryKey: patientKeys.settings(),
    queryFn: () => api<{ file_mode: FileMode }>('/patient-files/settings'),
  })
}

export function useSchemes() {
  return useQuery({
    queryKey: patientKeys.schemes(),
    queryFn: () => api<Array<Scheme>>('/medical-schemes'),
    staleTime: 10 * 60_000,
  })
}

/** Find someone already in the practice (POST: names stay out of URLs). */
export function usePersonSearch(query: string) {
  const q = query.trim()
  return useQuery({
    queryKey: patientKeys.personSearch(q),
    queryFn: () =>
      api<Array<PersonSummary>>('/persons/search', {
        method: 'POST',
        json: { query: q, limit: 8 },
      }),
    enabled: q.length >= 2,
    placeholderData: keepPreviousData,
  })
}

/** One request creates people, membership, account and patients together. */
export function useCreateFile() {
  const qc = useQueryClient()
  return useMutation({
    mutationFn: (body: CreateFileRequest) =>
      api<PatientFile>('/patient-files', { method: 'POST', json: body }),
    onSuccess: (file) => {
      qc.setQueryData(patientKeys.file(file.id), file)
      void qc.invalidateQueries({ queryKey: patientKeys.lists() })
    },
  })
}
