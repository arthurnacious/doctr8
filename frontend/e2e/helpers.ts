import { expect } from '@playwright/test'
import type { Page } from '@playwright/test'

export const PASSWORD = process.env.E2E_PASSWORD ?? 'correct horse battery'

/**
 * Sign in through the real screens, including the two-step stub and, for
 * users in several practices, the practice chooser.
 */
export async function signInAs(
  page: Page,
  email: string,
  redirect = '/',
  practice = 'Practice A',
) {
  await page.goto(`/sign-in?redirect=${encodeURIComponent(redirect)}`)
  await page.getByLabel('Email').fill(email)
  await page.getByLabel('Password', { exact: true }).fill(PASSWORD)
  await page.getByRole('button', { name: 'Sign in' }).click()
  await expect(page).not.toHaveURL(/\/sign-in/)
  if (page.url().includes('/two-step')) {
    await page.getByLabel('6-digit code').fill('000000')
    await page.getByRole('button', { name: 'Verify' }).click()
    await expect(page).not.toHaveURL(/\/two-step/)
  }
  if (page.url().includes('/choose-practice')) {
    await page.getByRole('button', { name: new RegExp(practice) }).click()
    await expect(page).not.toHaveURL(/\/choose-practice/)
  }
}

/** Collect console output so tests can assert patient details never appear. */
export function captureConsole(page: Page): Array<string> {
  const lines: Array<string> = []
  page.on('console', (m) => lines.push(m.text()))
  page.on('pageerror', (e) => lines.push(String(e)))
  return lines
}

export const API_URL = process.env.E2E_API_URL ?? 'http://127.0.0.1:3100'

/** Call the API directly as a user in a practice (no two-step: API only). */
export async function apiAs(email: string, practiceName: string) {
  const { request } = await import('@playwright/test')
  const api = await request.newContext({ baseURL: API_URL })
  const first = await (
    await api.post('/auth/login', { data: { email, password: PASSWORD } })
  ).json()
  const practice = (
    first.practices as Array<{ id: string; name: string }>
  ).find((p) => p.name === practiceName)
  if (!practice) throw new Error(`${email} is not in ${practiceName}`)
  const login = await (
    await api.post('/auth/login', {
      data: { email, password: PASSWORD, tenant_id: practice.id },
    })
  ).json()
  const headers = { authorization: `Bearer ${login.access_token as string}` }
  return { api, headers }
}

/** Append the Luhn check digit to 12 digits: a valid SA ID number. */
export function saId(twelve: string): string {
  for (let c = 0; c <= 9; c++) {
    const id = twelve + c
    let sum = 0
    for (let i = 0; i < id.length; i++) {
      let d = Number(id[id.length - 1 - i])
      if (i % 2 === 1) {
        d *= 2
        if (d > 9) d -= 9
      }
      sum += d
    }
    if (sum % 10 === 0) return id
  }
  throw new Error('unreachable')
}

export async function setFileMode(
  email: string,
  practiceName: string,
  mode: string,
) {
  const { api, headers } = await apiAs(email, practiceName)
  const r = await api.patch('/tenants/current', {
    headers,
    data: { file_mode: mode },
  })
  if (!r.ok()) throw new Error(`could not set file mode: ${r.status()}`)
  await api.dispose()
}
