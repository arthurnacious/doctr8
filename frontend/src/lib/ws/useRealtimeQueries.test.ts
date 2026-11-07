import { describe, expect, it, vi } from 'vitest'
import { QueryClient } from '@tanstack/react-query'

import { invalidateFor } from './useRealtimeQueries'

function client() {
  const qc = new QueryClient()
  const spy = vi.spyOn(qc, 'invalidateQueries')
  return { qc, keys: () => spy.mock.calls.map((c) => c[0]?.queryKey ?? 'ALL') }
}

describe('realtime → query invalidation', () => {
  it('a changed file refreshes lists, that file and people on screen', () => {
    const { qc, keys } = client()
    invalidateFor(qc, {
      type: 'event',
      event: 'patient_file.changed',
      id: 'a1',
      account_id: 'a1',
    })
    expect(keys()).toEqual([
      ['patients', 'list'],
      ['patients', 'file', 'a1'],
      ['patients', 'person'],
    ])
  })

  it('resync refreshes everything', () => {
    const { qc, keys } = client()
    invalidateFor(qc, { type: 'resync' })
    expect(keys()).toEqual(['ALL'])
  })

  it('ignores unrelated messages', () => {
    const { qc, keys } = client()
    invalidateFor(qc, { type: 'pong' })
    invalidateFor(qc, {
      type: 'event',
      event: 'claim.status_changed',
      id: 'c1',
    })
    expect(keys()).toEqual([])
  })
})
