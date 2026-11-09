import { expect, test, type Page } from '@playwright/test';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

// CORE-PASTE-001: paste options after a paste (one undo step overall) and the shared
// clipboard history panel, in BlueDoc, BlueSheet and BlueSlide.

const here = dirname(fileURLToPath(import.meta.url));
const deck = join(here, '..', '..', 'corpus', 'slide', 'slide-text-bullets.pptx');

test.use({ permissions: ['clipboard-read', 'clipboard-write'] });

async function newDocument(page: Page) {
  await page.goto('/doc');
  await page.getByTestId('new-document').click();
  await expect(page.getByTestId('caret')).toBeVisible();
}

async function choose(page: Page, label: RegExp) {
  await page.getByTestId('paste-options').click();
  await page.getByTestId('paste-options-menu').getByRole('menuitemradio', { name: label }).click();
}

test('BlueDoc: paste options switch the last paste in one undo step', async ({ page }) => {
  const errors: string[] = [];
  page.on('pageerror', (e) => errors.push(e.message));
  await newDocument(page);
  await page.keyboard.insertText('Một hai');
  await page.keyboard.press('Control+a');
  await page.keyboard.press('Control+b');
  await page.keyboard.press('Control+c');
  // The copy reaches the system clipboard asynchronously: wait before pasting.
  await expect.poll(() => page.evaluate(() => navigator.clipboard.readText())).toBe('Một hai');
  await page.keyboard.press('Control+End');
  await page.keyboard.press('Enter');
  await page.keyboard.press('Control+v');
  const words = page.getByTestId('status-words');
  await expect(words).toHaveText('4 từ');
  await expect(page.getByTestId('paste-options')).toBeVisible();

  // Text only, then back to the source formatting with the keyboard (Alt+Shift+V, digit).
  await choose(page, /Chỉ văn bản thuần/);
  await expect(words).toHaveText('4 từ');
  await page.keyboard.press('Alt+Shift+V');
  const menu = page.getByTestId('paste-options-menu');
  await expect(menu).toBeVisible();
  await expect(menu.getByRole('menuitemradio', { name: /Chỉ văn bản thuần/ })).toHaveAttribute(
    'aria-checked',
    'true',
  );
  await menu.press('1');
  await expect(menu).toHaveCount(0);
  await expect(words).toHaveText('4 từ');

  // As a picture (the copy carries a picture of the selection), then text only again.
  await choose(page, /Dạng ảnh/);
  // The picture counts as one object word, like an equation: "Một hai" + picture.
  await expect(words).toHaveText('3 từ');
  await choose(page, /Chỉ văn bản thuần/);
  await expect(words).toHaveText('4 từ');

  // However often the mode changed, one undo removes the paste.
  await page.keyboard.press('Control+z');
  await expect(words).toHaveText('2 từ');
  await expect(page.getByTestId('paste-options')).toHaveCount(0);
  expect(errors).toEqual([]);
});

test('BlueDoc: a pasted web address can become a link', async ({ page }) => {
  await newDocument(page);
  await page.evaluate(() => navigator.clipboard.writeText('https://bluestar.vn'));
  await page.locator('[data-command="edit.paste"]').first().click();
  const words = page.getByTestId('status-words');
  await expect(words).toHaveText('1 từ');
  await choose(page, /Dạng liên kết/);
  await expect(words).toHaveText('1 từ');
  await page.keyboard.press('Control+z');
  await expect(words).toHaveText('0 từ');
});

