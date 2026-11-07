import { expect, test, type Page } from '@playwright/test';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const here = dirname(fileURLToPath(import.meta.url));
const deck = join(here, '..', '..', 'corpus', 'slide', 'slide-text-bullets.pptx');

test.use({ permissions: ['clipboard-read', 'clipboard-write'] });

async function openDeck(page: Page, path: string) {
  const chooser = page.waitForEvent('filechooser');
  await page.getByRole('button', { name: 'Mở', exact: true }).click();
  await (await chooser).setFiles(path);
  await expect(page.getByTestId('slide-status')).toHaveText(/^Slide 1 \/ \d+$/);
  await expect(page.getByTestId('slide-overlay')).toBeVisible();
}

async function box(page: Page, testId: string) {
  const b = await page.getByTestId(testId).first().boundingBox();
  expect(b).not.toBeNull();
  return b!;
}

/** Select characters [from, to) of the shape text with the keyboard. */
async function selectChars(page: Page, from: number, to: number) {
  await page.keyboard.press('Control+Home');
  for (let i = 0; i < from; i++) await page.keyboard.press('ArrowRight');
  for (let i = from; i < to; i++) await page.keyboard.press('Shift+ArrowRight');
}

test('bolds part of a shape text, copies/pastes a shape, edits a table cell, saves and reopens', async ({
  page,
}) => {
  const errors: string[] = [];
  page.on('pageerror', (e) => errors.push(e.message));
  await page.goto('/slide');
  await openDeck(page, deck);
  const bold = page.getByRole('button', { name: 'Đậm', exact: true });

  // A new slide keeps the test away from the deck's own shapes.
  await page.getByRole('button', { name: '+ Thêm slide' }).click();
  await expect(page.getByTestId('slide-status')).toHaveText(/^Slide 2 \/ \d+$/);

  // Text box: type, select "chào" and make only that bold.
  await page.getByRole('tab', { name: 'Chèn' }).click();
  await page.getByRole('button', { name: 'Hộp văn bản' }).click();
  const tb = await box(page, 'selection-box');
  // Home tab (bold state) before editing: the keyboard stays in the text.
  await page.getByRole('tab', { name: 'Trang đầu' }).click();
  await page.mouse.dblclick(tb.x + tb.width / 2, tb.y + tb.height / 2);
  const editor = page.getByTestId('slide-text-editor');
  await expect(editor).toBeFocused();
  await expect(page.getByTestId('text-caret')).toBeAttached();
  await page.keyboard.insertText('Xin chào Việt Nam');
  await expect(editor).toHaveValue('Xin chào Việt Nam');
  await selectChars(page, 4, 8);
  await expect(page.getByTestId('text-selection')).toHaveCount(1);
  await page.keyboard.press('Control+b');
  await expect(bold).toHaveAttribute('aria-pressed', 'true');
  await selectChars(page, 0, 3);
  await expect(bold).toHaveAttribute('aria-pressed', 'false');
  await page.keyboard.press('Escape');
  await expect(editor).toHaveCount(0);

  // Copy / paste the text box: the copy lands 12 pt lower right.
  await page.mouse.click(tb.x + tb.width / 2, tb.y + tb.height / 2);
  await expect(page.getByTestId('selection-box')).toHaveCount(1);
  const orig = await box(page, 'selection-box');
  await page.keyboard.press('Control+c');
  await page.keyboard.press('Control+v');
  await expect.poll(async () => (await box(page, 'selection-box')).x - orig.x).toBeGreaterThan(5);
  const pasted = await box(page, 'selection-box');
  expect(pasted.y - orig.y).toBeGreaterThan(5);
  // Move the copy away so both are visible.
  await page.keyboard.press('Shift+ArrowDown');
  await page.keyboard.press('Shift+ArrowDown');

  // Table: double-click a cell and type.
  await page.getByRole('tab', { name: 'Chèn' }).click();
  await page.getByRole('button', { name: 'Bảng' }).click();
  // The new table (60 % of the slide width) is selected.
  await expect
    .poll(async () => (await page.getByTestId('selection-box').first().boundingBox())?.width ?? 0)
    .toBeGreaterThan(500);
  // Below the text boxes.
  const t0 = await box(page, 'selection-box');
  for (let i = 0; i < 12; i++) await page.keyboard.press('Shift+ArrowDown');
  await expect.poll(async () => (await box(page, 'selection-box')).y - t0.y).toBeGreaterThan(100);
  const t = await box(page, 'selection-box');
  const cell = { x: t.x + t.width / 6, y: t.y + (t.height * 3) / 6 };
  await page.mouse.dblclick(cell.x, cell.y);
  const cellEditor = page.getByTestId('table-cell-editor');
  await expect(cellEditor).toBeFocused();
  await page.keyboard.insertText('Hà Nội');
  await page.keyboard.press('Tab');
  await page.keyboard.insertText('Đà Nẵng');
  await page.keyboard.press('Escape');
  await expect(page.getByRole('tab', { name: 'Bảng' })).toBeVisible();
  await expect(page.getByTestId('table-cell-selected')).toHaveCount(1);

  // Save and reopen.
  const download = page.waitForEvent('download');
  await page.keyboard.press('Control+s');
  const saved = await (await download).path();
  await page.reload();
  await openDeck(page, saved);
  await page.getByTestId('thumb-2').click();
  await expect(page.getByTestId('slide-status')).toHaveText(/^Slide 2 \/ \d+$/);
  // The bold part survived.
  await page.mouse.click(orig.x + orig.width / 2, orig.y + orig.height / 2);
  const again = await box(page, 'selection-box');
  expect(Math.abs(again.x - orig.x)).toBeLessThan(2);
  await page.mouse.dblclick(again.x + again.width / 2, again.y + again.height / 2);
  await expect(page.getByTestId('slide-text-editor')).toHaveValue('Xin chào Việt Nam');
  await selectChars(page, 4, 8);
  await expect(bold).toHaveAttribute('aria-pressed', 'true');
  await page.keyboard.press('Escape');
  // Both copies are there and the table cells kept their text.
  await page.keyboard.press('Control+a');
  await expect.poll(() => page.getByTestId('selection-box').count()).toBeGreaterThanOrEqual(3);
  await page.keyboard.press('Escape');
  await expect(page.getByTestId('selection-box')).toHaveCount(0);
  await page.mouse.click(cell.x, cell.y);
  await page.mouse.dblclick(cell.x, cell.y);
  await expect(page.getByTestId('table-cell-editor')).toHaveValue('Hà Nội');
  await page.keyboard.press('Tab');
  await expect(page.getByTestId('table-cell-editor')).toHaveValue('Đà Nẵng');
  expect(errors).toEqual([]);
});
