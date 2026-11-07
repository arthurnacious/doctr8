import { expect } from '@playwright/test'
import type { Page } from '@playwright/test'

export const PASSWORD = process.env.E2E_PASSWORD ?? 'correct horse battery'

/** Sign in through the real screens, including the two-step stub. */
export async function signInAs(page: Page, email: string, redirect = '/') {
  await page.goto(`/sign-in?redirect=${encodeURIComponent(redirect)}`)
  await page.getByLabel('Email').fill(email)
  await page.getByLabel('Password', { exact: true }).fill(PASSWORD)
  await page.getByRole('button', { name: 'Sign in' }).click()
  await expect(page).toHaveURL(/\/two-step|\/choose-practice|^(?!.*sign-in)/)
  if (page.url().includes('/two-step')) {
    await page.getByLabel('6-digit code').fill('000000')
    await page.getByRole('button', { name: 'Verify' }).click()
  }
}

/** Collect console output so tests can assert patient details never appear. */
export function captureConsole(page: Page): Array<string> {
  const lines: Array<string> = []
  page.on('console', (m) => lines.push(m.text()))
  page.on('pageerror', (e) => lines.push(String(e)))
  return lines
}