test('BlueDoc: clipboard history panel pastes an earlier copy and clears', async ({ page }) => {
  await newDocument(page);
  await page.keyboard.insertText('alpha');
  await expect(page.getByTestId('status-words')).toHaveText('1 từ');
  await page.keyboard.press('Control+a');
  await page.keyboard.press('Control+c');
  // The copy reaches the system clipboard asynchronously: wait before pasting.
  await expect.poll(() => page.evaluate(() => navigator.clipboard.readText())).toBe('alpha');
  await page.keyboard.press('Control+End');
  await page.keyboard.insertText(' beta');
  await expect(page.getByTestId('status-words')).toHaveText('2 từ');
  await page.keyboard.press('Control+a');
  await page.keyboard.press('Control+c');
  // The copy reaches the system clipboard asynchronously: wait before pasting.
  await expect.poll(() => page.evaluate(() => navigator.clipboard.readText())).toBe('alpha beta');
  await page.keyboard.press('Control+End');
  await page.keyboard.insertText(' ');

  await page.locator('[data-command="edit.clipboardPanel"]').first().click();
  const panel = page.getByTestId('clipboard-panel');
  await expect(panel).toBeVisible();
  const items = panel.getByTestId('clipboard-item');
  await expect(items).toHaveCount(2);
  await expect(items.first()).toContainText('alpha beta');
  // The older copy.
  await items
    .nth(1)
    .getByRole('button', { name: /Dán alpha/ })
    .click();
  await expect(page.getByTestId('status-words')).toHaveText('3 từ');
  // Default paste settings live in the same panel.
  await expect(panel.getByTestId('paste-defaults')).toBeVisible();
  await panel.getByTestId('clipboard-clear').click();
  await expect(items).toHaveCount(0);
});

async function goTo(page: Page, ref: string) {
  const box = page.getByLabel('Hộp tên');
  await box.click();
  await box.fill(ref);
  await box.press('Enter');
  await expect(page.getByLabel('Ô đang nhập')).toBeFocused();
}

test('BlueSheet: paste options transpose the pasted cells, one undo step', async ({ page }) => {
  await page.goto('/sheet');
  await expect(page.getByTestId('sheet-canvas')).toHaveAttribute('data-rendered', /\d+/);
  await page.keyboard.type('1');
  await page.keyboard.press('Tab');
  await page.keyboard.type('2');
  await page.keyboard.press('Enter');
  await goTo(page, 'A1:B1');
  await page.keyboard.press('Control+c');
  await goTo(page, 'D3');
  await page.keyboard.press('Control+v');
  await expect(page.getByTestId('paste-options')).toBeVisible();
  await choose(page, /Đổi hàng thành cột/);
  await goTo(page, 'D4');
  await expect(page.getByLabel('Thanh công thức')).toHaveValue('2');
  await goTo(page, 'E3');
  await expect(page.getByLabel('Thanh công thức')).toHaveValue('');
  await page.keyboard.press('Control+z');
  await goTo(page, 'D3');
  await expect(page.getByLabel('Thanh công thức')).toHaveValue('');
});

test('BlueSlide: paste options for copied shapes', async ({ page }) => {
  await page.goto('/slide');
  const chooser = page.waitForEvent('filechooser');
  await page.getByRole('button', { name: 'Mở', exact: true }).click();
  await (await chooser).setFiles(deck);
  await expect(page.getByTestId('slide-overlay')).toBeVisible();
  await page.getByRole('button', { name: '+ Thêm slide' }).click();
  await page.getByRole('tab', { name: 'Chèn' }).click();
  await page.getByRole('button', { name: 'Hộp văn bản' }).click();
  const tb = (await page.getByTestId('selection-box').first().boundingBox())!;
  await page.mouse.dblclick(tb.x + tb.width / 2, tb.y + tb.height / 2);
  await expect(page.getByTestId('slide-text-editor')).toBeFocused();
  await page.keyboard.insertText('Hộp chữ');
  await page.keyboard.press('Escape');
  await page.mouse.click(tb.x + tb.width / 2, tb.y + tb.height / 2);
  await expect(page.getByTestId('selection-box')).toHaveCount(1);
  await page.keyboard.press('Control+c');
  // The copy reaches the system clipboard asynchronously: wait before pasting.
  await expect.poll(() => page.evaluate(() => navigator.clipboard.readText())).toBe('Hộp chữ');
  await page.keyboard.press('Control+v');
  await expect(page.getByTestId('paste-options')).toBeVisible();
  await choose(page, /Giữ nguyên định dạng gốc/);
  await expect(page.getByTestId('selection-box')).toHaveCount(1);
  await page.getByTestId('paste-options').click();
  await expect(
    page.getByTestId('paste-options-menu').getByRole('menuitemradio', { name: /Giữ nguyên/ }),
  ).toHaveAttribute('aria-checked', 'true');
  await page.keyboard.press('Escape');
  // The history panel lists the copy.
  await page.getByRole('tab', { name: 'Trang đầu' }).click();
  await page.locator('[data-command="edit.clipboardPanel"]').first().click();
  await expect(page.getByTestId('clipboard-panel').getByTestId('clipboard-item')).toHaveCount(1);
});
