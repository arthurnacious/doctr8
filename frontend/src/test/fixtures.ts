import type { AuthResponse } from '#/lib/auth/session'

export const PRACTICE_A = { id: 'p-a', slug: 'a', name: 'Practice A' }
export const PRACTICE_B = { id: 'p-b', slug: 'b', name: 'Practice B' }

export function authResponse(over: Partial<AuthResponse> = {}): AuthResponse {
  return {
    access_token: 'token-1',
    token_type: 'Bearer',
    expires_in: 600,
    user: {
      id: 'u-1',
      email: 'thabo@example.co.za',
      full_name: 'Thabo Mokoena',
      email_verified_at: null,
      created_at: '2026-11-01T00:00:00Z',
    },
    practice: PRACTICE_A,
    permissions: ['patients.view', 'billing.view'],
    practices: [PRACTICE_A],
    ...over,
  }
}

export function json(status: number, body: unknown): Response {
  return new Response(body === undefined ? null : JSON.stringify(body), {
    status,
    headers: { 'Content-Type': 'application/json' },
  })
}

export function apiError(status: number, code: string, message: string) {
  return json(status, { error: { code, message } })
}
