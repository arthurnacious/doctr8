import { expect, test } from '@playwright/test'

import { signInAs } from './helpers'

test.describe('printable statement', () => {
  test.beforeEach(async ({ page }) => {
    await signInAs(page, 'owner@e2e.test', '/patients')
    await page.getByLabel('Search patients').fill('Lerato')
    await page.getByRole('row', { name: /Lerato/ }).click()
    await page.getByRole('tab', { name: 'Billing' }).click()
    await page.getByRole('link', { name: 'Open statement' }).click()
    await expect(
      page.getByRole('table', { name: 'Amounts outstanding by age' }),
    ).toBeVisible()
  })

  test('the button opens the browser print dialog', async ({ page }) => {
    await page.evaluate(() => {
      ;(window as unknown as { printed: number }).printed = 0
      window.print = () => {
        ;(window as unknown as { printed: number }).printed++
      }
    })
    await page.getByRole('button', { name: 'Print or save as PDF' }).click()
    expect(
      await page.evaluate(
        () => (window as unknown as { printed: number }).printed,
      ),
    ).toBe(1)
  })

  test('prints light, in full, without the app around it', async ({ page }) => {
    // Dark theme on screen; paper is always white.
    await page.emulateMedia({ media: 'print' })
    const bg = await page.evaluate(
      () => getComputedStyle(document.body).backgroundColor,
    )
    expect(bg).toBe('rgb(255, 255, 255)')
    const color = await page
      .getByText('Practice A', { exact: true })
      .first()
      .evaluate((el) => getComputedStyle(el).color)
    expect(color).toBe('rgb(23, 23, 23)')

    for (const hidden of [
      page.getByRole('navigation').first(),
      page.getByRole('group', { name: 'Statement options' }),
      page.getByRole('button', { name: 'Print or save as PDF' }),
      page.getByRole('button', { name: 'Expand all' }),
    ]) {
      await expect(hidden).toBeHidden()
    }
    // The settled invoice is collapsed on screen but printed in full.
    await expect(page.getByText('Consultation, new patient')).toBeVisible()

    const closing = page.getByRole('region', {
      name: 'Payment and responsibility',
    })
    await expect(closing).toContainText('First Bank')
    await expect(closing).toContainText('6212345678')
    await expect(closing).toContainText('250655')
    await expect(closing).toContainText(
      'Medical aid and patient responsibility',
    )
    await expect(closing).toContainText(/printed/)

    // Page header and numbers for every page.
    const css = await page
      .locator('style')
      .evaluateAll((els) => els.map((e) => e.textContent).join('\n'))
    expect(css).toContain('"Practice A · Account')
    expect(css).toContain('counter(pages)')
  })
})
