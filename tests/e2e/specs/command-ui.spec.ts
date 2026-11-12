import { expect, test, type Page } from '@playwright/test';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

// CORE-UI-003 / CORE-UI-004: context menus, mini toolbar, KeyTips, custom shortcuts, F4.
const here = dirname(fileURLToPath(import.meta.url));
const deck = join(here, '..', '..', 'corpus', 'slide', 'slide-text-bullets.pptx');

const cmd = (page: Page, id: string) => page.locator(`[data-command="${id}"]`);
const page1 = (page: Page) => page.locator('.bo-page[data-page="1"]');

async function newDocument(page: Page) {
  await page.goto('/doc');
  await page.getByTestId('new-document').click();
  await expect(page.getByTestId('caret')).toBeVisible();
}

/** A point on the first text line of page 1. */
async function firstLine(page: Page, fx = 0.3) {
  const box = (await page1(page).boundingBox())!;
  return { x: box.x + box.width * fx, y: box.y + 100 };
}

test('doc: right click menus follow what is under the pointer; Shift+F10 opens one', async ({
  page,
}) => {
  const errors: string[] = [];
  page.on('pageerror', (e) => errors.push(e.message));
  await newDocument(page);
  await page.keyboard.insertText('Xin chào bạn');
  const line = await firstLine(page, 0.15);
  await page.mouse.click(line.x, line.y, { button: 'right' });
  const text = page.getByTestId('context-menu-text');
  await expect(text).toBeVisible();
  await expect(text).toHaveAttribute('role', 'menu');
  await expect(text.locator('[data-menu-command="edit.copy"]')).toBeVisible();
  await expect(text.locator('[data-menu-command="table.insertRowAbove"]')).toHaveCount(0);
  await page.keyboard.press('Escape');
  await expect(text).toHaveCount(0);

  // In a table: the table commands, enabled; running one closes the menu.
  await page.keyboard.press('End');
  await page.keyboard.press('Enter');
  await page.getByRole('tab', { name: 'Chèn' }).click();
  await cmd(page, 'insert.table').click();
  await expect(page.getByTestId('caret')).toBeVisible();
  const cell = (await page.getByTestId('caret').boundingBox())!;
  await page.mouse.click(cell.x + 4, cell.y + cell.height / 2, { button: 'right' });
  const table = page.getByTestId('context-menu-table');
  await expect(table).toBeVisible();
  const below = table.locator('[data-menu-command="table.insertRowBelow"]');
  await expect(below).not.toHaveAttribute('aria-disabled', 'true');
  await below.click();
  await expect(table).toHaveCount(0);

  // Keyboard: Shift+F10 at the caret, arrows move, Escape closes.
  await page.mouse.move(5, 5);
  await page.keyboard.press('Shift+F10');
  await expect(table).toBeVisible();
  const cut = table.locator('[data-menu-command="edit.cut"]');
  await expect(cut).not.toHaveAttribute('aria-disabled', 'true');
  await expect(cut).toHaveAttribute('data-active', 'true');
  await page.keyboard.press('ArrowDown');
  await expect(table.locator('[data-menu-command="edit.copy"]')).toHaveAttribute(
    'data-active',
    'true',
  );
  await page.keyboard.press('Escape');
  await expect(table).toHaveCount(0);
  expect(errors).toEqual([]);
});

test('doc: the mini toolbar formats a word selected with the mouse and fades away', async ({
  page,
}) => {
  await newDocument(page);
  await page.keyboard.insertText('Một hai ba bốn năm');
  const at = await firstLine(page, 0.12);
  await page.mouse.dblclick(at.x, at.y);
  const mini = page.getByTestId('mini-toolbar');
  await expect(mini).toBeVisible();
  await expect(mini.getByTestId('mini-font-size')).toBeVisible();
  await mini.locator('[data-mini-command="format.bold"]').click();
  await expect(cmd(page, 'format.bold')).toHaveAttribute('aria-pressed', 'true');
  // Far from the bar: it goes.
  const b = (await mini.boundingBox())!;
  await page.mouse.move(b.x + b.width / 2, b.y + b.height / 2);
  await page.mouse.move(b.x + b.width / 2, b.y + 400, { steps: 8 });
  await expect(mini).toHaveCount(0);
});

test('doc: KeyTips — Alt, the Home tab letter, then 1 makes the selection bold', async ({
  page,
}) => {
  await newDocument(page);
  await page.keyboard.insertText('Định dạng bằng phím');
  await page.keyboard.press('Control+a');
  await page.keyboard.press('Alt');
  const tips = page.getByTestId('keytips');
  await expect(tips).toHaveAttribute('data-level', 'tabs');
  await expect(tips.locator('[data-keytip-for="file"]')).toHaveText('F');
  const home = await tips.locator('[data-keytip-for="tab:home"]').getAttribute('data-keytip');
  expect(home).toBeTruthy();
  await page.keyboard.press(home!.toLowerCase());
  await expect(tips).toHaveAttribute('data-level', 'items');
  await expect(tips.locator('[data-keytip-for="format.bold"]')).toHaveText('1');
  // Codes of one tab are unique.
  const codes = await tips
    .locator('[data-keytip]')
    .evaluateAll((els) => els.map((e) => e.getAttribute('data-keytip')));
  expect(new Set(codes).size).toBe(codes.length);
  await page.keyboard.press('1');
  await expect(tips).toHaveCount(0);
  await expect(cmd(page, 'format.bold')).toHaveAttribute('aria-pressed', 'true');
  // Escape steps back: items → tabs → off.
  await page.keyboard.press('Alt');
  await page.keyboard.press(home!.toLowerCase());
  await expect(tips).toHaveAttribute('data-level', 'items');
  await page.keyboard.press('Escape');
  await expect(tips).toHaveAttribute('data-level', 'tabs');
  await page.keyboard.press('Escape');
  await expect(tips).toHaveCount(0);
});

