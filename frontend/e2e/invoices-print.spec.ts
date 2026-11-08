import { expect, test } from '@playwright/test'

import { signInAs } from './helpers'

test.describe('printable invoice', () => {
  test.beforeEach(async ({ page }) => {
    await signInAs(page, 'owner@e2e.test', '/billing')
    await page.getByRole('link', { name: '2', exact: true }).click()
    await expect(
      page.getByRole('table', { name: 'Invoice lines' }),
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

  test('prints light, with everything on the invoice', async ({ page }) => {
    await page.emulateMedia({ media: 'print' })
    expect(
      await page.evaluate(
        () => getComputedStyle(document.body).backgroundColor,
      ),
    ).toBe('rgb(255, 255, 255)')
    for (const hidden of [
      page.getByRole('navigation').first(),
      page.getByRole('button', { name: 'Print or save as PDF' }),
      page.getByRole('region', { name: 'Invoice actions' }),
    ]) {
      await expect(hidden).toBeHidden()
    }
    const inv = page.getByRole('article', { name: 'Invoice 2' })
    const header = inv.locator('header')
    await expect(header).toContainText('Practice A')
    await expect(header).toContainText('Lerato Statement (dependant 00)')
    await expect(header).toContainText('STM-1')
    await expect(header).toContainText('Claim accepted')
    await expect(header).toContainText('AUTH77')
    await expect(
      inv.getByRole('table', { name: 'Invoice lines' }),
    ).toContainText('0191')
    await expect(inv.getByLabel('Invoice totals')).toBeVisible()
    const pay = inv.getByRole('region', { name: 'Payment details' })
    await expect(pay).toContainText('First Bank')
    await expect(pay).toContainText('6212345678')
    await expect(pay).toContainText(/printed/)

    const css = await page
      .locator('style')
      .evaluateAll((els) => els.map((e) => e.textContent).join('\n'))
    expect(css).toContain('"Practice A · Invoice 2"')
    expect(css).toContain('counter(pages)')
  })
})
