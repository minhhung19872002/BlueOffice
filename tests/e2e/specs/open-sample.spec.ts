import { expect, test } from '@playwright/test';
import { mkdirSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const outDir = join(dirname(fileURLToPath(import.meta.url)), '..', '..', 'fidelity', 'output');

test('opens the sample .docx and renders it with the WASM engine', async ({ page }) => {
  const errors: string[] = [];
  page.on('pageerror', (e) => errors.push(e.message));
  await page.goto('/doc');
  await expect(page.getByRole('tab', { name: 'Trang đầu' })).toBeVisible();
  await expect(page.getByTestId('status-engine')).toHaveText('Lõi: WASM');

  await page.getByTestId('open-sample').click();
  await expect(page.getByTestId('status-page')).toHaveText(/^Trang 1\/\d+$/);
  await expect(page.getByTestId('doc-name')).toHaveText('phase1-sample.docx');
  const firstPage = page.locator('.bo-page[data-page="1"]');
  await expect(firstPage).toHaveAttribute('data-rendered', 'true');

  // The canvas contains ink (text was drawn), not just a white page.
  const inkRatio = await firstPage.locator('canvas').evaluate((c: HTMLCanvasElement) => {
    const ctx = c.getContext('2d')!;
    const { data } = ctx.getImageData(0, 0, c.width, c.height);
    let dark = 0;
    for (let i = 0; i < data.length; i += 4) if (data[i]! < 128) dark++;
    return dark / (c.width * c.height);
  });
  expect(inkRatio).toBeGreaterThan(0.01);
  expect(inkRatio).toBeLessThan(0.3);

  mkdirSync(outDir, { recursive: true });
  await firstPage.screenshot({ path: join(outDir, 'web-page-1.png') });
  expect(errors).toEqual([]);
});

test('zoom changes the page size and the status bar', async ({ page }) => {
  await page.goto('/doc');
  await page.getByTestId('open-sample').click();
  const firstPage = page.locator('.bo-page[data-page="1"]');
  await expect(firstPage).toHaveAttribute('data-rendered', 'true');
  const w1 = (await firstPage.boundingBox())!.width;
  await page.getByRole('button', { name: 'Phóng to' }).last().click();
  await expect(page.getByTestId('status-zoom')).toHaveText('110%');
  // The status bar updates with the zoom state; the page box is resized on the
  // next layout pass, so wait for it instead of measuring once.
  await expect
    .poll(async () => ((await firstPage.boundingBox())?.width ?? 0) / w1)
    .toBeCloseTo(1.1, 1);
});

test('switches to English and dark theme', async ({ page }) => {
  await page.goto('/doc');
  await page.getByRole('tab', { name: 'Xem', exact: true }).click();
  await page.getByRole('button', { name: 'English' }).click();
  await expect(page.getByRole('tab', { name: 'View', exact: true })).toBeVisible();
  await page.getByRole('button', { name: 'Light / Dark / High contrast' }).click();
  await expect(page.locator('.bo-root')).toHaveAttribute('data-theme', 'dark');
  await page.getByRole('button', { name: 'Light / Dark / High contrast' }).click();
  const root = page.locator('.bo-root');
  await expect(root).toHaveAttribute('data-theme', 'hc');
  // High contrast: white text on black, BlueDoc accent replaced by the high-contrast yellow.
  const vars = await root.evaluate((el) => {
    const cs = getComputedStyle(el);
    // The minified build may lower-case and shorten hex colours (#ffffff → #fff).
    const hex = (v: string) =>
      v
        .trim()
        .toLowerCase()
        .replace(/^#(\w)(\w)(\w)$/, '#$1$1$2$2$3$3');
    return [hex(cs.getPropertyValue('--bo-text')), hex(cs.getPropertyValue('--bo-app-accent'))];
  });
  expect(vars).toEqual(['#ffffff', '#ffd600']);
});

test('exports a PDF', async ({ page }) => {
  await page.goto('/doc');
  await page.getByTestId('open-sample').click();
  await expect(page.locator('.bo-page[data-page="1"]')).toHaveAttribute('data-rendered', 'true');
  // File screen (CORE-UI-002) › Export.
  await page.getByTestId('file-tab').click();
  await page
    .getByRole('navigation', { name: 'Tệp' })
    .getByRole('button', { name: 'Xuất', exact: true })
    .click();
  const download = page.waitForEvent('download');
  await page.locator('[data-testid="backstage"] [data-command="file.exportPdf"]').click();
  const d = await download;
  expect(d.suggestedFilename()).toBe('phase1-sample.pdf');
});

test('legacy routes redirect to the new app routes', async ({ page }) => {
  for (const [from, to] of [
    ['/write', '/doc'],
    ['/docs', '/doc'],
    ['/sheets', '/sheet'],
    ['/slides', '/slide'],
  ] as const) {
    await page.goto(from);
    await expect(page).toHaveURL(new RegExp(`${to}$`));
  }
  await expect(page.locator('.bo-root')).toHaveAttribute('data-app', 'slide');
});
