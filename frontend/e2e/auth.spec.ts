import { expect, test } from '@playwright/test'
import type { Page } from '@playwright/test'

const PASSWORD = process.env.E2E_PASSWORD ?? 'correct horse battery'
const NAV = 'Main'

async function signIn(page: Page, email: string) {
  await page.getByLabel('Email').fill(email)
  await page.getByLabel('Password', { exact: true }).fill(PASSWORD)
  await page.getByRole('button', { name: 'Sign in' }).click()
}

async function twoStep(page: Page, code = '000000', rememberDevice = false) {
  await expect(page).toHaveURL(/\/two-step/)
  await page.getByLabel('6-digit code').fill(code)
  if (rememberDevice) {
    await page.getByLabel("Don't ask again on this device for 30 days").check()
  }
  await page.getByRole('button', { name: 'Verify' }).click()
}

function navLinks(page: Page) {
  return page.getByRole('navigation', { name: NAV }).getByRole('link')
}

test('signed-out users are sent to sign in and never see app pages', async ({
  page,
}) => {
  await page.goto('/billing')
  await expect(page).toHaveURL(/\/sign-in\?redirect=%2Fbilling/)
  await expect(page.getByRole('heading', { name: 'Sign in' })).toBeVisible()
  await expect(page.getByRole('navigation', { name: NAV })).toHaveCount(0)
})

test('wrong password shows an error', async ({ page }) => {
  await page.goto('/sign-in')
  await page.getByLabel('Email').fill('owner@e2e.test')
  await page.getByLabel('Password', { exact: true }).fill('not the password')
  await page.getByRole('button', { name: 'Sign in' }).click()
  await expect(page.getByRole('alert')).toContainText('incorrect')
})

test('show password toggles the field', async ({ page }) => {
  await page.goto('/sign-in')
  const field = page.getByLabel('Password', { exact: true })
  await expect(field).toHaveAttribute('type', 'password')
  await page.getByRole('button', { name: 'Show password' }).click()
  await expect(field).toHaveAttribute('type', 'text')
  await expect(
    page.getByRole('button', { name: 'Hide password' }),
  ).toHaveAttribute('aria-pressed', 'true')
})

test('owner: two-step, choose practice, land where they were going, switch, reload, sign out', async ({
  page,
}) => {
  const sockets: Array<{ url: string; frames: Array<string> }> = []
  page.on('websocket', (ws) => {
    if (!ws.url().endsWith('/api/ws')) return // ignore Vite's live-reload socket
    const entry = { url: ws.url(), frames: [] as Array<string> }
    sockets.push(entry)
    ws.on('framereceived', (f) => entry.frames.push(String(f.payload)))
  })

  await page.goto('/billing')
  await signIn(page, 'owner@e2e.test')

  // Two-step (development stub): wrong code first.
  await expect(page.getByText('Development stub.')).toBeVisible()
  await twoStep(page, '123456')
  await expect(page.getByRole('alert')).toContainText("isn't right")
  await page.getByLabel('6-digit code').fill('000000')
  await page.getByRole('button', { name: 'Verify' }).click()

  // Two practices, none chosen yet.
  await expect(
    page.getByRole('heading', { name: 'Choose a practice' }),
  ).toBeVisible()
  await page.getByRole('button', { name: /Practice A/ }).click()

  // Back where they were going, with every section available to an owner.
  await expect(page).toHaveURL(/\/billing$/)
  await expect(page.getByRole('heading', { name: 'Billing' })).toBeVisible()
  await expect(navLinks(page)).toHaveText([
    'Dashboard',
    'Diary',
    'Patients',
    'Billing',
    'Claims',
    'Finance',
    'Settings',
  ])

  // One realtime connection, authenticated by its first message (the token
  // is never in the URL).
  await expect.poll(() => sockets.length).toBe(1)
  await expect.poll(() => sockets[0].frames.join()).toContain('"type":"ready"')

  // Switch practice from the sidebar: new token, new socket, back to the dashboard.
  await page.getByRole('button', { name: /Practice A/ }).click()
  await page.getByRole('button', { name: 'Practice B' }).click()
  await expect(page.getByText("You're signed in to Practice B.")).toBeVisible()
  await expect.poll(() => sockets.length).toBe(2)
  await expect.poll(() => sockets[1].frames.join()).toContain('"type":"ready"')

  // A reload loses the in-memory token; the refresh cookie restores it.
  await page.reload()
  await expect(page.getByText("You're signed in to Practice B.")).toBeVisible()
  const stored = await page.evaluate(() =>
    JSON.stringify({ ...localStorage, ...sessionStorage }),
  )
  expect(stored).not.toMatch(/eyJ[A-Za-z0-9_-]+\./) // no JWT in web storage

  // Sign out: back to sign in, and app pages are guarded again.
  await page.getByRole('button', { name: 'Sign out' }).click()
  await expect(page).toHaveURL(/\/sign-in/)
  await page.goto('/')
  await expect(page).toHaveURL(/\/sign-in/)
})

