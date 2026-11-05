import { defineConfig, devices } from '@playwright/test';

// Web target. The desktop target (Tauri WebDriver) is added with the Phase 2 editing flows.
export default defineConfig({
  testDir: './specs',
  timeout: 60_000,
  retries: process.env.CI ? 1 : 0,
  reporter: [['list'], ['html', { open: 'never' }]],
  use: {
    baseURL: 'http://localhost:4173',
    viewport: { width: 1366, height: 900 },
    trace: 'retain-on-failure',
  },
  projects: [
    {
      name: 'chromium',
      use: { ...devices['Desktop Chrome'], viewport: { width: 1366, height: 900 } },
    },
  ],
  webServer: {
    command: 'pnpm --filter @blueoffice/web preview',
    url: 'http://localhost:4173/doc',
    reuseExistingServer: !process.env.CI,
    timeout: 60_000,
  },
});
