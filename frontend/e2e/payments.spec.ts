// Runs after invoices.spec (alphabetical): it adds invoices to Practice A,
// which that spec counts.
import { expect, test } from '@playwright/test'
import type { Page } from '@playwright/test'

import { apiAs, signInAs } from './helpers'

/** A medical aid file with one posted invoice: R520 consult + R25 medicine. */
async function postedInvoice(
  first: string,
): Promise<{ invoiceId: string; accountId: string }> {
  const { api, headers } = await apiAs('owner@e2e.test', 'Practice A')
  const get = async <T>(path: string) =>
    (await (await api.get(path, { headers })).json()) as T
  const schemes =
    await get<
      Array<{ id: string; name: string; plans: Array<{ id: string }> }>
    >('/medical-schemes')
  const s = schemes.find((x) => x.name === 'E2E Health')!
  const doctor = (
    await get<Array<{ id: string; last_name: string }>>('/providers')
  ).find((p) => p.last_name === 'Doctor')!
  const rooms = (
    await get<Array<{ id: string; name: string }>>('/service-centres')
  ).find((c) => c.name === 'Rooms')!
  const file = await api.post('/patient-files', {
    headers,
    data: {
      patients: [
        { person: { new: { first_name: first, last_name: 'Corrections' } } },
      ],
      guarantor: { patient: 0 },
      medical_aid: {
        new: {
          scheme_id: s.id,
          plan_id: s.plans[0].id,
          membership_number: `COR-${first.toUpperCase()}`,
          main_member: { patient: 0 },
        },
      },
    },
  })
  const f = (await file.json()) as {
    id: string
    patients: Array<{ patient_id: string }>
  }
  const inv = await api.post('/invoices', {
    headers,
    data: {
      account_id: f.id,
      patient_id: f.patients[0].patient_id,
      treating_provider_id: doctor.id,
      service_centre_id: rooms.id,
      lines: [
        {
          tariff_code: '0190',
          unit_price_cents: 52000,
          icd10_codes: ['J06.9'],
        },
        {
          nappi_code: '716541001',
          unit_price_cents: 2500,
          icd10_codes: ['J06.9'],
        },
      ],
    },
  })
  if (!inv.ok()) throw new Error(`invoice: ${inv.status()} ${await inv.text()}`)
  const id = ((await inv.json()) as { id: string }).id
  await api.dispose()
  return { invoiceId: id, accountId: f.id }
}

const owed = (page: Page) => page.getByLabel('Still owed')

