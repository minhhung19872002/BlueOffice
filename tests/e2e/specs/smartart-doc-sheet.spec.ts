import { expect, test, type Page } from '@playwright/test';
import { copyFileSync, mkdirSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

// CORE-SMART-005: SmartArt in BlueDoc and BlueSheet — insert from the layout library, edit the text
// pane (demote a line), change layout / colours, save and reopen.
const outDir = join(dirname(fileURLToPath(import.meta.url)), '..', '..', 'fidelity', 'output');
const cmd = (page: Page, id: string) => page.locator(`[data-command="${id}"]`);

async function insertProcess(page: Page) {
  await page.getByRole('tab', { name: 'Chèn' }).click();
  await cmd(page, 'insert.smartart').click();
  const dialog = page.getByTestId('smartart-dialog');
  await expect(dialog.getByTestId('smartart-layout-process')).toBeChecked();
  await dialog.getByTestId('smartart-text').fill('Tiếp nhận\nXử lý\nTrả kết quả');
  await dialog.getByTestId('smartart-ok').click();
  await expect(dialog).toHaveCount(0);
}

async function editToHierarchy(page: Page) {
  await page.getByRole('tab', { name: 'SmartArt' }).click();
  await cmd(page, 'smartart.edit').click();
  const dialog = page.getByTestId('smartart-dialog');
  const pane = dialog.getByTestId('smartart-text');
  await expect(pane).toHaveValue('Tiếp nhận\nXử lý\nTrả kết quả');
  await dialog.getByTestId('smartart-layout-hierarchy').check();
  await pane.fill('Giám đốc\nKỹ thuật');
  await pane.press('Control+End');
  await pane.press('Tab');
  await expect(pane).toHaveValue('Giám đốc\n\tKỹ thuật');
  await dialog.getByTestId('smartart-colors').selectOption('green');
  await dialog.getByTestId('smartart-ok').click();
  await expect(dialog).toHaveCount(0);
}

async function expectEdited(page: Page) {
  await page.getByRole('tab', { name: 'SmartArt' }).click();
  await cmd(page, 'smartart.edit').click();
  const dialog = page.getByTestId('smartart-dialog');
  await expect(dialog.getByTestId('smartart-text')).toHaveValue('Giám đốc\n\tKỹ thuật');
  await expect(dialog.getByTestId('smartart-layout-hierarchy')).toBeChecked();
  await expect(dialog.getByTestId('smartart-colors')).toHaveValue('green');
  await page.keyboard.press('Escape');
  await expect(dialog).toHaveCount(0);
}

test('BlueDoc: inserts and edits a SmartArt, kept after save and reopen', async ({ page }) => {
  const errors: string[] = [];
  page.on('pageerror', (e) => errors.push(e.message));
  await page.goto('/doc');
  await page.getByTestId('new-document').click();
  await expect(page.getByTestId('caret')).toBeVisible();
  await expect(page.getByRole('tab', { name: 'SmartArt' })).toHaveCount(0);
  await insertProcess(page);
  // The new diagram stays selected, with its own ribbon tab.
  await expect(page.getByRole('tab', { name: 'SmartArt' })).toBeVisible();
  await editToHierarchy(page);

  const download = page.waitForEvent('download');
  await page.keyboard.press('Control+s');
  const saved = await (await download).path();
  mkdirSync(outDir, { recursive: true });
  copyFileSync(saved, join(outDir, 'web-smartart.docx'));
  await page.reload();
  const chooser = page.waitForEvent('filechooser');
  await page.getByRole('button', { name: 'Mở tệp từ máy' }).click();
  await (await chooser).setFiles(saved);
  await expect(page.locator('.bo-page[data-page="1"]')).toHaveAttribute('data-rendered', 'true');
  // Select the whole (one-paragraph) document: the diagram.
  await page.keyboard.press('Control+a');
  await expect(page.getByRole('tab', { name: 'SmartArt' })).toBeVisible();
  await expectEdited(page);
  expect(errors).toEqual([]);
});

test('BlueSheet: inserts, edits and deletes a SmartArt, kept after save and reopen', async ({
  page,
}) => {
  const errors: string[] = [];
  page.on('pageerror', (e) => errors.push(e.message));
  await page.goto('/sheet');
  await expect(page.getByTestId('sheet-canvas')).toHaveAttribute('data-rendered', /\d+/);
  await insertProcess(page);
  await expect(page.getByTestId('smartart-selection')).toBeVisible();
  await expect(page.getByRole('tab', { name: 'SmartArt' })).toBeVisible();
  await editToHierarchy(page);

  await page.getByLabel('Ô đang nhập').focus();
  const download = page.waitForEvent('download');
  await page.keyboard.press('Control+s');
  const saved = await (await download).path();
  mkdirSync(outDir, { recursive: true });
  copyFileSync(saved, join(outDir, 'web-smartart.xlsx'));
  await page.reload();
  await expect(page.getByRole('tab', { name: 'Sheet1' })).toBeVisible();
  const chooser = page.waitForEvent('filechooser');
  await cmd(page, 'file.open').click();
  await (await chooser).setFiles(saved);
  await expect(page.getByTestId('sheet-canvas')).toHaveAttribute('data-rendered', /\d+/);
  // The diagram covers A1:F10: click inside it.
  const canvas = (await page.getByTestId('sheet-canvas').boundingBox())!;
  await page.mouse.click(canvas.x + 200, canvas.y + 100);
  await expect(page.getByTestId('smartart-selection')).toBeVisible();
  await expectEdited(page);

  // Delete it from its tab.
  await page.getByRole('tab', { name: 'SmartArt' }).click();
  await cmd(page, 'smartart.delete').click();
  await expect(page.getByTestId('smartart-selection')).toHaveCount(0);
  await expect(page.getByRole('tab', { name: 'SmartArt' })).toHaveCount(0);
  expect(errors).toEqual([]);
});