test('doc: F4 repeats the last formatting on another selection', async ({ page }) => {
  await newDocument(page);
  await page.keyboard.insertText('abc def');
  await page.keyboard.press('Control+Shift+ArrowLeft');
  await page.keyboard.press('Control+i');
  await expect(cmd(page, 'format.italic')).toHaveAttribute('aria-pressed', 'true');
  // Moving the caret and selecting do not forget the last action.
  await page.keyboard.press('Home');
  await page.keyboard.press('Shift+ArrowRight');
  await page.keyboard.press('Shift+ArrowRight');
  await page.keyboard.press('Shift+ArrowRight');
  await expect(cmd(page, 'format.italic')).not.toHaveAttribute('aria-pressed', 'true');
  await page.keyboard.press('F4');
  await expect(cmd(page, 'format.italic')).toHaveAttribute('aria-pressed', 'true');
});

test('doc: a custom shortcut set in Options works and is kept after reload', async ({ page }) => {
  await newDocument(page);
  await page.getByTestId('file-tab').click();
  await page
    .getByRole('navigation', { name: 'Tệp' })
    .getByRole('button', { name: 'Tùy chọn', exact: true })
    .click();
  await page.getByRole('button', { name: 'Tùy biến phím tắt…' }).click();
  const dialog = page.getByTestId('shortcuts-dialog');
  await expect(dialog).toBeVisible();
  await dialog.getByTestId('shortcuts-search').fill('Đậm');
  const row = dialog.locator('[data-shortcut-row="format.bold"]');
  await expect(row.getByTestId('shortcut-value')).toHaveText('Ctrl+B');
  // A combo already used is reported with its command.
  await row.getByTestId('shortcut-change').click();
  await page.keyboard.press('Control+i');
  await expect(row.getByTestId('shortcut-conflict')).toContainText('Nghiêng');
  await page.keyboard.press('Control+Alt+k');
  await expect(row.getByTestId('shortcut-conflict')).toHaveCount(0);
  await row.getByTestId('shortcut-assign').click();
  await expect(row.getByTestId('shortcut-value')).toHaveText('Ctrl+Alt+K');
  await page.keyboard.press('Escape');
  await expect(dialog).toHaveCount(0);

  await page.keyboard.insertText('abc');
  await page.keyboard.press('Shift+Home');
  await page.keyboard.press('Control+Alt+k');
  await expect(cmd(page, 'format.bold')).toHaveAttribute('aria-pressed', 'true');

  await page.reload();
  await page.getByTestId('new-document').click();
  await expect(page.getByTestId('caret')).toBeVisible();
  await page.keyboard.insertText('xyz');
  await page.keyboard.press('Shift+Home');
  await page.keyboard.press('Control+Alt+k');
  await expect(cmd(page, 'format.bold')).toHaveAttribute('aria-pressed', 'true');
});

test('sheet: F4 cycles the reference; row header and sheet tab menus', async ({ page }) => {
  const errors: string[] = [];
  page.on('pageerror', (e) => errors.push(e.message));
  await page.goto('/sheet');
  await expect(page.getByRole('tab', { name: 'Sheet1' })).toBeVisible();
  await expect(page.getByTestId('sheet-canvas')).toHaveAttribute('data-rendered', /\d+/);
  const bar = page.getByLabel('Thanh công thức');
  await page.keyboard.type('=A1+B2');
  await page.keyboard.press('F4');
  await expect(bar).toHaveValue('=A1+$B$2');
  await page.keyboard.press('F4');
  await expect(bar).toHaveValue('=A1+B$2');
  await page.keyboard.press('F4');
  await expect(bar).toHaveValue('=A1+$B2');
  await page.keyboard.press('Escape');

  // Right click a row header: row commands; the row gets selected.
  const canvas = (await page.getByTestId('sheet-canvas').boundingBox())!;
  await page.mouse.click(canvas.x + 10, canvas.y + 70, { button: 'right' });
  const rowMenu = page.getByTestId('context-menu-rowHeader');
  await expect(rowMenu).toBeVisible();
  await expect(rowMenu.locator('[data-menu-command="cells.insertRow"]')).toBeVisible();
  await expect(rowMenu.locator('[data-menu-command="cells.insertCol"]')).toHaveCount(0);
  await page.keyboard.press('Escape');

  // A cell: cell commands.
  await page.mouse.click(canvas.x + 200, canvas.y + 120, { button: 'right' });
  await expect(page.getByTestId('context-menu-cell')).toBeVisible();
  await page.keyboard.press('Escape');

  // The sheet tab: sheet commands and the tab colour.
  await page.getByRole('tab', { name: 'Sheet1' }).click({ button: 'right' });
  const tabMenu = page.getByTestId('context-menu-sheetTab');
  await expect(tabMenu.locator('[data-menu-command="sheet.rename"]')).toBeVisible();
  await tabMenu.locator('[data-menu-command="sheet.tabColor"]').click();
  await expect(tabMenu).toHaveCount(0);
  expect(errors).toEqual([]);
});

