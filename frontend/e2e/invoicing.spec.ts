import { expect, test } from '@playwright/test'
import type { Page } from '@playwright/test'

import { apiAs, captureConsole, signInAs } from './helpers'

/** A file of its own for each test, so other specs' numbers don't move. */
async function createFile(
  first: string,
  person: Record<string, string> = {},
  medicalAid = false,
): Promise<string> {
  const { api, headers } = await apiAs('owner@e2e.test', 'Practice A')
  let aid: object | undefined
  if (medicalAid) {
    const schemes = (await (
      await api.get('/medical-schemes', { headers })
    ).json()) as Array<{
      id: string
      name: string
      plans: Array<{ id: string }>
    }>
    const s = schemes.find((x) => x.name === 'E2E Health')!
    aid = {
      new: {
        scheme_id: s.id,
        plan_id: s.plans[0].id,
        membership_number: `INV-${first.toUpperCase()}`,
        main_member: { patient: 0 },
      },
    }
  }
  const r = await api.post('/patient-files', {
    headers,
    data: {
      patients: [
        {
          person: {
            new: { first_name: first, last_name: 'Invoicing', ...person },
          },
        },
      ],
      guarantor: { patient: 0 },
      medical_aid: aid,
    },
  })
  if (!r.ok()) throw new Error(`create file: ${r.status()} ${await r.text()}`)
  const id = ((await r.json()) as { id: string }).id
  await api.dispose()
  return id
}

async function newInvoice(page: Page, accountId: string) {
  await signInAs(
    page,
    'owner@e2e.test',
    `/patients/file/${accountId}?tab=billing`,
  )
  await page.getByRole('button', { name: 'New invoice' }).click()
  await expect(page).toHaveURL(/\/billing\/drafts\/[0-9a-f-]+$/)
  await expect(page.getByRole('heading', { name: /New invoice/ })).toBeVisible()
}

async function header(page: Page) {
  await page
    .getByLabel('Treating doctor')
    .selectOption({ label: 'Dr T Doctor' })
  await page.getByLabel('Service centre').selectOption({ label: 'Rooms' })
}

const line = (page: Page, n: number) =>
  page.getByRole('group', { name: `Line ${n}` })

