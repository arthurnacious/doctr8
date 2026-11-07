import { expect, test } from '@playwright/test'
import type { Page } from '@playwright/test'

import { apiAs, captureConsole, setFileMode, signInAs } from './helpers'

/** Open a file through the API; returns its account id. */
async function createFile(body: object): Promise<string> {
  const { api, headers } = await apiAs('owner@e2e.test', 'Practice A')
  const r = await api.post('/patient-files', { headers, data: body })
  if (!r.ok()) throw new Error(`create file: ${r.status()} ${await r.text()}`)
  const id = ((await r.json()) as { id: string }).id
  await api.dispose()
  return id
}

const privateFile = (first: string, last: string) => ({
  patients: [{ person: { new: { first_name: first, last_name: last } } }],
  guarantor: { patient: 0 },
})

async function openFile(
  page: Page,
  accountId: string,
  email = 'owner@e2e.test',
) {
  await signInAs(page, email, `/patients/file/${accountId}`)
  await expect(page.getByRole('heading', { name: /^File \d+/ })).toBeVisible()
}

test.describe('editing a file', () => {
  test('edit a person: validation, save, plain-language errors', async ({
    page,
  }) => {
    const logs = captureConsole(page)
    const id = await createFile(privateFile('Edna', 'Editme'))
    await openFile(page, id)

    await page.getByRole('button', { name: 'Edit details' }).click()
    const form = page.getByRole('form', {
      name: 'Edit details for Edna Editme',
    })
    await expect(form.getByLabel('Title')).toBeFocused()

    await form.getByLabel('First name').fill('')
    await form.getByLabel('Mobile').fill('082 000 1111')
    await form.getByRole('button', { name: 'Save details' }).click()
    await expect(form.getByText('Enter a first name')).toBeVisible()

    await form.getByLabel('First name').fill('Edwina')
    await form.getByRole('button', { name: 'Save details' }).click()
    await expect(
      page.getByRole('status').getByText('Details saved.'),
    ).toBeVisible()
    await expect(
      page.getByRole('button', { name: 'Edit details' }),
    ).toBeFocused()
    await expect(
      page.getByRole('heading', { name: /Edwina Editme/ }),
    ).toBeVisible()
    await expect(page.getByText('0820001111')).toBeVisible()

    // Someone else's ID number: the backend refuses, in plain language.
    await page.getByRole('button', { name: 'Edit details' }).click()
    const again = page.getByRole('form', { name: /Edit details for/ })
    await again.getByLabel('South African ID').check()
    await again.getByLabel('SA ID number').fill('8001015009087')
    await again.getByRole('button', { name: 'Save details' }).click()
    await expect(again.getByRole('alert')).toContainText(
      'belongs to someone else',
    )
    await again.getByRole('button', { name: 'Cancel' }).click()
    await expect(page.getByText('8001015009087')).toHaveCount(0)

    expect(logs.join('\n')).not.toMatch(
      /Edwina|Editme|0820001111|8001015009087/,
    )
  })

  test('add, edit and remove a medical aid', async ({ page }) => {
    const id = await createFile(privateFile('Mandla', 'Aidtest'))
    await openFile(page, id)
    await page.getByRole('tab', { name: 'Medical aid' }).click()
    await expect(page.getByText('This file is private')).toBeVisible()

    await page.getByRole('button', { name: 'Add medical aid' }).click()
    const add = page.getByRole('form', { name: 'Add medical aid' })
    await add.getByRole('button', { name: 'Save medical aid' }).click()
    await expect(add.getByText('Choose the medical scheme')).toBeVisible()
    await add.getByLabel('Medical scheme').selectOption({ label: 'E2E Health' })
    await add.getByLabel('Plan').selectOption({ label: 'Core' })
    await add.getByLabel('Membership number').fill('ma 1')
    await add.getByRole('button', { name: 'Save medical aid' }).click()
    await expect(page.getByText('Medical aid changed.')).toBeVisible()
    await expect(page.getByRole('tabpanel').getByText('MA1')).toBeVisible()
    await expect(
      page.getByRole('tabpanel').getByText('Mandla Aidtest'),
    ).toBeVisible()

    await page.getByRole('button', { name: 'Edit plan or number' }).click()
    const edit = page.getByRole('form', { name: 'Edit medical aid' })
    await expect(edit.getByLabel('Membership number')).toHaveValue('MA1')
    await edit.getByLabel('Membership number').fill('MA-2')
    await edit.getByRole('button', { name: 'Save medical aid' }).click()
    await expect(page.getByText('Medical aid saved.')).toBeVisible()
    await expect(page.getByRole('tabpanel').getByText('MA-2')).toBeVisible()

    await page.getByRole('button', { name: 'Change medical aid' }).click()
    const change = page.getByRole('form', { name: 'Change medical aid' })
    await change.getByLabel('Private').check()
    await change.getByRole('button', { name: 'Make private' }).click()
    await expect(page.getByText('This file is private')).toBeVisible()
  })

  test('file-mode errors from the backend read plainly', async ({ page }) => {
    // A child is the main member (00) but a parent pays: making the file
    // private would make the parent the principal, which 00 doesn't follow.
    const { api, headers } = await apiAs('owner@e2e.test', 'Practice A')
    const schemes = (await (
      await api.get('/medical-schemes', { headers })
    ).json()) as Array<{
      id: string
      name: string
      plans: Array<{ id: string }>
    }>
    await api.dispose()
    const s = schemes.find((x) => x.name === 'E2E Health')!
    const id = await createFile({
      patients: [
        { person: { new: { first_name: 'Kidmain', last_name: 'Rules' } } },
      ],
      guarantor: { new: { first_name: 'Parent', last_name: 'Rules' } },
      medical_aid: {
        new: {
          scheme_id: s.id,
          plan_id: s.plans[0].id,
          membership_number: 'RULES-1',
          main_member: { patient: 0 },
        },
      },
    })
    await openFile(page, id)
    await page.getByRole('tab', { name: 'Medical aid' }).click()
    await page.getByRole('button', { name: 'Change medical aid' }).click()
    const change = page.getByRole('form', { name: 'Change medical aid' })
    await change.getByLabel('Private').check()
    await change.getByRole('button', { name: 'Make private' }).click()
    await expect(change.getByRole('alert')).toContainText(
      'Code 00 always belongs to the medical aid main member',
    )
  })

  test('add a dependant to a family file', async ({ page }) => {
    await setFileMode('owner@e2e.test', 'Practice A', 'family_per_file')
    let id: string
    try {
      id = await createFile(privateFile('Thandeka', 'Famtest'))
    } finally {
      await setFileMode('owner@e2e.test', 'Practice A', 'patient_per_file')
    }
    await openFile(page, id)

    await page.getByRole('button', { name: 'Add dependant' }).click()
    const form = page.getByRole('form', { name: 'Add a dependant' })
    await expect(form.getByLabel('Surname')).toHaveValue('Famtest')
    await expect(form.getByLabel('Dependant code')).toHaveValue('01')
    await form.getByLabel('First name').fill('Lwazi')
    await form.getByLabel('No document').check()
    await form.getByLabel('Dependant code').fill('00')
    await form.getByRole('button', { name: 'Add dependant' }).click()
    await expect(form.getByText(/00 is the main member/)).toBeVisible()

    await form.getByLabel('Dependant code').fill('01')
    await form.getByRole('button', { name: 'Add dependant' }).click()
    await expect(page.getByText('Dependant added.')).toBeVisible()
    const list = page.getByRole('list', { name: 'Patients on this file' })
    await expect(list.getByRole('button')).toHaveCount(2)
    await expect(
      page.getByRole('heading', { name: /Lwazi Famtest · dependant 01/ }),
    ).toBeVisible()
  })

  test('single-patient files offer no dependants', async ({ page }) => {
    const id = await createFile(privateFile('Solo', 'Single'))
    await openFile(page, id)
    await expect(
      page.getByRole('button', { name: 'Add dependant' }),
    ).toHaveCount(0)
  })

  test('mark a patient inactive and active again', async ({ page }) => {
    const id = await createFile(privateFile('Ina', 'Toggle'))
    await openFile(page, id)
    await page.getByRole('button', { name: 'Mark inactive' }).click()
    const confirm = page.getByRole('group', {
      name: 'Confirm marking inactive',
    })
    await expect(confirm).toContainText('Mark Ina Toggle inactive?')
    await confirm.getByRole('button', { name: 'Mark inactive' }).click()
    const list = page.getByRole('list', { name: 'Patients on this file' })
    await expect(list.getByText('Inactive')).toBeVisible()
    await page.getByRole('button', { name: 'Mark active' }).click()
    await expect(list.getByText('Inactive')).toHaveCount(0)
    await expect(
      page.getByRole('button', { name: 'Mark inactive' }),
    ).toBeVisible()
  })

  test('staff without manage permissions see no editing', async ({ page }) => {
    const id = await createFile(privateFile('View', 'Only'))
    await openFile(page, id, 'reception@e2e.test')
    await expect(page.getByText('View Only').first()).toBeVisible()
    for (const name of ['Edit details', 'Mark inactive', 'Add dependant']) {
      await expect(page.getByRole('button', { name })).toHaveCount(0)
    }
    await page.getByRole('tab', { name: 'Medical aid' }).click()
    await expect(
      page.getByRole('button', { name: 'Add medical aid' }),
    ).toHaveCount(0)
  })
})
