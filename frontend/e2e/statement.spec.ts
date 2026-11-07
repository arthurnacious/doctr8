import { expect, test } from '@playwright/test'
import type { Page } from '@playwright/test'

import { apiAs, captureConsole, signInAs } from './helpers'

/** Open a patient's file from the list, then its statement. */
async function openStatement(page: Page, name: string) {
  await signInAs(page, 'owner@e2e.test', '/patients')
  await page.getByLabel('Search patients').fill(name)
  await page
    .getByRole('row', { name: new RegExp(name.split(' ')[0]) })
    .first()
    .click()
  await page.getByRole('tab', { name: 'Billing' }).click()
  await page.getByRole('link', { name: 'Open statement' }).click()
  await expect(page.getByRole('heading', { name: /^Statement/ })).toBeVisible()
}

const money = (r: string) => new RegExp(`^-?R\\s${r.replace('.', '\\.')}$`)

test.describe('account statement', () => {
  test('shows the header, totals, visits and ageing', async ({ page }) => {
    const logs = captureConsole(page)
    await openStatement(page, 'Lerato Statement')
    const st = page.getByRole('article', { name: 'Statement' })

    const header = st.locator('header')
    await expect(header).toContainText('Practice A')
    await expect(header).toContainText('1 Main Rd')
    await expect(header).toContainText('Patient per file')
    await expect(header).toContainText('E2E Health')
    await expect(header).toContainText('Core')
    await expect(header).toContainText('STM-1')
    await expect(header).toContainText('00Lerato Statement · born 12 Apr 1985')

    const totals = st.getByRole('region', { name: 'Totals' })
    await expect(totals.getByText(money('65.00'))).toBeVisible()
    await expect(totals.getByText(money('25.00'))).toBeVisible()
    await expect(totals.getByText(money('40.00'))).toBeVisible()

    // Two visits, newest last.
    const visits = st.locator('.statement-visit')
    await expect(visits).toHaveCount(2)
    const latest = visits.last()
    await expect(latest).toContainText(
      'Dr T Doctor · Pr 1234567 · HPCSA MP0123456',
    )
    await expect(latest).toContainText('Referred by')
    await expect(latest).toContainText('Dr Referrer · Pr 7654321')
    await expect(latest).toContainText('AUTH77')
    await expect(latest).toContainText('Assistant: Dr Assist · HPCSA MP7777777')
    await expect(
      latest.getByRole('cell', { name: '0018', exact: true }),
    ).toBeVisible()
    await expect(
      latest.getByRole('cell', { name: 'J06.9, Z00.0' }),
    ).toBeVisible()
    await expect(latest).toContainText('Medical aid payment')
    // The reversed write-off cancels out, so it isn't shown.
    await expect(latest).not.toContainText('Journal')

    const owed = latest
      .getByRole('definition')
      .filter({ hasText: money('25.00') })
    await expect(owed).toHaveCount(1)

    // The EFT was allocated to the open invoice, so nothing is unallocated.
    await expect(latest).toContainText('Payment received (EFT) EFT-1')
    await expect(
      st.getByRole('region', { name: 'Not allocated to an invoice' }),
    ).toHaveCount(0)

    const ageing = st.getByRole('table', { name: 'Amounts outstanding by age' })
    const totalRow = ageing.getByRole('row', { name: /^Total/ })
    await expect(totalRow.getByRole('cell').first()).toHaveText(money('65.00'))

    expect(logs.join('\n')).not.toMatch(/Lerato|STM-1|1985/)
  })

  test('an account with no invoices says so', async ({ page }) => {
    await openStatement(page, 'Naledi')
    await expect(page.getByText('No invoices on this account')).toBeVisible()
    await expect(page.getByRole('region', { name: 'Totals' })).toHaveCount(0)
  })

  test('updates while open when a payment comes in', async ({ page }) => {
    await openStatement(page, 'Lerato Statement')
    const accountId = page.url().split('/statement/')[1].split('?')[0]
    const totals = page.getByRole('region', { name: 'Totals' })
    await expect(totals.getByText(money('40.00'))).toBeVisible()

    const { api, headers } = await apiAs('owner@e2e.test', 'Practice A')
    const r = await api.post('/payments', {
      headers,
      data: { account_id: accountId, method: 'cash', amount_cents: 1000 },
    })
    expect(r.ok()).toBe(true)
    await api.dispose()

    await expect(totals.getByText(money('30.00'))).toBeVisible()
    await expect(totals.getByText(money('55.00'))).toBeVisible()
  })

  test('staff without billing access see no statement', async ({ page }) => {
    await signInAs(page, 'reception@e2e.test', '/patients')
    await page.getByLabel('Search patients').fill('Lerato Statement')
    await page.getByRole('row', { name: /Lerato/ }).click()
    const accountId = page.url().split('/file/')[1].split('?')[0]
    await page.getByRole('tab', { name: 'Billing' }).click()
    await expect(
      page.getByText("You don't have access to billing"),
    ).toBeVisible()
    await expect(
      page.getByRole('link', { name: 'Open statement' }),
    ).toHaveCount(0)

    await page.goto(`/patients/statement/${accountId}`)
    await expect(page).toHaveURL(/\/$/)
  })
})
