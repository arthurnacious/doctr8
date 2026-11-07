/**
 * The patient-list search text, kept in memory only.
 *
 * It may be a name or an ID number, so it never goes in the URL (browser
 * history, logs) or web storage. It survives navigating to a file and back,
 * and is forgotten on sign-out or reload.
 */
import { useSyncExternalStore } from 'react'

import { subscribe as onSessionChange, getSession } from '#/lib/auth/session'

let query = ''
const listeners = new Set<() => void>()

export function getListQuery(): string {
  return query
}

export function setListQuery(next: string): void {
  query = next
  for (const l of listeners) l()
}

function subscribe(l: () => void) {
  listeners.add(l)
  return () => listeners.delete(l)
}

export function useListQuery(): [string, (q: string) => void] {
  return [useSyncExternalStore(subscribe, getListQuery, () => ''), setListQuery]
}

if (typeof window !== 'undefined') {
  onSessionChange(() => {
    if (getSession().status !== 'signed_in' && query) setListQuery('')
  })
}
