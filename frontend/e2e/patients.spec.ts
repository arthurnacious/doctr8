import { expect, test } from '@playwright/test'

import { captureConsole, signInAs } from './helpers'
import type { Page } from '@playwright/test'

const table = (page: Page) => page.getByRole('table')
const bodyRows = (page: Page) => table(page).locator('tbody tr')

test.describe('patient list', () => {
  test('pages on the server and shows the agreed columns', async ({ page }) => {
    const consoleLines = captureConsole(page)
    await signInAs(page, 'reception@e2e.test', '/patients')
    await expect(page.getByRole('heading', { name: 'Patients' })).toBeVisible()
    await expect(bodyRows(page)).toHaveCount(25)
    await expect(page.getByText('Showing 1–25 of 31')).toBeVisible()
    await expect(table(page).locator('thead th')).toHaveText([
      'File',
      'Name',
      'Dep.',
      'Date of birth',
      'ID number',
      'Medical aid',
      'Main member',
    ])
    await page.getByRole('button', { name: 'Next' }).click()
    await expect(page.getByText('Showing 26–31 of 31')).toBeVisible()
    await expect(bodyRows(page)).toHaveCount(6)
    await expect(page).toHaveURL(/page=2/)
    expect(consoleLines.join('\n')).not.toMatch(
      /Listtest|Mokoena|8001015009087/,
    )
  })

  test('one search box: name, file number, ID number and membership number', async ({
    page,
  }) => {
    await signInAs(page, 'reception@e2e.test', '/patients')
    const box = page.getByLabel('Search patients')

    await box.fill('mokoena')
    await expect(bodyRows(page)).toHaveCount(1)
    const row = bodyRows(page).first()
    await expect(row).toContainText('Mokoena, Naledi')
    await expect(row).toContainText('00')
    await expect(row).toContainText('01 Jan 1980')
    await expect(row).toContainText('E2E Health · Core')
    // Masked in the list; the full number never reaches the page.
    await expect(row).toContainText('800101******7')
    await expect(page.locator('body')).not.toContainText('8001015009087')

    await box.fill('8001015009087')
    await expect(bodyRows(page)).toHaveCount(1)
    await box.fill('e2e-77')
    await expect(bodyRows(page)).toHaveCount(1)
    await box.fill('3')
    await expect(bodyRows(page).first()).toContainText('3')

    // Search text is never put in the URL.
    expect(page.url()).not.toContain('e2e-77')
    expect(page.url()).not.toContain('8001015009087')

    await box.fill('zzzz-nobody')
    await expect(page.getByText('No patients match that search')).toBeVisible()
  })

  test('active / inactive filter', async ({ page }) => {
    await signInAs(page, 'reception@e2e.test', '/patients')
    await page.getByLabel('Search patients').fill('ctive')
    await expect(page.getByText('No patients match that search')).toBeVisible()
    await page.getByText('Inactive', { exact: true }).click()
    await expect(bodyRows(page)).toHaveCount(1)
    await expect(bodyRows(page).first()).toContainText('Inactive')
    await page.getByText('All', { exact: true }).click()
    await expect(bodyRows(page)).toHaveCount(1)
  })

  test('keyboard: up/down through rows, Enter opens the file', async ({
    page,
  }) => {
    await signInAs(page, 'reception@e2e.test', '/patients')
    await expect(bodyRows(page)).toHaveCount(25)
    await bodyRows(page).first().focus()
    await page.keyboard.press('ArrowDown')
    await page.keyboard.press('ArrowDown')
    await page.keyboard.press('ArrowUp')
    await expect(bodyRows(page).nth(1)).toBeFocused()
    const label = await bodyRows(page).nth(1).getAttribute('aria-label')
    await page.keyboard.press('Enter')
    await expect(page).toHaveURL(/\/patients\/file\/[0-9a-f-]+\?patient=/)
    expect(label).toMatch(/^Open file \d+ for /)
  })

  test('a practice with no patients explains how to start', async ({
    page,
  }) => {
    await signInAs(page, 'owner@e2e.test', '/patients')
    await page.getByRole('button', { name: /Practice B/ }).click()
    await expect(page.getByText('No patients yet')).toBeVisible()
    await expect(page.getByText(/Create the first file/)).toBeVisible()
  })

  test('errors are shown with a retry', async ({ page }) => {
    await signInAs(page, 'reception@e2e.test', '/')
    let fail = true
    await page.route('**/api/patients/search', (route) =>
      fail ? route.fulfill({ status: 500, body: '{}' }) : route.continue(),
    )
    await page.goto('/patients')
    await expect(page.getByRole('alert')).toContainText('Something went wrong')
    fail = false
    await page.getByRole('button', { name: 'Try again' }).click()
    await expect(bodyRows(page)).toHaveCount(25)
  })
})
