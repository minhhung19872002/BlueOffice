import { expect, test, type Page } from '@playwright/test';
import { copyFileSync, mkdirSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

// CORE-SMART-003 / 004: the feature's own name in the interface ("Sơ đồ thông minh"), layout
// thumbnails in the dialog, a picture on a node (kept after save and reopen), editing a node's
// text on the slide (one undo step), "Convert to shapes" in BlueDoc and BlueSheet.
const here = dirname(fileURLToPath(import.meta.url));
const deck = join(here, '..', '..', 'corpus', 'slide', 'slide-text-bullets.pptx');
const picture = join(here, '..', '..', '..', 'assets', 'templates', 'doc', 'bao-cao.png');
const outDir = join(here, '..', '..', 'fidelity', 'output');
const NAME = 'Sơ đồ thông minh';
const cmd = (page: Page, id: string) => page.locator(`[data-command="${id}"]`);

async function openDeck(page: Page, path: string) {
  const chooser = page.waitForEvent('filechooser');
  await page.getByRole('button', { name: 'Mở', exact: true }).click();
  await (await chooser).setFiles(path);
  await expect(page.getByTestId('slide-status')).toHaveText(/^Slide 1 \/ \d+$/);
  await expect(page.getByTestId('slide-overlay')).toBeVisible();
}

/** Choose the picture of the node at the caret of the text pane. */
async function choosePicture(page: Page) {
  const chooser = page.waitForEvent('filechooser');
  await page.getByTestId('smartart-dialog').getByTestId('smartart-node-picture').click();
  await (await chooser).setFiles(picture);
}

test('BlueSlide: own name, previews, a node picture and on-canvas text edit', async ({ page }) => {
  const errors: string[] = [];
  page.on('pageerror', (e) => errors.push(e.message));
  await page.goto('/slide');
  await openDeck(page, deck);
  await page.getByRole('button', { name: '+ Thêm slide' }).click();
  await expect(page.getByTestId('slide-status')).toHaveText(/^Slide 2 \/ \d+$/);

  // The feature is named in BlueOffice's own words: button, dialog title, tab.
  await page.getByRole('tab', { name: 'Chèn' }).click();
  await expect(page.getByRole('button', { name: 'SmartArt' })).toHaveCount(0);
  await page.getByRole('button', { name: NAME }).click();
  const dialog = page.getByTestId('smartart-dialog');
  await expect(dialog).toContainText(`Chèn ${NAME.toLowerCase()}`);
  await expect(dialog).not.toContainText('SmartArt');

  // Every layout has a thumbnail drawn by the engine; other colours draw them again.
  const thumb = dialog.getByTestId('smartart-preview-pictureStrip');
  await expect(thumb).toBeVisible();
  await expect(dialog.locator('img[data-testid^="smartart-preview-"]')).toHaveCount(17);
  const before = await thumb.getAttribute('src');
  expect(before).toContain('data:image/svg+xml');
  await dialog.getByTestId('smartart-colors').selectOption('green');
  await expect(thumb).not.toHaveAttribute('src', before!);

  // A picture strip of three places; the first and the last get a picture.
  await dialog.getByTestId('smartart-layout-pictureStrip').check();
  const pane = dialog.getByTestId('smartart-text');
  await pane.fill('Hà Nội\nHuế\nĐà Nẵng');
  await pane.press('Control+Home');
  await choosePicture(page);
  await expect(dialog.getByTestId('smartart-pictures').locator('li')).toHaveText(['Hà Nội']);
  await expect(dialog.getByTestId('smartart-node-picture')).toHaveText('Đổi ảnh…');
  await pane.press('Control+End');
  await choosePicture(page);
  await expect(dialog.getByTestId('smartart-pictures').locator('li')).toHaveText([
    'Hà Nội',
    'Đà Nẵng',
  ]);
  // Remove it again from the last node.
  await dialog.getByTestId('smartart-node-removePicture').click();
  await expect(dialog.getByTestId('smartart-pictures').locator('li')).toHaveText(['Hà Nội']);
  await dialog.getByTestId('smartart-ok').click();
  await expect(dialog).toHaveCount(0);
  await expect(page.getByRole('tab', { name: NAME })).toBeVisible();

  // Edit a node on the slide: double-click the middle caption, type, Enter.
  const slide = (await page.getByTestId('slide-overlay').boundingBox())!;
  // The frame takes 80 % × 65 % of the slide from (10 %, 20 %): three rows, captions on the right.
  const at = { x: slide.x + slide.width * 0.6, y: slide.y + slide.height * 0.525 };
  await page.mouse.dblclick(at.x, at.y);
  const editor = page.getByTestId('smartart-node-editor');
  await expect(editor).toBeVisible();
  await expect(editor).toHaveValue('Huế');
  await editor.fill('Cố đô Huế');
  await editor.press('Enter');
  await expect(editor).toHaveCount(0);
  await page.getByRole('tab', { name: NAME }).click();
  await page.getByRole('button', { name: 'Ngăn văn bản' }).click();
  await expect(pane).toHaveValue('Hà Nội\nCố đô Huế\nĐà Nẵng');
  // The picture stayed on its node.
  await expect(dialog.getByTestId('smartart-pictures').locator('li')).toHaveText(['Hà Nội']);
  await dialog.getByRole('button', { name: 'Huỷ' }).click();
  // One undo step takes the edit back; redo brings it again.
  await page.getByTestId('slide-overlay').click({ position: { x: 5, y: 5 } });
  await page.keyboard.press('Control+z');
  await page.mouse.dblclick(at.x, at.y);
  await expect(editor).toHaveValue('Huế');
  await editor.press('Escape');
  await expect(editor).toHaveCount(0);
  await page.keyboard.press('Control+y');

  // Save, reload, reopen: the outline, the layout and the picture are in the file.
  const download = page.waitForEvent('download');
  await page.keyboard.press('Control+s');
  const saved = await (await download).path();
  // Kept for validation (bo-cli validate tests/fidelity/output/web-smartart-pictures.pptx).
  mkdirSync(outDir, { recursive: true });
  copyFileSync(saved, join(outDir, 'web-smartart-pictures.pptx'));
  await page.reload();
  await openDeck(page, saved);
  await page.getByTestId('thumb-2').click();
  await expect(page.getByTestId('slide-status')).toHaveText(/^Slide 2 \/ \d+$/);
  const box = (await page.getByTestId('slide-overlay').boundingBox())!;
  await page.mouse.click(box.x + box.width * 0.6, box.y + box.height * 0.525);
  await expect(page.getByRole('tab', { name: NAME })).toBeVisible();
  await page.getByRole('tab', { name: NAME }).click();
  await page.getByRole('button', { name: 'Ngăn văn bản' }).click();
  await expect(dialog.getByTestId('smartart-layout-pictureStrip')).toBeChecked();
  await expect(pane).toHaveValue('Hà Nội\nCố đô Huế\nĐà Nẵng');
  await expect(dialog.getByTestId('smartart-pictures').locator('li')).toHaveText(['Hà Nội']);
  await dialog.getByRole('button', { name: 'Huỷ' }).click();
  expect(errors).toEqual([]);
});

async function insertProcess(page: Page) {
  await page.getByRole('tab', { name: 'Chèn' }).click();
  await cmd(page, 'insert.smartart').click();
  const dialog = page.getByTestId('smartart-dialog');
  await expect(dialog).toContainText(`Chèn ${NAME.toLowerCase()}`);
  await expect(dialog.getByTestId('smartart-preview-process')).toBeVisible();
  await dialog.getByTestId('smartart-text').fill('Tiếp nhận\nXử lý\nTrả kết quả');
  await dialog.getByTestId('smartart-ok').click();
  await expect(dialog).toHaveCount(0);
}

test('BlueDoc: a diagram is converted to a group of shapes, kept in the file', async ({ page }) => {
  const errors: string[] = [];
  page.on('pageerror', (e) => errors.push(e.message));
  await page.goto('/doc');
  await page.getByTestId('new-document').click();
  await expect(page.getByTestId('caret')).toBeVisible();
  await insertProcess(page);
  await expect(page.getByRole('tab', { name: NAME })).toBeVisible();
  await expect(page.getByRole('tab', { name: 'SmartArt' })).toHaveCount(0);

  // Convert to shapes: the frame is no diagram any more, its tab goes.
  await page.getByRole('tab', { name: NAME }).click();
  await expect(cmd(page, 'smartart.toShapes')).toHaveText(/Chuyển thành hình/);
  await cmd(page, 'smartart.toShapes').click();
  await expect(page.getByRole('tab', { name: NAME })).toHaveCount(0);
  // One undo step brings the diagram back; redo converts again.
  await page.keyboard.press('Control+z');
  await expect(page.getByRole('tab', { name: NAME })).toBeVisible();
  await page.keyboard.press('Control+y');
  await expect(page.getByRole('tab', { name: NAME })).toHaveCount(0);

  const download = page.waitForEvent('download');
  await page.keyboard.press('Control+s');
  const saved = await (await download).path();
  // Kept for validation (bo-cli validate tests/fidelity/output/web-smartart-converted.docx).
  mkdirSync(outDir, { recursive: true });
  copyFileSync(saved, join(outDir, 'web-smartart-converted.docx'));
  await page.reload();
  const chooser = page.waitForEvent('filechooser');
  await page.getByRole('button', { name: 'Mở tệp từ máy' }).click();
  await (await chooser).setFiles(saved);
  await expect(page.locator('.bo-page[data-page="1"]')).toHaveAttribute('data-rendered', 'true');
  // The group is read back as shapes: selecting it shows no diagram tab.
  await page.keyboard.press('Control+a');
  await expect(page.getByRole('tab', { name: NAME })).toHaveCount(0);
  expect(errors).toEqual([]);
});

test('BlueSheet: a diagram is converted to a group of shapes, kept in the file', async ({
  page,
}) => {
  const errors: string[] = [];
  page.on('pageerror', (e) => errors.push(e.message));
  await page.goto('/sheet');
  await expect(page.getByTestId('sheet-canvas')).toHaveAttribute('data-rendered', /\d+/);
  await insertProcess(page);
  await expect(page.getByTestId('smartart-selection')).toBeVisible();
  await expect(page.getByRole('tab', { name: NAME })).toBeVisible();
  await page.getByRole('tab', { name: NAME }).click();
  await cmd(page, 'smartart.toShapes').click();
  await expect(page.getByTestId('smartart-selection')).toHaveCount(0);
  await expect(page.getByRole('tab', { name: NAME })).toHaveCount(0);

  await page.getByLabel('Ô đang nhập').focus();
  const download = page.waitForEvent('download');
  await page.keyboard.press('Control+s');
  const saved = await (await download).path();
  // Kept for validation (bo-cli validate tests/fidelity/output/web-smartart-converted.xlsx).
  mkdirSync(outDir, { recursive: true });
  copyFileSync(saved, join(outDir, 'web-smartart-converted.xlsx'));
  await page.reload();
  await expect(page.getByRole('tab', { name: 'Sheet1' })).toBeVisible();
  const chooser = page.waitForEvent('filechooser');
  await cmd(page, 'file.open').click();
  await (await chooser).setFiles(saved);
  await expect(page.getByTestId('sheet-canvas')).toHaveAttribute('data-rendered', /\d+/);
  // Where the diagram was there are shapes now: a click there selects no diagram.
  const canvas = (await page.getByTestId('sheet-canvas').boundingBox())!;
  await page.mouse.click(canvas.x + 200, canvas.y + 100);
  await expect(page.getByTestId('smartart-selection')).toHaveCount(0);
  await expect(page.getByRole('tab', { name: NAME })).toHaveCount(0);
  expect(errors).toEqual([]);
});
