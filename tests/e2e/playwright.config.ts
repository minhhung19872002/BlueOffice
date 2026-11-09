import { defineConfig, devices } from '@playwright/test';

// Port can be overridden (several checkouts may run previews at once).
const port = Number(process.env.E2E_PORT ?? 4173);

/** Specs that read or write the system clipboard (see the `clipboard` project). */
const CLIPBOARD_SPECS = /(paste|slide-rich)\.spec\.ts/;

// Web target. The desktop target (Tauri WebDriver) is added with the Phase 2 editing flows.
export default defineConfig({
  testDir: './specs',
  // Real-server collaboration specs run with playwright.collab.config.ts.
  testIgnore: /collab-.*\.spec\.ts/,
  timeout: 60_000,
  // Engine round trips run in a worker; 5 s (default) is short on a loaded machine.
  expect: { timeout: 10_000 },
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
      testIgnore: [/collab-.*\.spec\.ts/, CLIPBOARD_SPECS],
      use: { ...devices['Desktop Chrome'], viewport: { width: 1366, height: 900 } },
    },
    {
      // The system clipboard is shared by every browser: specs that copy and paste run one
      // at a time, after the others.
      name: 'clipboard',
      testMatch: CLIPBOARD_SPECS,
      workers: 1,
      dependencies: ['chromium'],
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