test.describe('creating an invoice', () => {
  test('keyboard line entry, tariff prices, checks and finalising', async ({
    page,
  }) => {
    const logs = captureConsole(page)
    const id = await createFile(
      'Kagiso',
      { date_of_birth: '1990-05-05', sex: 'female' },
      true,
    )
    await newInvoice(page, id)
    await expect(page.getByLabel('Patient')).toHaveValue(/.+/) // only patient chosen
    await header(page)
    await expect(page.getByLabel('Place of service')).toHaveValue('11') // from the centre

    // Line 1: code search with the keyboard.
    const code1 = page.getByRole('combobox', { name: 'Line 1 code' })
    await code1.fill('0190')
    await expect(
      page.getByRole('option', { name: /Consultation, new patient/ }),
    ).toBeVisible()
    await code1.press('Enter')
    await expect(line(page, 1)).toContainText('Consultation, new patient')
    await expect(page.getByLabel('Line 1 unit price in rand')).toHaveValue(
      '520.00',
    )
    await expect(line(page, 1)).toContainText(
      /Price from the tariff: R\s520\.00 \(plan rate/,
    )

    const icd1 = page.getByRole('combobox', { name: 'Line 1 ICD-10 codes' })
    await icd1.fill('J06')
    await expect(page.getByRole('option', { name: /J06\.9/ })).toBeVisible()
    await icd1.press('Enter')
    await expect(line(page, 1)).toContainText(
      'J06.9 Acute upper respiratory infection',
    )

    // Changing the price leaves a visible note, and can be undone.
    await page.getByLabel('Line 1 unit price in rand').fill('480')
    await expect(line(page, 1)).toContainText(
      /Price changed from the tariff rate of R\s520\.00/,
    )
    await page.getByRole('button', { name: 'Use the tariff rate' }).click()
    await expect(page.getByLabel('Line 1 unit price in rand')).toHaveValue(
      '520.00',
    )

    // Enter adds a line and puts you in its code search.
    await page.getByLabel('Line 1 quantity').press('Enter')
    const code2 = page.getByRole('combobox', { name: 'Line 2 code' })
    await expect(code2).toBeFocused()
    await code2.fill('parac')
    await expect(
      page.getByRole('option', { name: /Paracetamol/ }),
    ).toBeVisible()
    await code2.press('ArrowDown')
    await code2.press('Enter')
    await expect(line(page, 2)).toContainText('716541001')
    await page.getByLabel('Line 2 quantity').fill('2')
    await page.getByLabel('Line 2 unit price in rand').fill('12.50')
    await expect(page.getByLabel('Line 2 amount')).toHaveText(/R\s25\.00/)

    // Checks before finalising: line 2 has no ICD-10 code yet.
    await page.getByRole('button', { name: 'Finalise invoice' }).click()
    await expect(line(page, 2)).toContainText('Add at least one ICD-10 code')
    await expect(
      page.getByRole('group', { name: 'Confirm finalising' }),
    ).toHaveCount(0)
    const icd2 = page.getByRole('combobox', { name: 'Line 2 ICD-10 codes' })
    await icd2.fill('J06')
    await expect(page.getByRole('option', { name: /J06\.9/ })).toBeVisible()
    await icd2.press('Enter')
    await expect(line(page, 2)).not.toContainText(
      'Add at least one ICD-10 code',
    )

    await expect(page.getByLabel('Invoice total')).toHaveText(/R\s545\.00/)
    await expect(
      page.getByRole('status').filter({ hasText: 'Draft saved' }),
    ).toBeVisible()

    await page.getByRole('button', { name: 'Finalise invoice' }).click()
    const confirm = page.getByRole('group', { name: 'Confirm finalising' })
    await expect(confirm).toContainText(/can.t be edited afterwards/)
    await confirm.getByRole('button', { name: 'Post invoice' }).click()

    await expect(page).toHaveURL(/\/billing\/invoices\/[0-9a-f-]+$/)
    const inv = page.getByRole('article', { name: /^Invoice \d+/ })
    await expect(inv.getByLabel('Invoice totals')).toContainText(
      /Net\s*R\s545\.00/,
    )
    await expect(inv.getByLabel('Still owed')).toContainText(
      /Medical aid owes\s*R\s545\.00/,
    )
    await expect(
      inv.getByRole('table', { name: 'Invoice lines' }).locator('tbody tr'),
    ).toHaveCount(2)

    expect(logs.join('\n')).not.toMatch(/Kagiso|INV-KAGISO/)
  })

  test('a draft saves itself, survives a reload and can be discarded', async ({
    page,
  }) => {
    const id = await createFile('Draftie')
    await newInvoice(page, id)
    await header(page)
    await page.getByLabel('Authorisation number (optional)').fill('AUTH-1')
    await expect(
      page.getByRole('status').filter({ hasText: 'Draft saved' }),
    ).toBeVisible()
    await page.waitForTimeout(1200) // let the autosave land
    await page.reload()
    await expect(page.getByLabel('Treating doctor')).toHaveValue(/.+/)
    await expect(
      page.getByLabel('Authorisation number (optional)'),
    ).toHaveValue('AUTH-1')

    // Listed on the file until finalised or discarded.
    await page.goto(`/patients/file/${id}?tab=billing`)
    await expect(page.getByRole('link', { name: /Draft saved/ })).toHaveCount(1)
    await page.getByRole('link', { name: /Draft saved/ }).click()
    page.once('dialog', (d) => void d.accept())
    await page.getByRole('button', { name: 'Discard draft' }).click()
    await expect(page).toHaveURL(/\/billing$/)
    await page.goto(`/patients/file/${id}?tab=billing`)
    await expect(page.getByRole('link', { name: /Draft saved/ })).toHaveCount(0)
  })

  test('warns about ICD-10 codes that do not fit the patient', async ({
    page,
  }) => {
    const id = await createFile('Bheki', {
      date_of_birth: '1990-01-01',
      sex: 'male',
    })
    await newInvoice(page, id)
    await header(page)
    const code = page.getByRole('combobox', { name: 'Line 1 code' })
    await code.fill('0191')
    await expect(
      page.getByRole('option', { name: /established/ }),
    ).toBeVisible()
    await code.press('Enter')
    // Private file: the default rate.
    await expect(page.getByLabel('Line 1 unit price in rand')).toHaveValue(
      '390.00',
    )
    await expect(line(page, 1)).toContainText('(default rate')

    const icd = page.getByRole('combobox', { name: 'Line 1 ICD-10 codes' })
    await icd.fill('O80')
    await expect(page.getByRole('option', { name: /O80/ })).toBeVisible()
    await icd.press('Enter')
    await expect(
      page.getByRole('list', { name: 'Line 1 warnings' }),
    ).toContainText('O80 is for female patients only.')

    // A code that can't be primary, put first, is stopped before posting.
    await icd.press('Backspace') // removes O80
    await icd.fill('Y40')
    await expect(page.getByRole('option', { name: /Y40\.0/ })).toContainText(
      'Not primary',
    )
    await icd.press('Enter')
    await page.getByRole('button', { name: 'Finalise invoice' }).click()
    await expect(line(page, 1)).toContainText(
      "Y40.0 can't be the primary (first) code",
    )
  })
})