test.describe('corrections and payments', () => {
  test('credit note, liability move, payment and journal', async ({ page }) => {
    const { invoiceId } = await postedInvoice('Thato')
    await signInAs(page, 'owner@e2e.test', `/billing/invoices/${invoiceId}`)
    await expect(owed(page)).toContainText(/Medical aid owes\s*R\s545\.00/)

    // Credit note: reverse the medicine line, with a required reason.
    await page.getByRole('button', { name: 'Issue credit note' }).click()
    const cn = page.getByRole('form', { name: 'Issue a credit note' })
    await cn.getByRole('checkbox', { name: /Paracetamol/ }).check()
    await cn.getByRole('button', { name: 'Issue credit note' }).click()
    await expect(
      cn.getByText('Give a reason for the credit note'),
    ).toBeVisible()
    await cn.getByLabel('Reason').fill('Medicine not dispensed')
    await cn.getByRole('button', { name: 'Issue credit note' }).click()
    await expect(
      page.getByRole('status').filter({ hasText: 'Credit note issued.' }),
    ).toBeVisible()
    const lines = page.getByRole('table', { name: 'Invoice lines' })
    await expect(lines.locator('tr', { hasText: 'Paracetamol' })).toContainText(
      'Reversed by credit note',
    )
    await expect(page.getByLabel('Invoice totals')).toContainText(
      /Net\s*R\s520\.00/,
    )
    await expect(
      page.getByRole('region', { name: 'Credit notes' }),
    ).toContainText('Medicine not dispensed')

    // Move a co-payment to the patient; can't move more than is owed.
    await page.getByRole('button', { name: 'Move liability' }).click()
    const mv = page.getByRole('form', { name: 'Move liability' })
    await mv.getByLabel('Amount (R)').fill('600')
    await mv.getByLabel('Reason').fill('Co-payment the scheme won’t pay')
    await mv.getByRole('button', { name: 'Move liability' }).click()
    await expect(mv.getByText(/owes only 520\.00/)).toBeVisible()
    await mv.getByLabel('Amount (R)').fill('90')
    await mv.getByRole('button', { name: 'Move liability' }).click()
    await expect(
      page.getByRole('status').filter({ hasText: 'Liability moved.' }),
    ).toBeVisible()
    await expect(owed(page)).toContainText(/Medical aid owes\s*R\s430\.00/)
    await expect(owed(page)).toContainText(/Patient owes\s*R\s90\.00/)

    // Payment: defaults to what the patient owes, onto this invoice.
    await page.getByRole('button', { name: 'Record payment' }).click()
    const pay = page.getByRole('form', { name: 'Record a payment' })
    await expect(pay.getByLabel('Amount (R)')).toHaveValue('90.00')
    await pay.getByLabel('Method').selectOption('eft')
    await pay.getByLabel('Reference (optional)').fill('EFT-COR')
    await pay.getByRole('button', { name: 'Record payment' }).click()
    const receipt = page.getByRole('status', { name: 'Payment recorded' })
    await expect(receipt).toContainText(/Receipt \d+ · R\s90\.00/)
    await expect(receipt).toContainText('EFT · EFT-COR')
    await expect(
      receipt.getByRole('list', { name: 'Allocated to' }),
    ).toContainText(/Invoice \d+\s*R\s90\.00/)
    await expect(owed(page)).toContainText(/Patient owes\s*R\s0\.00/)

    // Journal with a reason type from the practice's list.
    await page.getByRole('button', { name: 'Post journal' }).click()
    const j = page.getByRole('form', { name: 'Post a journal' })
    await j
      .getByLabel('Reason', { exact: true })
      .selectOption({ label: 'Small balance write-off' })
    await j.getByLabel('Whose balance').selectOption('medical_aid')
    await j.getByLabel('Amount (R)').fill('30')
    await j.getByLabel('Note').fill('Scheme short-paid; not worth chasing')
    await j.getByRole('button', { name: 'Post journal' }).click()
    await expect(
      page.getByRole('status').filter({ hasText: 'Journal posted.' }),
    ).toBeVisible()
    await expect(owed(page)).toContainText(/Medical aid owes\s*R\s400\.00/)

    const txns = page.getByRole('table', { name: /Transactions on invoice/ })
    await expect(txns).toContainText('Credit note')
    await expect(txns).toContainText('Liability transfer')
    await expect(txns).toContainText(
      /Payment received \(EFT\) EFT-COR · receipt \d+/,
    )
    await expect(txns).toContainText('Journal: Small balance write-off')
    // The staff note is never printed.
    await expect(page.getByText('not worth chasing')).toHaveCount(0)
  })

  test('a payment from the file shows where it went', async ({ page }) => {
    const { accountId } = await postedInvoice('Lebo')
    await signInAs(
      page,
      'owner@e2e.test',
      `/patients/file/${accountId}?tab=billing`,
    )
    await page.getByRole('button', { name: 'Record payment' }).click()
    const pay = page.getByRole('form', { name: 'Record a payment' })
    await pay.getByLabel('Amount (R)').fill('50')
    await pay.getByRole('button', { name: 'Record payment' }).click()
    // Nothing is owed by the patient yet (the medical aid owes it all), so it
    // stays on the account as credit.
    const receipt = page.getByRole('status', { name: 'Payment recorded' })
    await expect(receipt).toContainText(/Receipt \d+ · R\s50\.00/)
    await expect(receipt).toContainText(
      /Kept on the account as credit\s*R\s50\.00/,
    )
  })

  test('staff who can only view billing get no actions', async ({ page }) => {
    const { invoiceId, accountId } = await postedInvoice('Viewonly')
    await signInAs(page, 'viewer@e2e.test', `/billing/invoices/${invoiceId}`)
    await expect(
      page.getByRole('table', { name: 'Invoice lines' }),
    ).toBeVisible()
    for (const name of [
      'Record payment',
      'Issue credit note',
      'Move liability',
      'Post journal',
    ]) {
      await expect(page.getByRole('button', { name })).toHaveCount(0)
    }
    await page.goto(`/patients/file/${accountId}?tab=billing`)
    await expect(
      page.getByRole('link', { name: 'Open statement' }),
    ).toBeVisible()
    await expect(page.getByRole('button', { name: 'New invoice' })).toHaveCount(
      0,
    )
    await expect(
      page.getByRole('button', { name: 'Record payment' }),
    ).toHaveCount(0)
  })
})
