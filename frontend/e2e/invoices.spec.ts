import { expect, test } from '@playwright/test'
import type { Page } from '@playwright/test'

import { apiAs, captureConsole, signInAs } from './helpers'

async function openFileBilling(page: Page, search: string, row: RegExp) {
  await signInAs(page, 'owner@e2e.test', '/patients')
  await page.getByLabel('Search patients').fill(search)
  await page.getByRole('row', { name: row }).first().click()
  await page.getByRole('tab', { name: 'Billing' }).click()
}

const invoices = (page: Page) =>
  page.getByRole('table', { name: /Invoices/ }).locator('tbody tr')

test.describe('invoices', () => {
  test('the file lists its invoices and opens one', async ({ page }) => {
    const logs = captureConsole(page)
    await openFileBilling(page, 'Lerato', /Lerato/)
    await expect(invoices(page)).toHaveCount(2)
    await page.getByRole('link', { name: '2', exact: true }).click()
    await expect(page).toHaveURL(/\/billing\/invoices\/[0-9a-f-]+$/)

    const inv = page.getByRole('article', { name: 'Invoice 2' })
    const header = inv.locator('header')
    await expect(header).toContainText('Practice A')
    await expect(header).toContainText('Lerato Statement (dependant 00)')
    await expect(header).toContainText('Born 12 Apr 1985')
    await expect(header).toContainText('E2E Health · Core')
    await expect(header).toContainText('STM-1')
    await expect(header).toContainText(
      'Dr T Doctor · Pr 1234567 · HPCSA MP0123456',
    )
    await expect(header).toContainText('Dr Referrer · Pr 7654321')
    await expect(header).toContainText('AUTH77')
    await expect(header).toContainText('Claim accepted')

    const lines = inv.getByRole('table', { name: 'Invoice lines' })
    await expect(lines.locator('tbody tr')).toHaveCount(2)
    await expect(lines).toContainText('Consultation, established patient')
    await expect(lines).toContainText('Assistant: Dr Assist · HPCSA MP7777777')
    await expect(lines).toContainText('716541001')

    await expect(inv.getByLabel('Invoice totals')).toContainText(
      /Net\s*R\s415\.00/,
    )
    await expect(inv.getByLabel('Still owed')).toContainText(
      /Medical aid owes\s*R\s25\.00/,
    )

    const txns = inv.getByRole('table', { name: 'Transactions on invoice 2' })
    await expect(txns).toContainText('Medical aid payment RA-E2E')
    await expect(txns).toContainText('Liability transfer')
    // Reversed entries show on the invoice itself, marked.
    await expect(txns).toContainText('(reversed)')

    expect(logs.join('\n')).not.toMatch(/Lerato|STM-1/)
  })

  test('the practice-wide list filters', async ({ page }) => {
    await signInAs(page, 'owner@e2e.test', '/billing')
    await expect(page.getByRole('heading', { name: 'Billing' })).toBeVisible()
    await expect(invoices(page)).toHaveCount(4)
    await expect(page.getByRole('status')).toContainText('Showing 1–4 of 4')

    await page.getByLabel('Status').selectOption('settled')
    await expect(invoices(page)).toHaveCount(1)
    await expect(invoices(page).first()).toContainText('Lerato Statement')
    await expect(page).toHaveURL(/status=settled/)

    await page.getByLabel('Status').selectOption('')
    await page.getByLabel('Claim').selectOption('none')
    await expect(invoices(page)).toHaveCount(2)
    await expect(invoices(page).filter({ hasText: 'Dube' })).toHaveCount(2)

    await page.getByLabel('Claim').selectOption('')
    await page.getByLabel('Patient').fill('Ayanda')
    await page.getByRole('button', { name: /Dube, Ayanda/ }).click()
    await expect(invoices(page)).toHaveCount(1)
    await expect(invoices(page).first()).toContainText('Ayanda Dube (01)')
    await expect(page.getByText('Dube, Ayanda (01)')).toBeVisible()
    // Only an id in the URL, never the name.
    expect(page.url()).not.toMatch(/Ayanda|Dube/i)
    await page.getByRole('button', { name: 'Clear the patient filter' }).click()
    await expect(invoices(page)).toHaveCount(4)

    const today = new Intl.DateTimeFormat('en-CA', {
      timeZone: 'Africa/Johannesburg',
    }).format(new Date())
    await page.getByLabel('From').fill(today)
    await expect(invoices(page)).toHaveCount(3)
    await page.getByLabel('Doctor').selectOption({ label: 'Dr T Doctor' })
    await expect(invoices(page)).toHaveCount(3)
  })

  test('staff without billing access see no invoices', async ({ page }) => {
    await signInAs(page, 'reception@e2e.test', '/billing')
    await expect(page).toHaveURL(/\/$/)
    await expect(page.getByRole('link', { name: 'Billing' })).toHaveCount(0)
  })

  test('an open invoice updates when its claim changes', async ({ page }) => {
    await openFileBilling(page, 'dube', /Dube, Ayanda/)
    await invoices(page).first().getByRole('link').click()
    const header = page
      .getByRole('article', { name: /^Invoice/ })
      .locator('header')
    await expect(header).toContainText('Not claimed')
    const invoiceId = page.url().split('/invoices/')[1]

    const { api, headers } = await apiAs('owner@e2e.test', 'Practice A')
    const r = await api.post(`/invoices/${invoiceId}/claims`, {
      headers,
      data: {},
    })
    expect(r.ok()).toBe(true)
    await api.dispose()

    await expect(header).toContainText('Claim submitted')
  })
})
