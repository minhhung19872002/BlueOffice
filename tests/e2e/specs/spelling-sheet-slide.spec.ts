import { expect, test, type Page } from '@playwright/test';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

// CORE-SPELL-006: spelling in BlueSheet cells and BlueSlide text boxes.

const here = dirname(fileURLToPath(import.meta.url));
const deck = join(here, '..', '..', 'corpus', 'slide', 'slide-text-bullets.pptx');

async function goTo(page: Page, ref: string) {
  const box = page.getByLabel('Hộp tên');
  await box.click();
  await box.fill(ref);
  await box.press('Enter');
  await expect(page.getByLabel('Ô đang nhập')).toBeFocused();
}

async function valueOf(page: Page, ref: string) {
  await goTo(page, ref);
  return page.getByLabel('Thanh công thức');
}

/** Right click just above a wavy mark (on the word it underlines). */
async function rightClickMark(page: Page, testId: string) {
  const b = (await page.getByTestId(testId).first().boundingBox())!;
  expect(b).not.toBeNull();
  await page.mouse.click(b.x + b.width / 2, b.y - 4, { button: 'right' });
}

test('BlueSheet: underlines typed words, F7 replaces everywhere in one undo step', async ({
  page,
}) => {
  const errors: string[] = [];
  page.on('pageerror', (e) => errors.push(e.message));
  await page.goto('/sheet');
  await expect(page.getByRole('tab', { name: 'Sheet1' })).toBeVisible();
  await expect(page.getByTestId('sheet-canvas')).toHaveAttribute('data-rendered', /\d+/);

  // Typing in A1: "chaò" (tone on the wrong vowel) is underlined while the cell is edited.
  await page.keyboard.type('Xin chaò');
  const marks = page.getByTestId('cell-spell-mark');
  await expect(marks.first()).toBeVisible({ timeout: 15_000 });
  await expect(marks.first()).toHaveText('chaò');
  await rightClickMark(page, 'cell-spell-mark');
  const menu = page.getByTestId('spell-menu');
  await expect(menu).toBeVisible();
  await menu.getByRole('menuitem', { name: 'chào', exact: true }).click();
  await expect(page.getByLabel('Thanh công thức')).toHaveValue('Xin chào');
  await expect(marks).toHaveCount(0, { timeout: 15_000 });
  await page.keyboard.press('Enter');

  // Text cells with the same typo, a number and a formula (formulas are never checked).
  for (const v of ['the documnet', 'documnet again', '12345', '="documnet"']) {
    await page.keyboard.type(v);
    await page.keyboard.press('Enter');
  }
  await expect(await valueOf(page, 'A1')).toHaveValue('Xin chào');

  // F7: check the workbook with any dictionary (Vietnamese and English words both pass).
  await page.keyboard.press('F7');
  const dialog = page.getByTestId('spellcheck-dialog');
  await expect(dialog).toBeVisible();
  await page.getByTestId('spellcheck-sheet-language').selectOption('');
  await expect(page.getByTestId('spellcheck-word')).toHaveText('documnet', { timeout: 15_000 });
  await expect(page.getByTestId('spellcheck-where')).toContainText('A2');
  await page.getByTestId('spellcheck-replacement').fill('document');
  await page.getByTestId('spellcheck-change-all').click();
  // A5 holds a formula: nothing else to check.
  await expect(page.getByTestId('spellcheck-done')).toBeVisible({ timeout: 15_000 });
  await page.getByTestId('spellcheck-close').click();
  await expect(dialog).toHaveCount(0);

  await expect(await valueOf(page, 'A2')).toHaveValue('the document');
  await expect(await valueOf(page, 'A3')).toHaveValue('document again');
  await expect(await valueOf(page, 'A5')).toHaveValue('="documnet"');

  // Replace everywhere was one edit: one undo brings both cells back.
  await page.getByLabel('Ô đang nhập').focus();
  await page.keyboard.press('Control+z');
  await expect(await valueOf(page, 'A2')).toHaveValue('the documnet');
  await expect(await valueOf(page, 'A3')).toHaveValue('documnet again');
  expect(errors).toEqual([]);
});

test('BlueSlide: underlines text box words, corrects while editing, F7 walks the deck', async ({
  page,
}) => {
  const errors: string[] = [];
  page.on('pageerror', (e) => errors.push(e.message));
  await page.goto('/slide');
  const chooser = page.waitForEvent('filechooser');
  await page.getByRole('button', { name: 'Mở', exact: true }).click();
  await (await chooser).setFiles(deck);
  await expect(page.getByTestId('slide-status')).toHaveText(/^Slide 1 \/ \d+$/);
  await expect(page.getByTestId('slide-overlay')).toBeVisible();

  // A new slide with a text box keeps the test away from the deck's own text.
  await page.getByRole('button', { name: '+ Thêm slide' }).click();
  await expect(page.getByTestId('slide-status')).toHaveText(/^Slide 2 \/ \d+$/);
  await page.getByRole('tab', { name: 'Chèn' }).click();
  await page.getByRole('button', { name: 'Hộp văn bản' }).click();
  const tb = (await page.getByTestId('selection-box').first().boundingBox())!;
  await page.mouse.dblclick(tb.x + tb.width / 2, tb.y + tb.height / 2);
  const editor = page.getByTestId('slide-text-editor');
  await expect(editor).toBeFocused();
  await page.keyboard.insertText('Xin chaò the documnet');

  // Two marks on the canvas: "chaò" and "documnet".
  const marks = page.getByTestId('slide-spell-mark');
  await expect(marks).toHaveCount(2, { timeout: 15_000 });

  // Right click the first one while the text box is edited: the correction goes into the text.
  await rightClickMark(page, 'slide-spell-mark');
  const menu = page.getByTestId('spell-menu');
  await expect(menu).toBeVisible();
  await menu.getByRole('menuitem', { name: 'chào', exact: true }).click();
  await expect(editor).toHaveValue('Xin chào the documnet', { timeout: 15_000 });
  await expect(marks).toHaveCount(1, { timeout: 15_000 });
  await page.keyboard.press('Escape');
  await expect(editor).toHaveCount(0);

  // F7 walks the whole deck; skip what the deck itself holds until the typo of slide 2.
  await page.keyboard.press('F7');
  await expect(page.getByTestId('spellcheck-dialog')).toBeVisible();
  const word = page.getByTestId('spellcheck-word');
  await expect(word.or(page.getByTestId('spellcheck-done'))).toBeVisible({ timeout: 15_000 });
  for (let i = 0; i < 50 && (await word.count()) > 0; i++) {
    if ((await word.textContent()) === 'documnet') break;
    await page.getByTestId('spellcheck-ignore-once').click();
  }
  await expect(word).toHaveText('documnet');
  await expect(page.getByTestId('spellcheck-where')).toHaveText('Trang chiếu 2');
  await page.getByTestId('spellcheck-replacement').fill('document');
  await page.getByTestId('spellcheck-change').click();
  await expect(word.or(page.getByTestId('spellcheck-done'))).toBeVisible({ timeout: 15_000 });
  await page.getByTestId('spellcheck-close').click();
  await expect(marks).toHaveCount(0, { timeout: 15_000 });

  // The correction is an undoable edit.
  await page.keyboard.press('Control+z');
  await expect(marks).toHaveCount(1, { timeout: 15_000 });
  expect(errors).toEqual([]);
});
