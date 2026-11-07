import { expect, test } from '@playwright/test'

import { saId, setFileMode, signInAs } from './helpers'
import type { Page } from '@playwright/test'

const next = (page: Page) => page.getByRole('button', { name: 'Next' }).click()
const steps = (page: Page) =>
  page.getByRole('list', { name: 'Steps' }).getByRole('listitem')

async function startNewFile(
  page: Page,
  practice: 'Practice A' | 'Practice B' = 'Practice A',
) {
  await signInAs(page, 'owner@e2e.test', '/patients')
  if (practice === 'Practice B') {
    await page.getByRole('button', { name: /Practice A/ }).click()
    await page.getByRole('button', { name: 'Practice B' }).click()
    await expect(
      page.getByText("You're signed in to Practice B."),
    ).toBeVisible()
    await page.getByRole('link', { name: 'Patients' }).first().click()
  }
  await page.getByRole('link', { name: 'New file' }).first().click()
  await expect(page.getByRole('heading', { name: 'New file' })).toBeVisible()
}

test.describe('new patient file', () => {
  test.afterAll(async () => {
    await setFileMode('owner@e2e.test', 'Practice A', 'patient_per_file')
    await setFileMode('owner@e2e.test', 'Practice B', 'patient_per_file')
  })

  test('only users who may manage patients see "New file"', async ({
    page,
  }) => {
    await signInAs(page, 'reception@e2e.test', '/patients')
    await expect(page.getByRole('table')).toBeVisible()
    await expect(page.getByRole('link', { name: 'New file' })).toHaveCount(0)
    await page.goto('/patients/new')
    await expect(page).toHaveURL(/\/$/)
  })

  test('patient per file: validation, ID → date of birth, medical aid, created in one go', async ({
    page,
  }) => {
    await startNewFile(page)
    await expect(page.getByText('Patient per file.')).toBeVisible()
    await expect(steps(page)).toHaveText([
      '1. Patient',
      '2. Who pays',
      '3. Medical aid',
      '4. Review',
    ])

    await next(page)
    await expect(page.getByText('Enter a first name')).toBeVisible()
    await expect(page.getByText('Enter a surname')).toBeVisible()

    await page.getByLabel('First name').fill('Thabo')
    await page.getByLabel('Surname').fill('Ndlovu')
    await page.getByLabel('SA ID number').fill('9002150123089')
    await next(page)
    await expect(page.getByText(/isn't valid: check for a typo/)).toBeVisible()

    await page.getByLabel('SA ID number').fill('900215 0123 088')
    await expect(page.getByLabel('Date of birth')).toHaveValue('1990-02-15')
    await expect(page.getByText('Filled in from the ID number')).toBeVisible()
    await page.getByLabel('Mobile').fill('082 555 0101')
    await next(page)

    await expect(page.getByRole('heading', { name: 'Who pays' })).toBeFocused()
    await next(page) // the patient pays (default)

    await page.getByText('Medical aid', { exact: true }).last().click()
    await next(page)
    await expect(page.getByText('Choose the medical scheme')).toBeVisible()
    await page
      .getByLabel('Medical scheme')
      .selectOption({ label: 'E2E Health' })
    await page.getByLabel('Plan').selectOption({ label: 'Core' })
    await page.getByLabel('Membership number').fill('new 1')
    await next(page)

    await expect(
      page.getByText('Nothing is saved until you choose Create file.'),
    ).toBeVisible()
    await expect(page.getByRole('definition').first()).toContainText(
      'Thabo Ndlovu',
    )
    await page.getByRole('button', { name: 'Create file' }).click()

    await expect(page).toHaveURL(/\/patients\/file\/[0-9a-f-]+/)
    await expect(
      page.getByText('Patient per file', { exact: true }),
    ).toBeVisible()
    await expect(page.getByText('NEW1')).toBeVisible()
    await expect(page.getByRole('tabpanel')).toContainText('9002150123088')
    await expect(page.getByRole('tabpanel')).toContainText('15 Feb 1990')
  })

  test('the date of birth from an ID can be corrected (century)', async ({
    page,
  }) => {
    await startNewFile(page)
    const id = saId('250101580008')
    await page.getByLabel('First name').fill('Gogo')
    await page.getByLabel('Surname').fill('Century')
    await page.getByLabel('SA ID number').fill(id)
    await expect(page.getByLabel('Date of birth')).toHaveValue('2025-01-01')
    await page.getByLabel('Date of birth').fill('1925-01-02')
    await next(page)
    await expect(
      page.getByText("This doesn't match the date in the ID number"),
    ).toBeVisible()
    await page.getByLabel('Date of birth').fill('1925-01-01')
    await next(page)
    await next(page)
    await next(page)
    await page.getByRole('button', { name: 'Create file' }).click()
    await expect(page.getByRole('tabpanel')).toContainText('01 Jan 1925')
  })

  test("the backend's error is shown in plain language", async ({ page }) => {
    await startNewFile(page)
    await page.getByLabel('First name').fill('Thabo')
    await page.getByLabel('Surname').fill('Again')
    await page.getByLabel('SA ID number').fill('9002150123088')
    await next(page)
    await next(page)
    await next(page)
    await page.getByRole('button', { name: 'Create file' }).click()
    await expect(page.getByRole('alert')).toContainText('already exists')
    await expect(page.getByRole('alert')).toContainText(
      'Someone already in the practice',
    )
    await expect(page).toHaveURL(/\/patients\/new$/)
  })

  test('family per file: payer is main member (00), dependants 01 and 02, passport allowed', async ({
    page,
  }) => {
    await setFileMode('owner@e2e.test', 'Practice B', 'family_per_file')
    await startNewFile(page, 'Practice B')
    await expect(page.getByText('Family per file.')).toBeVisible()
    await expect(steps(page)).toHaveText([
      '1. Patient',
      '2. Who pays',
      '3. Medical aid',
      '4. Dependants',
      '5. Review',
    ])

    await page.getByLabel('First name').fill('Lerato')
    await page.getByLabel('Surname').fill('Zulu')
    await page.getByLabel('No document').check()
    await next(page)

    await page.getByLabel('Someone else, new to the practice').check()
    await page.getByLabel('First name').fill('Sipho')
    await page.getByLabel('Surname').fill('Zulu')
    await page.getByLabel('Passport').check()
    await page.getByLabel('Passport number').fill('ab12345')
    await next(page)
    await expect(
      page.getByText('Choose the country that issued the passport'),
    ).toBeVisible()
    await page.getByLabel('Issuing country').fill('zw')
    await next(page)

    await page.getByText('Medical aid', { exact: true }).last().click()
    await page
      .getByLabel('Medical scheme')
      .selectOption({ label: 'E2E Health' })
    await page.getByLabel('Plan').selectOption({ label: 'Core' })
    await page.getByLabel('Membership number').fill('ZULU-1')
    await page.getByLabel('The person who pays', { exact: true }).check()
    await page.getByLabel("Patient's dependant code").fill('01')
    await next(page)

    await page.getByRole('button', { name: 'Add dependant' }).click()
    await expect(page.getByLabel('Dependant code')).toHaveValue('02')
    await page.getByLabel('First name').fill('Musa')
    await page.getByLabel('Surname').fill('Zulu')
    await page.getByLabel('No document').check()
    await next(page)

    await expect(page.getByText('02 · Musa Zulu')).toBeVisible()
    await page.getByRole('button', { name: 'Create file' }).click()

    const people = page
      .getByRole('list', { name: 'Patients on this file' })
      .getByRole('button')
    await expect(people).toHaveText([
      /00\s*Sipho Zulu\s*Main member/,
      /01\s*Lerato Zulu/,
      /02\s*Musa Zulu/,
    ])
    await expect(
      page.getByText('Family per file', { exact: true }),
    ).toBeVisible()
  })

  test('new debtor per visit explains itself and has no dependants step', async ({
    page,
  }) => {
    await setFileMode('owner@e2e.test', 'Practice B', 'debtor_per_visit')
    await startNewFile(page, 'Practice B')
    await expect(
      page.getByText('opens a new account at each visit'),
    ).toBeVisible()
    await expect(steps(page)).toHaveText([
      '1. Patient',
      '2. Who pays',
      '3. Medical aid',
      '4. Review',
    ])
  })
})
