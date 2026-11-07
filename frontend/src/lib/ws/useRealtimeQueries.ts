import { useEffect } from 'react'
import { useQueryClient } from '@tanstack/react-query'

import { getRealtime } from './useRealtime'
import type { QueryClient } from '@tanstack/react-query'
import type { RealtimeMessage } from './realtime'

/**
 * Turn realtime events into query invalidations. Events carry ids only;
 * the refetch goes through the API, so permissions are always checked.
 */
export function invalidateFor(qc: QueryClient, msg: RealtimeMessage): void {
  if (msg.type === 'resync') {
    void qc.invalidateQueries()
    return
  }
  if (msg.type !== 'event') return
  if (msg.event === 'patient_file.changed') {
    void qc.invalidateQueries({ queryKey: ['patients', 'list'] })
    if (msg.account_id) {
      void qc.invalidateQueries({
        queryKey: ['patients', 'file', msg.account_id],
      })
    }
    // A person's details may have changed; refetch whichever are on screen.
    void qc.invalidateQueries({ queryKey: ['patients', 'person'] })
    // So does the medical aid (plan, number) shown in an edit form.
    void qc.invalidateQueries({ queryKey: ['patients', 'membership'] })
  }
}

export function useRealtimeQueries(): void {
  const qc = useQueryClient()
  useEffect(() => getRealtime().subscribe((m) => invalidateFor(qc, m)), [qc])
}