test('slide: thumbnail and shape menus', async ({ page }) => {
  await page.goto('/slide');
  const chooser = page.waitForEvent('filechooser');
  await page.getByRole('button', { name: 'Mở', exact: true }).click();
  await (await chooser).setFiles(deck);
  await expect(page.getByTestId('slide-status')).toHaveText(/^Slide 1 \/ \d+$/);
  const total = Number((await page.getByTestId('slide-status').textContent())!.split('/')[1]);

  await page.getByTestId('thumb-1').click({ button: 'right' });
  const thumbMenu = page.getByTestId('context-menu-thumbnail');
  await expect(thumbMenu).toBeVisible();
  await thumbMenu.locator('[data-menu-command="slide.duplicate"]').click();
  await expect(page.getByTestId('slide-status')).toHaveText(new RegExp(`/ ${total + 1}$`));

  // Right click on the title shape selects it and offers the shape commands.
  const o = (await page.getByTestId('slide-overlay').boundingBox())!;
  await page.mouse.click(o.x + o.width * 0.48, o.y + o.height * 0.3, { button: 'right' });
  const shape = page.getByTestId('context-menu-shape');
  await expect(shape).toBeVisible();
  await expect(shape.locator('[data-menu-command="arrange.front"]')).toBeVisible();
  await expect(page.getByTestId('selection-box')).toHaveCount(1);
  await page.keyboard.press('Escape');
  await expect(shape).toHaveCount(0);
});

test('slide: the mini toolbar has font name / size boxes; spelling uses the shared menu', async ({
  page,
}) => {
  const errors: string[] = [];
  page.on('pageerror', (e) => errors.push(e.message));
  await page.goto('/slide');
  const chooser = page.waitForEvent('filechooser');
  await page.getByRole('button', { name: 'Mở', exact: true }).click();
  await (await chooser).setFiles(deck);
  await expect(page.getByTestId('slide-status')).toHaveText(/^Slide 1 \/ \d+$/);
  await page.getByRole('button', { name: '+ Thêm slide' }).click();
  await page.getByRole('tab', { name: 'Chèn' }).click();
  await page.getByRole('button', { name: 'Hộp văn bản' }).click();
  await page.keyboard.press('Enter');
  const tb = (await page.getByTestId('selection-box').first().boundingBox())!;
  await page.mouse.dblclick(tb.x + tb.width / 2, tb.y + tb.height / 2);
  const editor = page.getByTestId('slide-text-editor');
  await expect(editor).toBeFocused();
  await page.keyboard.insertText('Xin chaò the documnet');

  // Corrections head the shared text menu (role=menu, arrows), with the text commands below.
  const marks = page.getByTestId('slide-spell-mark');
  await expect(marks).toHaveCount(2, { timeout: 15_000 });
  const m = (await marks.first().boundingBox())!;
  await page.mouse.click(m.x + m.width / 2, m.y - 4, { button: 'right' });
  const menu = page.getByTestId('spell-menu');
  await expect(menu).toHaveAttribute('role', 'menu');
  await expect(menu.locator('[data-menu-command="edit.copy"]')).toBeVisible();
  // Keyboard: Home goes to the first correction ("chào"), Enter runs it (the pointer may rest
  // on another line of the menu).
  const pick = menu.locator('[data-menu-command="spell.pick.chào"]');
  await page.keyboard.press('Home');
  await expect(pick).toHaveAttribute('data-active', 'true');
  await page.keyboard.press('ArrowDown');
  await expect(pick).not.toHaveAttribute('data-active', 'true');
  await page.keyboard.press('Home');
  await expect(pick).toBeFocused();
  await page.keyboard.press('Enter');
  await expect(editor).toHaveValue('Xin chào the documnet', { timeout: 15_000 });
  await expect(menu).toHaveCount(0);

  // Select the text with the mouse: the floating bar shows the font name and size boxes.
  const box = tb;
  await page.mouse.move(box.x + 6, box.y + box.height / 2);
  await page.mouse.down();
  await page.mouse.move(box.x + box.width - 6, box.y + box.height / 2, { steps: 6 });
  await page.mouse.up();
  const mini = page.getByTestId('mini-toolbar');
  await expect(mini).toBeVisible();
  await expect(mini.getByTestId('mini-font-family')).toBeVisible();
  await mini.getByTestId('mini-font-size').selectOption('32');
  await expect(page.getByTestId('slide-font-size')).toHaveValue('32');
  expect(errors).toEqual([]);
});
