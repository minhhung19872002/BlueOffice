import { defineConfig, devices } from '@playwright/test';

// Port can be overridden (several checkouts may run previews at once).
const port = Number(process.env.E2E_PORT ?? 4173);

// Web target. The desktop target (Tauri WebDriver) is added with the Phase 2 editing flows.
export default defineConfig({
  testDir: './specs',
  timeout: 60_000,
  retries: process.env.CI ? 1 : 0,
  reporter: [['list'], ['html', { open: 'never' }]],
  use: {
    baseURL: `http://localhost:${port}`,
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
    command: `pnpm --filter @blueoffice/web preview --port ${port} --strictPort`,
    url: `http://localhost:${port}/doc`,
    reuseExistingServer: !process.env.CI,
    timeout: 60_000,
  },
});
