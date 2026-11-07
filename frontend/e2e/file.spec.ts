import { expect, request, test } from '@playwright/test'

import { PASSWORD, signInAs } from './helpers'
import type { Page } from '@playwright/test'

const API = process.env.E2E_API_URL ?? 'http://127.0.0.1:3100'

async function openFromList(page: Page, query: string, rowText: string) {
  await page.getByLabel('Search patients').fill(query)
  await page
    .getByRole('table')
    .locator('tbody tr', { hasText: rowText })
    .click()
  await expect(page).toHaveURL(/\/patients\/file\/[0-9a-f-]+/)
}

test.describe('patient file', () => {
  test('shows the file: mode, who pays, medical aid, everyone on it', async ({
    page,
  }) => {
    await signInAs(page, 'reception@e2e.test', '/patients')
    await openFromList(page, 'dube', 'Dube, Ayanda')

    await expect(page.getByRole('heading', { level: 1 })).toHaveText(/File \d+/)
    await expect(page.getByText('Family per file')).toBeVisible()
    await expect(page.getByText('main member is 00')).toBeVisible()
    const whoPays = page.locator('section', {
      has: page.getByRole('heading', { name: 'Who pays' }),
    })
    await expect(whoPays).toContainText('Sipho Dube')
    const aid = page.locator('section', {
      has: page.getByRole('heading', { name: 'Medical aid', exact: true }),
    })
    await expect(aid).toContainText('E2E Health · Core')
    await expect(aid).toContainText('FAM-9')
    await expect(aid).toContainText('Sipho Dube')

    const people = page
      .getByRole('list', { name: 'Patients on this file' })
      .getByRole('button')
    await expect(people).toHaveText([
      /00\s*Sipho Dube\s*Main member/,
      /01\s*Ayanda Dube/,
    ])
    // Opened on the patient chosen in the list.
    await expect(
      page.getByRole('heading', { name: /Ayanda Dube/ }),
    ).toContainText('dependant 01')
    await expect(people.nth(1)).toHaveAttribute('aria-current', 'true')

    // Clicking someone else opens their details.
    await people.nth(0).click()
    await expect(
      page.getByRole('heading', { name: /Sipho Dube/ }),
    ).toContainText('dependant 00')
  })

  test('tabs: details, medical aid, and honest empty states', async ({
    page,
  }) => {
    await signInAs(page, 'reception@e2e.test', '/patients')
    await openFromList(page, 'dube', 'Dube, Ayanda')
    const tabs = page.getByRole('tab')
    await expect(tabs).toHaveText([
      'Details',
      'Medical aid',
      'Visits',
      'Billing',
    ])
    await expect(page.getByRole('tabpanel')).toContainText('0821110000')

    await page.getByRole('tab', { name: 'Details' }).focus()
    await page.keyboard.press('ArrowRight')
    await expect(
      page.getByRole('tab', { name: 'Medical aid' }),
    ).toHaveAttribute('aria-selected', 'true')
    await expect(page.getByRole('tabpanel')).toContainText('Dependant code')
    await expect(page.getByRole('tabpanel')).toContainText('01')

    await page.keyboard.press('ArrowRight')
    await expect(page.getByRole('tabpanel')).toHaveText(
      'Available once booking and billing are set up.',
    )
    await page.keyboard.press('End')
    await expect(page.getByRole('tab', { name: 'Billing' })).toBeFocused()
    await expect(page.getByRole('tabpanel')).toHaveText(
      'Available once booking and billing are set up.',
    )
    await expect(page).toHaveURL(/tab=billing/)
  })

  test("the full ID number appears only on the patient's own page", async ({
    page,
  }) => {
    await signInAs(page, 'reception@e2e.test', '/patients')
    await page.getByLabel('Search patients').fill('mokoena')
    await expect(page.getByRole('table')).toContainText('800101******7')
    await expect(page.locator('body')).not.toContainText('8001015009087')
    await page.getByRole('table').locator('tbody tr').first().click()
    await expect(page.getByRole('tabpanel')).toContainText('8001015009087')
    await expect(page.getByRole('tabpanel')).toContainText('South African ID')
    await expect(page.getByRole('tabpanel')).toContainText('01 Jan 1980')
  })

  test('refreshes when someone else edits the patient', async ({ page }) => {
    await signInAs(page, 'reception@e2e.test', '/patients')
    await openFromList(page, 'dube', 'Dube, Ayanda')
    await expect(page.getByRole('tabpanel')).toContainText('0821110000')

    // Another user (the owner, via the API) changes Ayanda's mobile number.
    const api = await request.newContext({ baseURL: API })
    const login = await api.post('/auth/login', {
      data: { email: 'owner@e2e.test', password: PASSWORD },
    })
    let token = (await login.json()).access_token as string
    const practices = (await login.json()).practices as Array<{
      id: string
      name: string
    }>
    const a = practices.find((p) => p.name === 'Practice A')!
    token = (
      await (
        await api.post('/auth/login', {
          data: {
            email: 'owner@e2e.test',
            password: PASSWORD,
            tenant_id: a.id,
          },
        })
      ).json()
    ).access_token
    const auth = { authorization: `Bearer ${token}` }
    const found = await (
      await api.post('/patients/search', {
        headers: auth,
        data: { query: 'ayanda dube' },
      })
    ).json()
    const personId = found.items[0].person_id as string
    const person = await (
      await api.get(`/persons/${personId}`, { headers: auth })
    ).json()
    const put = await api.put(`/persons/${personId}`, {
      headers: auth,
      data: {
        first_name: person.first_name,
        last_name: person.last_name,
        mobile: '0829990000',
      },
    })
    expect(put.ok()).toBe(true)

    // No reload: the WebSocket event refreshes the open file.
    await expect(page.getByRole('tabpanel')).toContainText('0829990000')
    await api.dispose()
  })

  test("a file that doesn't exist says so", async ({ page }) => {
    await signInAs(page, 'reception@e2e.test', '/')
    await page.goto('/patients/file/00000000-0000-0000-0000-000000000001')
    await expect(page.getByRole('alert')).toContainText("doesn't exist")
    await page.getByRole('link', { name: 'Patients' }).last().click()
    await expect(page).toHaveURL(/\/patients$/)
  })
})
