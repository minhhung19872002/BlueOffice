import { expect, test, type Page } from '@playwright/test';
import { mkdirSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const outDir = join(dirname(fileURLToPath(import.meta.url)), '..', '..', 'fidelity', 'output');

async function goTo(page: Page, ref: string) {
  const box = page.getByLabel('Hộp tên');
  await box.click();
  await box.fill(ref);
  await box.press('Enter');
  // Keyboard focus returns to the (hidden) cell editor.
  await expect(page.getByLabel('Ô đang nhập')).toBeFocused();
}

async function formulaOf(page: Page, ref: string) {
  await goTo(page, ref);
  return page.getByLabel('Thanh công thức');
}

test('BlueSheet: numbers + SUM, bold + fill, sort, save .xlsx and reopen', async ({ page }) => {
  const errors: string[] = [];
  page.on('pageerror', (e) => errors.push(e.message));
  await page.goto('/sheet');
  await expect(page.getByRole('tab', { name: 'Sheet1' })).toBeVisible();
  await expect(page.getByTestId('sheet-canvas')).toHaveAttribute('data-rendered', /\d+/);

  // Type a column of numbers and a SUM formula (the cell editor has focus).
  for (const v of ['30', '10', '20', '=SUM(A1:A3)']) {
    await page.keyboard.type(v);
    await page.keyboard.press('Enter');
  }
  await expect(await formulaOf(page, 'A4')).toHaveValue('=SUM(A1:A3)');
  await goTo(page, 'A1:A4');
  await expect(page.getByTestId('status-sum')).toHaveText('Tổng: 120');

  // Bold (Ctrl+B) and a fill colour from the ribbon palette.
  await goTo(page, 'A1');
  await page.keyboard.press('Control+b');
  const bold = page.locator('[data-command="format.bold"]');
  await expect(bold).toHaveAttribute('aria-pressed', 'true');
  await page.locator('[data-command="format.fillColor"]').click();
  await page.getByTestId('color-menu').locator('[data-color="FFEE00"]').click();

  // Sort A1:A3 ascending from the Data tab.
  await goTo(page, 'A1:A3');
  await page.getByRole('tab', { name: 'Dữ liệu' }).click();
  await page.locator('[data-command="data.sortAsc"]').click();
  await expect(await formulaOf(page, 'A1')).toHaveValue('10');
  await expect(await formulaOf(page, 'A3')).toHaveValue('30');
  // The style moved with its value (30 was bold + filled).
  await page.getByRole('tab', { name: 'Trang đầu' }).click();
  await expect(bold).toHaveAttribute('aria-pressed', 'true');
  await goTo(page, 'A1');
  await expect(bold).toHaveAttribute('aria-pressed', 'false');
  mkdirSync(outDir, { recursive: true });
  await page.screenshot({ path: join(outDir, 'web-sheet-editing.png') });

  // Save (Ctrl+S) downloads the workbook.
  await page.getByLabel('Ô đang nhập').focus();
  const download = page.waitForEvent('download');
  await page.keyboard.press('Control+s');
  const d = await download;
  expect(d.suggestedFilename()).toMatch(/\.xlsx$/);
  const path = await d.path();

  // Reopen it through the file picker.
  await page.reload();
  await expect(page.getByRole('tab', { name: 'Sheet1' })).toBeVisible();
  const chooser = page.waitForEvent('filechooser');
  await page.locator('[data-command="file.open"]').click();
  await (await chooser).setFiles(path);
  await expect(await formulaOf(page, 'A4')).toHaveValue('=SUM(A1:A3)');
  await expect(await formulaOf(page, 'A2')).toHaveValue('20');
  await goTo(page, 'A3');
  await expect(bold).toHaveAttribute('aria-pressed', 'true');
  await goTo(page, 'A1:A4');
  await expect(page.getByTestId('status-sum')).toHaveText('Tổng: 120');
  expect(errors).toEqual([]);
});
