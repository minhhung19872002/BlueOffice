import { expect, test, type Locator, type Page } from '@playwright/test';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

// CORE-SMART-004, as in the office suites the apps are compatible with: the selected diagram
// shows its frame with the text pane arrow on the left edge (BlueDoc too); a click on a shape
// inside the diagram selects that shape — it is marked, Add Shape › After adds the new shape
// after it and the text pane's caret goes to its line; a layout under the pointer in the gallery
// previews over the diagram itself.
const here = dirname(fileURLToPath(import.meta.url));
const deck = join(here, '..', '..', 'corpus', 'slide', 'slide-text-bullets.pptx');
const cmd = (page: Page, id: string) => page.locator(`[data-command="${id}"]`).first();
const TAB = 'Sơ đồ thông minh';

async function insertProcess(page: Page) {
  await page.getByRole('tab', { name: 'Chèn' }).click();
  await cmd(page, 'insert.smartart').click();
  const dialog = page.getByTestId('smartart-dialog');
  await dialog.getByTestId('smartart-text').fill('Tiếp nhận\nXử lý\nTrả kết quả');
  await dialog.getByTestId('smartart-ok').click();
  await expect(dialog).toHaveCount(0);
  await expect(page.getByRole('tab', { name: TAB })).toBeVisible();
}

async function box(l: Locator) {
  const b = await l.boundingBox();
  expect(b).not.toBeNull();
  return b ?? { x: 0, y: 0, width: 0, height: 0 };
}

/** The pane's lines. */
async function lines(page: Page): Promise<string[]> {
  const rows = page.getByTestId('smartart-pane').locator('li input');
  return rows.evaluateAll((els) => els.map((e) => (e as HTMLInputElement).value));
}

/**
 * The shared flow: `frame` is the diagram's frame on screen. A click in the middle of the frame
 * picks the middle shape of the process; then the first shape; Add Shape › After the current
 * one; the text pane from the arrow on the frame shows the new empty shape after it.
 */
async function pickFlow(page: Page, frame: Locator) {
  await expect(frame).toBeVisible();
  const f = await box(frame);
  // The arrow sits on the frame's left edge, half way down.
  const arrow = await box(page.getByTestId('smartart-pane-toggle'));
  expect(Math.abs(arrow.x + arrow.width - f.x)).toBeLessThan(3);
  expect(Math.abs(arrow.y + arrow.height / 2 - (f.y + f.height / 2))).toBeLessThan(3);
  // The middle of a three-step process is its second shape.
  await page.mouse.click(f.x + f.width / 2, f.y + f.height / 2);
  const mark = page.getByTestId('smartart-node-selection');
  await expect(mark).toHaveAttribute('data-node', '1');
  const m = await box(mark);
  expect(m.x).toBeGreaterThan(f.x + f.width / 4);
  expect(m.x + m.width).toBeLessThan(f.x + (f.width * 3) / 4);
  // The first shape: at a sixth of the width.
  await page.mouse.click(f.x + f.width / 6, f.y + f.height / 2);
  await expect(mark).toHaveAttribute('data-node', '0');
  // Add Shape › After acts on the shape picked: the new one comes second.
  await page.getByRole('tab', { name: TAB }).click();
  await page.getByTestId('smartart-add-shape').click();
  await page.getByTestId('smartart-addAfter').click();
  await page.getByTestId('smartart-pane-toggle').click();
  await expect(page.getByTestId('smartart-pane')).toBeVisible();
  expect(await lines(page)).toEqual(['Tiếp nhận', '', 'Xử lý', 'Trả kết quả']);
  await page.getByTestId('smartart-pane-close').click();
  // The layout gallery's live preview lies over the diagram's frame.
  await page.getByRole('tab', { name: TAB }).click();
  await page.getByTestId('smartart-layouts').click();
  await page.getByTestId('smartart-gallery-cycle').hover();
  const live = page.getByTestId('smartart-live-preview');
  await expect(live).toBeVisible();
  const l = await box(live);
  const f2 = await box(frame);
  // (Within the frame's border.)
  expect(Math.abs(l.x - f2.x)).toBeLessThan(5);
  expect(Math.abs(l.y - f2.y)).toBeLessThan(5);
  expect(Math.abs(l.width - f2.width)).toBeLessThan(6);
  await page.keyboard.press('Escape');
  await expect(live).toHaveCount(0);
}

test('BlueDoc: the frame of the diagram and a click on one of its shapes', async ({ page }) => {
  const errors: string[] = [];
  page.on('pageerror', (e) => errors.push(e.message));
  await page.goto('/doc');
  await page.getByTestId('new-document').click();
  await expect(page.getByTestId('caret')).toBeVisible();
  await insertProcess(page);
  await pickFlow(page, page.getByTestId('smartart-frame'));
  expect(errors).toEqual([]);
});

test('BlueSheet: a click on a shape of the diagram selects it', async ({ page }) => {
  const errors: string[] = [];
  page.on('pageerror', (e) => errors.push(e.message));
  await page.goto('/sheet');
  await expect(page.getByTestId('sheet-canvas')).toHaveAttribute('data-rendered', /\d+/);
  await insertProcess(page);
  await pickFlow(page, page.getByTestId('smartart-selection'));
  expect(errors).toEqual([]);
});

test('BlueSlide: a click on a shape of the diagram selects it', async ({ page }) => {
  const errors: string[] = [];
  page.on('pageerror', (e) => errors.push(e.message));
  await page.goto('/slide');
  const chooser = page.waitForEvent('filechooser');
  await page.getByRole('button', { name: 'Mở', exact: true }).click();
  await (await chooser).setFiles(deck);
  await expect(page.getByTestId('slide-overlay')).toBeVisible();
  await page.getByRole('button', { name: '+ Thêm slide' }).click();
  await insertProcess(page);
  await pickFlow(page, page.getByTestId('smartart-frame'));
  expect(errors).toEqual([]);
});
