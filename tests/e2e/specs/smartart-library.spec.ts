import { expect, test, type Page } from '@playwright/test';
import { copyFileSync, mkdirSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

// CORE-SMART-003 / 004: the layout library by category (the picture category included) and the
// node buttons of the text pane (add, remove, promote, demote, move); kept after save and reopen.
const here = dirname(fileURLToPath(import.meta.url));
const deck = join(here, '..', '..', 'corpus', 'slide', 'slide-text-bullets.pptx');
const outDir = join(here, '..', '..', 'fidelity', 'output');

async function openDeck(page: Page, path: string) {
  const chooser = page.waitForEvent('filechooser');
  await page.getByRole('button', { name: 'Mở', exact: true }).click();
  await (await chooser).setFiles(path);
  await expect(page.getByTestId('slide-status')).toHaveText(/^Slide 1 \/ \d+$/);
  await expect(page.getByTestId('slide-overlay')).toBeVisible();
}

test('library layouts and node buttons of the text pane', async ({ page }) => {
  const errors: string[] = [];
  page.on('pageerror', (e) => errors.push(e.message));
  await page.goto('/slide');
  await openDeck(page, deck);
  await page.getByRole('button', { name: '+ Thêm slide' }).click();
  await expect(page.getByTestId('slide-status')).toHaveText(/^Slide 2 \/ \d+$/);

  // The library: eight categories, each with its layouts; insert a chevron process.
  await page.getByRole('tab', { name: 'Chèn' }).click();
  await page.getByRole('button', { name: 'Sơ đồ thông minh' }).click();
  const dialog = page.getByTestId('smartart-dialog');
  for (const c of [
    'list',
    'process',
    'cycle',
    'hierarchy',
    'matrix',
    'pyramid',
    'relationship',
    'picture',
  ])
    await expect(dialog.getByTestId(`smartart-category-${c}`)).toBeVisible();
  await expect(dialog.getByTestId('smartart-category-picture').getByRole('radio')).toHaveCount(2);
  await dialog.getByTestId('smartart-layout-processChevron').check();
  const pane = dialog.getByTestId('smartart-text');
  await pane.fill('Hà Nội\nHuế\nĐà Nẵng');

  // Node buttons act on the node at the caret: the caret is at the end ("Đà Nẵng").
  await dialog.getByTestId('smartart-node-up').click();
  await expect(pane).toHaveValue('Hà Nội\nĐà Nẵng\nHuế');
  await dialog.getByTestId('smartart-node-demote').click();
  await expect(pane).toHaveValue('Hà Nội\n\tĐà Nẵng\nHuế');
  await dialog.getByTestId('smartart-node-add').click();
  await expect(pane).toHaveValue('Hà Nội\n\tĐà Nẵng\n\tMục 4\nHuế');
  // The caret is on the new node: typing replaces nothing, it continues its line.
  await pane.pressSequentially(' mới');
  await expect(pane).toHaveValue('Hà Nội\n\tĐà Nẵng\n\tMục 4 mới\nHuế');
  await dialog.getByTestId('smartart-node-promote').click();
  await expect(pane).toHaveValue('Hà Nội\n\tĐà Nẵng\nMục 4 mới\nHuế');
  await dialog.getByTestId('smartart-node-down').click();
  await expect(pane).toHaveValue('Hà Nội\n\tĐà Nẵng\nHuế\nMục 4 mới');
  await dialog.getByTestId('smartart-node-remove').click();
  await expect(pane).toHaveValue('Hà Nội\n\tĐà Nẵng\nHuế');
  await dialog.getByTestId('smartart-ok').click();
  await expect(dialog).toHaveCount(0);
  await expect(page.getByRole('tab', { name: 'Sơ đồ thông minh' })).toBeVisible();

  // Text pane again: the chevron layout is the one checked; switch to a picture layout.
  await page.getByRole('tab', { name: 'Sơ đồ thông minh' }).click();
  await page.locator('[data-command="smartart.edit"]').click();
  await expect(dialog.getByTestId('smartart-layout-processChevron')).toBeChecked();
  await expect(pane).toHaveValue('Hà Nội\n\tĐà Nẵng\nHuế');
  await dialog.getByTestId('smartart-layout-pictureStrip').check();
  await dialog.getByTestId('smartart-ok').click();
  await expect(dialog).toHaveCount(0);

  // Save, reload, reopen: the same layout of the library and outline.
  const download = page.waitForEvent('download');
  await page.keyboard.press('Control+s');
  const saved = await (await download).path();
  // Kept for validation (bo-cli validate tests/fidelity/output/web-smartart-library.pptx).
  mkdirSync(outDir, { recursive: true });
  copyFileSync(saved, join(outDir, 'web-smartart-library.pptx'));
  await page.reload();
  await openDeck(page, saved);
  await page.getByTestId('thumb-2').click();
  await expect(page.getByTestId('slide-status')).toHaveText(/^Slide 2 \/ \d+$/);
  const slideBox = (await page.getByTestId('slide-overlay').boundingBox())!;
  await page.mouse.click(slideBox.x + slideBox.width * 0.5, slideBox.y + slideBox.height * 0.4);
  await expect(page.getByRole('tab', { name: 'Sơ đồ thông minh' })).toBeVisible();
  await page.getByRole('tab', { name: 'Sơ đồ thông minh' }).click();
  await page.locator('[data-command="smartart.edit"]').click();
  await expect(dialog.getByTestId('smartart-layout-pictureStrip')).toBeChecked();
  await expect(dialog.getByTestId('smartart-text')).toHaveValue('Hà Nội\n\tĐà Nẵng\nHuế');
  await dialog.getByRole('button', { name: 'Huỷ' }).click();
  expect(errors).toEqual([]);
});
