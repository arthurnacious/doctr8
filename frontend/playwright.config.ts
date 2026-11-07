import { defineConfig, devices } from '@playwright/test'

// Driven by e2e/run.sh, which starts the API and the dev server first.
export default defineConfig({
  testDir: './e2e',
  fullyParallel: false,
  workers: 1,
  retries: 0,
  reporter: [['list']],
  use: {
    baseURL: process.env.E2E_BASE_URL ?? 'http://localhost:5174',
    trace: 'retain-on-failure',
    ...devices['Desktop Chrome'],
    launchOptions: process.env.CHROMIUM_PATH
      ? { executablePath: process.env.CHROMIUM_PATH }
      : {},
  },
})