test('reception: one practice skips the chooser; navigation follows permissions; remembered device', async ({
  page,
}) => {
  await page.goto('/sign-in')
  await signIn(page, 'reception@e2e.test')
  await twoStep(page, '000000', true)

  await expect(
    page.getByRole('heading', { name: 'Welcome, Front' }),
  ).toBeVisible()
  await expect(navLinks(page)).toHaveText(['Dashboard', 'Diary', 'Patients'])

  // Typing a forbidden section's URL doesn't get them in.
  await page.goto('/billing')
  await expect(page).toHaveURL(/\/$/)

  // "Don't ask again on this device": the next sign-in skips the code.
  await page.getByRole('button', { name: 'Sign out' }).click()
  await signIn(page, 'reception@e2e.test')
  await expect(
    page.getByRole('heading', { name: 'Welcome, Front' }),
  ).toBeVisible()
})

test('a user with no practice is told so', async ({ page }) => {
  await page.goto('/sign-in')
  await signIn(page, 'nobody@e2e.test')
  await twoStep(page)
  await expect(
    page.getByRole('heading', { name: 'No practice yet' }),
  ).toBeVisible()
  await page.getByRole('button', { name: 'Sign out' }).click()
  await expect(page).toHaveURL(/\/sign-in/)
})

test('signing out in one tab signs out the others', async ({ context }) => {
  const first = await context.newPage()
  await first.goto('/sign-in')
  await signIn(first, 'reception@e2e.test')
  await twoStep(first)
  await expect(
    first.getByRole('heading', { name: 'Welcome, Front' }),
  ).toBeVisible()

  const second = await context.newPage()
  await second.goto('/')
  await expect(
    second.getByRole('heading', { name: 'Welcome, Front' }),
  ).toBeVisible()

  await first.getByRole('button', { name: 'Sign out' }).click()
  await expect(second).toHaveURL(/\/sign-in/)
})

test('small screens: the sidebar becomes a menu', async ({ page }) => {
  await page.setViewportSize({ width: 390, height: 844 })
  await page.goto('/sign-in')
  await signIn(page, 'reception@e2e.test')
  await twoStep(page)
  await expect(
    page.getByRole('heading', { name: 'Welcome, Front' }),
  ).toBeVisible()

  await expect(page.getByRole('navigation', { name: NAV })).toBeHidden()
  await page.getByRole('button', { name: 'Open menu' }).click()
  const dialog = page.getByRole('dialog', { name: 'Navigation' })
  await expect(dialog).toBeVisible()
  await expect(dialog.getByRole('link', { name: 'Patients' })).toBeVisible()
  await page.keyboard.press('Escape')
  await expect(dialog).toBeHidden()

  await page.getByRole('button', { name: 'Open menu' }).click()
  await dialog.getByRole('link', { name: 'Patients' }).click()
  await expect(page).toHaveURL(/\/patients$/)
  await expect(dialog).toBeHidden()
})

test('theme: dark by default, light remembered', async ({ page }) => {
  await page.goto('/sign-in')
  await expect(page.locator('html')).not.toHaveAttribute('data-theme', 'light')
  await page.getByRole('button', { name: 'Switch to light mode' }).click()
  await expect(page.locator('html')).toHaveAttribute('data-theme', 'light')
  await page.reload()
  await expect(page.locator('html')).toHaveAttribute('data-theme', 'light')
})

test('touch targets are at least 44px on the sign-in screen', async ({
  page,
}) => {
  await page.goto('/sign-in')
  for (const el of await page
    .locator('button, input:not([type=checkbox])')
    .all()) {
    const box = await el.boundingBox()
    expect(box && box.height).toBeGreaterThanOrEqual(44)
  }
})
