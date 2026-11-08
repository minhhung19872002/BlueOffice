import { expect, test, type Page } from '@playwright/test';
import { copyFileSync, mkdirSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

// CORE-SMART-003 / 004: insert a SmartArt from the layout library, edit it in the text pane, change its
// layout, save and reopen, convert it to shapes.
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

test('inserts, edits and converts a SmartArt; kept after save and reopen', async ({ page }) => {
  const errors: string[] = [];
  page.on('pageerror', (e) => errors.push(e.message));
  await page.goto('/slide');
  await openDeck(page, deck);
  await page.getByRole('button', { name: '+ Thêm slide' }).click();
  await expect(page.getByTestId('slide-status')).toHaveText(/^Slide 2 \/ \d+$/);

  // Insert: a process of three steps typed in the text pane.
  await page.getByRole('tab', { name: 'Chèn' }).click();
  await page.getByRole('button', { name: 'SmartArt' }).click();
  const dialog = page.getByTestId('smartart-dialog');
  await expect(dialog.getByTestId('smartart-layout-process')).toBeChecked();
  await dialog.getByTestId('smartart-text').fill('Tiếp nhận\nXử lý\nTrả kết quả');
  await dialog.getByTestId('smartart-ok').click();
  await expect(dialog).toHaveCount(0);
  // Selected, with its own ribbon tab.
  await expect(page.getByRole('tab', { name: 'SmartArt' })).toBeVisible();

  // Text pane: a hierarchy with two departments under the director (Tab demotes a line).
  await page.getByRole('tab', { name: 'SmartArt' }).click();
  await page.getByRole('button', { name: 'Ngăn văn bản' }).click();
  await expect(dialog.getByTestId('smartart-text')).toHaveValue('Tiếp nhận\nXử lý\nTrả kết quả');
  await dialog.getByTestId('smartart-layout-hierarchy').check();
  const pane = dialog.getByTestId('smartart-text');
  await pane.fill('Giám đốc\nKỹ thuật\nKinh doanh');
  // Demote lines 2 and 3 with Tab.
  await pane.press('Control+Home');
  await pane.press('ArrowDown');
  await pane.press('Tab');
  await pane.press('ArrowDown');
  await pane.press('Tab');
  await expect(pane).toHaveValue('Giám đốc\n\tKỹ thuật\n\tKinh doanh');
  // Colours and look.
  await dialog.getByTestId('smartart-colors').selectOption('green');
  await dialog.getByTestId('smartart-style').selectOption('outline');
  await dialog.getByTestId('smartart-ok').click();
  await expect(dialog).toHaveCount(0);

  // Save, reload, reopen: the SmartArt reads back with its outline and layout.
  const download = page.waitForEvent('download');
  await page.keyboard.press('Control+s');
  const saved = await (await download).path();
  // Kept for validation (bo-cli validate tests/fidelity/output/web-smartart.pptx).
  mkdirSync(outDir, { recursive: true });
  copyFileSync(saved, join(outDir, 'web-smartart.pptx'));
  await page.reload();
  await openDeck(page, saved);
  await page.getByTestId('thumb-2').click();
  await expect(page.getByTestId('slide-status')).toHaveText(/^Slide 2 \/ \d+$/);
  // Click inside the diagram's frame (centre of the slide).
  const slideBox = (await page.getByTestId('slide-overlay').boundingBox())!;
  await page.mouse.click(slideBox.x + slideBox.width * 0.5, slideBox.y + slideBox.height * 0.52);
  await expect(page.getByRole('tab', { name: 'SmartArt' })).toBeVisible();
  await page.getByRole('tab', { name: 'SmartArt' }).click();
  await page.getByRole('button', { name: 'Ngăn văn bản' }).click();
  await expect(dialog.getByTestId('smartart-text')).toHaveValue(
    'Giám đốc\n\tKỹ thuật\n\tKinh doanh',
  );
  await expect(dialog.getByTestId('smartart-layout-hierarchy')).toBeChecked();
  await expect(dialog.getByTestId('smartart-colors')).toHaveValue('green');
  await expect(dialog.getByTestId('smartart-style')).toHaveValue('outline');
  await dialog.getByRole('button', { name: 'Huỷ' }).click();

  // Convert to shapes: the SmartArt tab goes away (a group of shapes is selected).
  await page.getByRole('button', { name: 'Chuyển thành hình' }).click();
  await expect(page.getByRole('tab', { name: 'SmartArt' })).toHaveCount(0);
  expect(errors).toEqual([]);
});
