// PWA (prompt §3.4 `offline`: "web = IndexedDB + Service Worker"): once the
// service worker has cached the app shell, the WASM engine, the fonts and the
// sample, BlueDoc starts and opens a document without a network.
import { expect, test } from '@playwright/test';

test('the web app starts and opens a document offline after the first visit', async ({
  page,
  context,
}) => {
  const errors: string[] = [];
  page.on('pageerror', (e) => errors.push(e.message));
  await page.goto('/doc');
  await expect(page.getByTestId('status-engine')).toHaveText('Lõi: WASM');
  // Installed (the whole precache list is cached) and activated.
  const cached = await page.evaluate(async () => {
    const reg = await navigator.serviceWorker.ready;
    const keys = await caches.keys();
    const shell = keys.find((k) => k.startsWith('bo-shell-'));
    const n = shell ? (await (await caches.open(shell)).keys()).length : 0;
    return { active: !!reg.active, shell, n };
  });
  expect(cached.active).toBe(true);
  expect(cached.shell).toBeTruthy();
  expect(cached.n).toBeGreaterThan(10);

  await context.setOffline(true);
  await page.reload();
  await expect(page.getByRole('tab', { name: 'Trang đầu' })).toBeVisible();
  await expect(page.getByTestId('status-engine')).toHaveText('Lõi: WASM');
  await page.getByTestId('open-sample').click();
  await expect(page.getByTestId('doc-name')).toHaveText('phase1-sample.docx');
  await expect(page.locator('.bo-page[data-page="1"]')).toHaveAttribute('data-rendered', 'true');
  // Other routes are the same single page.
  await page.goto('/sheet');
  await expect(page.getByTestId('sheet-canvas')).toHaveAttribute('data-rendered', /\d+/);
  await context.setOffline(false);
  expect(errors).toEqual([]);
});
