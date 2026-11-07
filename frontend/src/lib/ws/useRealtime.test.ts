import { describe, expect, it } from 'vitest'

import { createRealtimeBinding } from './useRealtime'

function setup() {
  const calls: Array<string> = []
  const pending: Array<() => void> = []
  const conn = {
    start: () => calls.push('start'),
    stop: () => calls.push('stop'),
    restart: () => calls.push('restart'),
  }
  const binding = createRealtimeBinding(conn, (fn) => {
    pending.push(fn)
    return () => pending.splice(pending.indexOf(fn), 1)
  })
  const flush = () => pending.splice(0).forEach((fn) => fn())
  return { calls, binding, flush }
}

describe('realtime binding', () => {
  it('opens once and closes on release', () => {
    const { calls, binding, flush } = setup()
    binding.acquire('a')
    binding.release()
    expect(calls).toEqual(['start'])
    flush()
    expect(calls).toEqual(['start', 'stop'])
  })

  it('keeps the socket when the layout remounts for the same practice', () => {
    const { calls, binding, flush } = setup()
    binding.acquire('a')
    binding.release()
    binding.acquire('a')
    flush()
    expect(calls).toEqual(['start'])
  })

  it('restarts for a different practice', () => {
    const { calls, binding, flush } = setup()
    binding.acquire('a')
    binding.release()
    binding.acquire('b')
    flush()
    expect(calls).toEqual(['start', 'restart'])
  })

  it('starts fresh after a real close', () => {
    const { calls, binding, flush } = setup()
    binding.acquire('a')
    binding.release()
    flush()
    binding.acquire('a')
    expect(calls).toEqual(['start', 'stop', 'start'])
  })
})
