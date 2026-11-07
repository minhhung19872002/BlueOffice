import { defineConfig, devices } from '@playwright/test';
import { collabUrls } from './collab/ports.mjs';

// Collaboration E2E against the real BlueOffice server + mock BlueDrive
// (collab/stack.mjs). Needs the built web app (`pnpm --filter @blueoffice/web build`)
// and the .NET 8 SDK. Ports: COLLAB_PORT or E2E_PORT + 20.
const urls = collabUrls();

export default defineConfig({
  testDir: './specs',
  testMatch: /collab-.*\.spec\.ts/,
  timeout: 120_000,
  expect: { timeout: 15_000 },
  // One server room per file; the specs create their own files but share the stack.
  workers: 1,
  retries: process.env.CI ? 1 : 0,
  reporter: [['list'], ['html', { open: 'never', outputFolder: 'playwright-report-collab' }]],
  outputDir: 'test-results-collab',
  use: {
    baseURL: urls.host,
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
    command: 'node collab/stack.mjs',
    url: `${urls.host}/`,
    reuseExistingServer: !process.env.CI,
    timeout: 300_000,
    stdout: 'pipe',
    stderr: 'pipe',
  },
});
