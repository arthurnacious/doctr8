import { expect, test } from '@playwright/test'
import type { Page } from '@playwright/test'

import { signInAs } from './helpers'

async function openStatement(page: Page, search: string, row: RegExp) {
  await signInAs(page, 'owner@e2e.test', '/patients')
  await page.getByLabel('Search patients').fill(search)
  await page.getByRole('row', { name: row }).first().click()
  await page.getByRole('tab', { name: 'Billing' }).click()
  await page.getByRole('link', { name: 'Open statement' }).click()
  await expect(page.getByRole('heading', { name: /^Statement/ })).toBeVisible()
}

const sa = (daysAgo: number) => {
  const d = new Date(Date.now() - daysAgo * 86_400_000)
  return new Intl.DateTimeFormat('en-CA', {
    timeZone: 'Africa/Johannesburg',
  }).format(d)
}

test.describe('statement controls', () => {
  test('settled invoices start collapsed and can be expanded', async ({
    page,
  }) => {
    await openStatement(page, 'Lerato', /Lerato/)
    const settled = page.getByRole('button', { name: /^Invoice 1 .*Settled/ })
    const open = page.getByRole('button', { name: /^Invoice 2 / })
    await expect(settled).toHaveAttribute('aria-expanded', 'false')
    await expect(open).toHaveAttribute('aria-expanded', 'true')
    await expect(page.getByText('Consultation, new patient')).toBeHidden()

    await settled.click()
    await expect(settled).toHaveAttribute('aria-expanded', 'true')
    await expect(page.getByText('Consultation, new patient')).toBeVisible()

    await page.getByRole('button', { name: 'Collapse all' }).click()
    await expect(open).toHaveAttribute('aria-expanded', 'false')
    await expect(settled).toHaveAttribute('aria-expanded', 'false')
    await page.getByRole('button', { name: 'Expand all' }).click()
    await expect(open).toHaveAttribute('aria-expanded', 'true')
    await expect(settled).toHaveAttribute('aria-expanded', 'true')
  })

  test('reversed journals are hidden until asked for', async ({ page }) => {
    await openStatement(page, 'Lerato', /Lerato/)
    const st = page.getByRole('article', { name: 'Statement' })
    const hide = page.getByLabel('Hide reversed journals')
    await expect(hide).toBeChecked()
    await expect(st).not.toContainText('Journal:')

    await hide.uncheck()
    await expect(st).toContainText('Journal: Small balance write-off')
    await expect(st).toContainText('(reversed)')
    await expect(st).toContainText('Reversal')
    await expect(page).toHaveURL(/reversed=shown/)

    await hide.check()
    await expect(st).not.toContainText('Journal:')
  })

  test('a statement as of an earlier date', async ({ page }) => {
    await openStatement(page, 'Lerato', /Lerato/)
    const date = page.getByLabel('Statement as of')
    await expect(date).toHaveValue(sa(0))

    await date.fill(sa(50))
    await expect(page).toHaveURL(new RegExp(`as_of=${sa(50)}`))
    const st = page.getByRole('article', { name: 'Statement' })
    await expect(st.locator('.statement-visit')).toHaveCount(1)
    // Fifty days ago the medical aid still owed the whole first visit.
    const totals = st.getByRole('region', { name: 'Totals' })
    await expect(totals).toContainText(/Total due\s*R\s520\.00/)
    await expect(st.locator('header')).toContainText(
      new Intl.DateTimeFormat('en-ZA', {
        day: '2-digit',
        month: 'short',
        year: 'numeric',
        timeZone: 'UTC',
      }).format(new Date(`${sa(50)}T00:00:00Z`)),
    )

    await date.fill(sa(-1))
    await expect(
      page.getByText("A statement can't be dated in the future"),
    ).toBeVisible()
    await expect(page).toHaveURL(new RegExp(`as_of=${sa(50)}`))
  })

  test('filter to one patient on a family file', async ({ page }) => {
    await openStatement(page, 'dube', /Dube, Sipho/)
    const st = page.getByRole('article', { name: 'Statement' })
    await expect(st.locator('.statement-visit')).toHaveCount(2)

    await page.getByLabel('Patient').selectOption({ label: '01 · Ayanda Dube' })
    await expect(st.locator('.statement-visit')).toHaveCount(1)
    await expect(st.locator('.statement-visit')).toContainText(
      'Ayanda Dube (01)',
    )
    await expect(
      page.getByRole('status').filter({ hasText: 'Showing' }),
    ).toContainText('Showing Ayanda Dube’s invoices only')
    // Totals stay for the whole account.
    await expect(st.getByRole('region', { name: 'Totals' })).toContainText(
      /Total due\s*R\s1\s040\.00/,
    )

    await page
      .getByLabel('Patient')
      .selectOption({ label: 'Everyone on the file' })
    await expect(st.locator('.statement-visit')).toHaveCount(2)
  })
})
