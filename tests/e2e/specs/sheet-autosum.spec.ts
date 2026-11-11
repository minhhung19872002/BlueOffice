import { expect, test, type Page } from '@playwright/test';

// Auto total of BlueSheet (Σ, Alt+=) through the real UI: the data is typed, the selection is
// made with the Name Box, the keyboard or the mouse, and the formula and the value of the
// exact target cell are read back (formula bar; value from the active cell's announcement).
//
// Reference behaviour (the spreadsheet convention the project follows):
//  * a column of numbers selected → the total goes into the first empty cell BELOW, at once,
//    and the selection grows over it;
//  * a row of numbers → the first empty cell on the RIGHT;
//  * a block → one total under each column;
//  * one empty cell → the formula is proposed in the cell editor with its range selected
//    (numbers above win over numbers on the left); Enter commits, Esc cancels;
//  * one undo step per auto total.

async function goTo(page: Page, ref: string) {
  const box = page.getByLabel('Hộp tên');
  await box.click();
  await box.fill(ref);
  await box.press('Enter');
  await expect(page.getByLabel('Ô đang nhập')).toBeFocused();
}

async function openSheet(page: Page) {
  const errors: string[] = [];
  page.on('pageerror', (e) => errors.push(e.message));
  await page.goto('/sheet');
  await expect(page.getByTestId('sheet-canvas')).toHaveAttribute('data-rendered', /\d+/);
  return errors;
}

/** Types `rows` from `ref` on (Tab between the cells of a row; "" leaves a cell empty). */
async function typeRows(page: Page, ref: string, rows: string[][]) {
  const m = /^([A-Z]+)(\d+)$/.exec(ref)!;
  for (const [i, row] of rows.entries()) {
    await goTo(page, `${m[1]}${Number(m[2]) + i}`);
    for (const v of row) {
      if (v) await page.keyboard.type(v);
      await page.keyboard.press('Tab');
    }
  }
}

/**
 * The formula bar text and the announced value of the cell `ref` (`more`: what the
 * announcement adds about the cell's formatting).
 */
async function expectCell(page: Page, ref: string, formula: string, value: string, more = '') {
  await goTo(page, ref);
  await expect(page.getByLabel('Thanh công thức')).toHaveValue(formula);
  const said = formula.startsWith('=')
    ? `${ref}: ${value}, công thức ${formula}`
    : `${ref}: ${value}`;
  await expect(page.getByTestId('sheet-a11y-live')).toHaveText(said + more);
}

async function expectEmpty(page: Page, ref: string) {
  await goTo(page, ref);
  await expect(page.getByLabel('Thanh công thức')).toHaveValue('');
}

/** The owner's sheet: a header in C1 and 12,5 / 25 / 37,5 in C2:C4 (vi-VN decimal comma). */
async function ownerColumn(page: Page) {
  await typeRows(page, 'C1', [['Số tiền'], ['12,5'], ['25'], ['37,5']]);
}

test('BlueSheet: Alt+= on a selected column puts the total below it (C2:C4 → C5 = 75)', async ({
  page,
}) => {
  const errors = await openSheet(page);
  await ownerColumn(page);
  // Select C2:C4 with the keyboard: Shift+Arrow from C2.
  await goTo(page, 'C2');
  await page.keyboard.press('Shift+ArrowDown');
  await page.keyboard.press('Shift+ArrowDown');
  await expect(page.getByLabel('Hộp tên')).toHaveValue('C2:C4');
  await page.keyboard.press('Alt+=');
  // The selection grows over the total; nothing is written on the right.
  await expect(page.getByLabel('Hộp tên')).toHaveValue('C2:C5');
  // 12,5 + 25 + 37,5 = 75
  await expectCell(page, 'C5', '=SUM(C2:C4)', '75');
  await expectEmpty(page, 'D2');
  await expectEmpty(page, 'D4');
  // One undo step takes the total back; redo brings it again.
  await page.keyboard.press('Control+z');
  await expectEmpty(page, 'C5');
  await expectCell(page, 'C4', '37,5', '37,5');
  await page.keyboard.press('Control+y');
  await expectCell(page, 'C5', '=SUM(C2:C4)', '75');
  expect(errors).toEqual([]);
});

test('BlueSheet: the Σ button totals a column selected with the mouse', async ({ page }) => {
  // A wide window: the Editing group at the right end of the Home tab shows its buttons (in a
  // narrow one the group folds into its drop-down, like every group of the ribbon).
  await page.setViewportSize({ width: 1920, height: 1000 });
  const errors = await openSheet(page);
  await ownerColumn(page);
  // Drag from C2 to C4 on the grid.
  await goTo(page, 'C2');
  const canvas = page.getByTestId('sheet-canvas');
  const box = (await canvas.boundingBox())!;
  const cell = async (ref: string) => {
    await goTo(page, ref);
    const active = page.getByLabel('Ô đang nhập');
    const b = (await active.boundingBox())!;
    return { x: b.x + b.width / 2, y: b.y + b.height / 2 };
  };
  const from = await cell('C2');
  const to = await cell('C4');
  expect(from.x).toBeGreaterThan(box.x);
  await page.mouse.move(from.x, from.y);
  await page.mouse.down();
  await page.mouse.move(to.x, to.y, { steps: 4 });
  await page.mouse.up();
  await expect(page.getByLabel('Hộp tên')).toHaveValue('C2:C4');
  // Home tab, Editing group.
  const sigma = page.locator('[data-command="formula.autoSum"]');
  await expect(sigma).toBeVisible();
  await expect(sigma).toHaveAttribute('aria-keyshortcuts', 'Alt+=');
  await sigma.click();
  await expect(page.getByLabel('Hộp tên')).toHaveValue('C2:C5');
  await expectCell(page, 'C5', '=SUM(C2:C4)', '75');
  // The same button in the Formulas tab, with its label.
  await page.getByRole('tab', { name: 'Công thức' }).click();
  await expect(page.locator('[data-command="formula.autoSum"]')).toContainText('Tự tính tổng');
  expect(errors).toEqual([]);
});

test('BlueSheet: Alt+= on a row puts the total on its right, on a block under each column', async ({
  page,
}) => {
  const errors = await openSheet(page);
  // A1:C3 = 1 2 3 / 4 5 6 / 7 8 9.
  await typeRows(page, 'A1', [
    ['1', '2', '3'],
    ['4', '5', '6'],
    ['7', '8', '9'],
  ]);
  // One row: A1:C1 → D1 = 1 + 2 + 3.
  await goTo(page, 'A1:C1');
  await page.keyboard.press('Alt+=');
  await expect(page.getByLabel('Hộp tên')).toHaveValue('A1:D1');
  await expectCell(page, 'D1', '=SUM(A1:C1)', '6');
  await expectEmpty(page, 'A4');
  await page.keyboard.press('Control+z');
  await expectEmpty(page, 'D1');
  // The block A1:C3 → row 4: 1+4+7, 2+5+8, 3+6+9; nothing in column D.
  await goTo(page, 'A1:C3');
  await page.keyboard.press('Alt+=');
  await expect(page.getByLabel('Hộp tên')).toHaveValue('A1:C4');
  await expectCell(page, 'A4', '=SUM(A1:A3)', '12');
  await expectCell(page, 'B4', '=SUM(B1:B3)', '15');
  await expectCell(page, 'C4', '=SUM(C1:C3)', '18');
  await expectEmpty(page, 'D1');
  // One undo step removes the three totals.
  await page.keyboard.press('Control+z');
  await expectEmpty(page, 'A4');
  await expectEmpty(page, 'C4');
  // The block with an empty row and column selected too (A1:D4): both are filled, and the
  // corner gets the grand total 45.
  await goTo(page, 'A1:D4');
  await page.keyboard.press('Alt+=');
  await expectCell(page, 'D2', '=SUM(A2:C2)', '15');
  await expectCell(page, 'B4', '=SUM(B1:B3)', '15');
  await expectCell(page, 'D4', '=SUM(D1:D3)', '45');
  expect(errors).toEqual([]);
});

test('BlueSheet: Alt+= in one empty cell proposes the formula; Enter commits, Esc cancels', async ({
  page,
}) => {
  const errors = await openSheet(page);
  await ownerColumn(page);
  const editor = page.getByLabel('Ô đang nhập');
  // Under the column: the numbers above, without the header in C1.
  await goTo(page, 'C5');
  await page.keyboard.press('Alt+=');
  await expect(editor).toHaveValue('=SUM(C2:C4)');
  await expect(page.getByTestId('sheet-status')).toHaveText('Sửa');
  // The proposed range is selected inside the formula so that it can be adjusted.
  expect(
    await editor.evaluate((el: HTMLInputElement) =>
      el.value.slice(el.selectionStart ?? 0, el.selectionEnd ?? 0),
    ),
  ).toBe('C2:C4');
  // Esc: nothing is written.
  await page.keyboard.press('Escape');
  await expectEmpty(page, 'C5');
  // Again, adjusting the range by typing over the selection, then Enter: 25 + 37,5 = 62,5.
  await goTo(page, 'C5');
  await page.keyboard.press('Alt+=');
  await expect(editor).toHaveValue('=SUM(C2:C4)');
  await page.keyboard.type('C3:C4');
  await page.keyboard.press('Enter');
  await expectCell(page, 'C5', '=SUM(C3:C4)', '62,5');
  // Alt+= on the total written before opens it again with its range selected.
  await goTo(page, 'C5');
  await page.keyboard.press('Alt+=');
  await expect(editor).toHaveValue('=SUM(C3:C4)');
  expect(
    await editor.evaluate((el: HTMLInputElement) =>
      el.value.slice(el.selectionStart ?? 0, el.selectionEnd ?? 0),
    ),
  ).toBe('C3:C4');
  await page.keyboard.press('Escape');

  // Right of a row with nothing above: the numbers on the left (F8:H8 → I8).
  await typeRows(page, 'F8', [['2', '4', '6']]);
  await goTo(page, 'I8');
  await page.keyboard.press('Alt+=');
  await expect(editor).toHaveValue('=SUM(F8:H8)');
  await page.keyboard.press('Enter');
  await expectCell(page, 'I8', '=SUM(F8:H8)', '12');
  // One undo step for the committed proposal.
  await page.keyboard.press('Control+z');
  await expectEmpty(page, 'I8');

  // Nothing around: an empty call with the caret between the parentheses.
  await goTo(page, 'K20');
  await page.keyboard.press('Alt+=');
  await expect(editor).toHaveValue('=SUM()');
  expect(
    await editor.evaluate((el: HTMLInputElement) => [el.selectionStart, el.selectionEnd]),
  ).toEqual([5, 5]);
  await page.keyboard.type('C2:C3');
  await page.keyboard.press('Enter');
  // 12,5 + 25
  await expectCell(page, 'K20', '=SUM(C2:C3)', '37,5');
  expect(errors).toEqual([]);
});

test('BlueSheet: the Σ menu averages, and More functions starts a formula', async ({ page }) => {
  const errors = await openSheet(page);
  await ownerColumn(page);
  // The function library is the first group of the Formulas tab.
  await page.getByRole('tab', { name: 'Công thức' }).click();
  await goTo(page, 'C2:C4');
  await page.locator('[data-command="formula.autoSumMenu"]').click();
  const menu = page.getByTestId('menu-autoSum');
  await expect(menu.locator('[data-menu]')).toHaveText([
    'Tổng',
    'Trung bình',
    'Đếm số',
    'Lớn nhất',
    'Nhỏ nhất',
    'Hàm khác…',
  ]);
  await menu.locator('[data-menu="average"]').click();
  await expect(page.getByLabel('Hộp tên')).toHaveValue('C2:C5');
  // (12,5 + 25 + 37,5) / 3 = 25
  await expectCell(page, 'C5', '=AVERAGE(C2:C4)', '25');
  await page.keyboard.press('Control+z');
  await expectEmpty(page, 'C5');
  // Count numbers, largest and smallest of the same cells.
  for (const [id, formula, value] of [
    ['count', '=COUNT(C2:C4)', '3'],
    ['max', '=MAX(C2:C4)', '37,5'],
    ['min', '=MIN(C2:C4)', '12,5'],
  ] as const) {
    await goTo(page, 'C2:C4');
    await page.locator('[data-command="formula.autoSumMenu"]').click();
    await page.getByTestId('menu-autoSum').locator(`[data-menu="${id}"]`).click();
    await expectCell(page, 'C5', formula, value);
    await page.keyboard.press('Control+z');
    await expectEmpty(page, 'C5');
  }
  // More functions…: the list of the engine's functions; the pick starts the formula.
  await goTo(page, 'E2');
  await page.locator('[data-command="formula.autoSumMenu"]').click();
  await page.getByTestId('menu-autoSum').locator('[data-menu="more"]').click();
  const dialog = page.getByTestId('insert-function-dialog');
  await expect(dialog).toBeVisible();
  await page.getByTestId('insert-function-search').fill('MEDIAN');
  await expect(dialog.locator('[data-function]')).toHaveText(['MEDIAN']);
  await page.getByTestId('insert-function-ok').click();
  await expect(dialog).toHaveCount(0);
  const editor = page.getByLabel('Ô đang nhập');
  await expect(editor).toHaveValue('=MEDIAN()');
  await expect(editor).toBeFocused();
  await page.keyboard.type('C2:C4');
  await page.keyboard.press('Enter');
  await expectCell(page, 'E2', '=MEDIAN(C2:C4)', '25');
  expect(errors).toEqual([]);
});

test('BlueSheet: Quick Analysis totals say where they go; one column only offers the row below', async ({
  page,
}) => {
  const errors = await openSheet(page);
  await ownerColumn(page);
  await goTo(page, 'C2:C4');
  await page.getByTestId('quick-analysis').click();
  await page.getByTestId('qa-tab-totals').click();
  // The row below the data first, then the column on the right — each named for its place.
  await expect(page.getByTestId('qa-totals-rows')).toContainText('Thêm hàng tổng bên dưới dữ liệu');
  await expect(page.getByTestId('qa-totals-cols')).toContainText('Thêm cột tổng bên phải dữ liệu');
  const below = page.getByTestId('qa-total-rows-sum');
  const right = page.getByTestId('qa-total-cols-sum');
  await expect(below).toHaveAttribute('aria-label', 'Tổng (thêm hàng bên dưới)');
  await expect(right).toHaveAttribute('aria-label', 'Tổng (thêm cột bên phải)');
  // One column selected: a column of totals on the right would only repeat the values.
  await expect(right).toHaveAttribute('aria-disabled', 'true');
  await expect(right).toHaveAttribute('title', /chỉ có một cột/);
  await right.click({ force: true });
  await expect(page.getByTestId('quick-analysis-panel')).toBeVisible();
  // Hover: the formula and its cell before anything is written.
  await below.hover();
  await expect(page.getByTestId('qa-hint')).toHaveText(
    'Tổng (thêm hàng bên dưới) — C5: =SUM(C2:C4)',
  );
  await below.click();
  await expect(page.getByTestId('quick-analysis-panel')).toHaveCount(0);
  // The row of totals is written in bold.
  await expectCell(page, 'C5', '=SUM(C2:C4)', '75', ', đậm');
  await expectEmpty(page, 'D2');
  await expectEmpty(page, 'D3');
  await expectEmpty(page, 'D4');
  expect(errors).toEqual([]);
});

test('BlueSheet: the status bar summarises two or more filled cells', async ({ page }) => {
  const errors = await openSheet(page);
  await ownerColumn(page);
  await goTo(page, 'C2:C4');
  await expect(page.getByTestId('status-average')).toHaveText('Trung bình: 25');
  await expect(page.getByTestId('status-count')).toHaveText('Số lượng: 3');
  await expect(page.getByTestId('status-sum')).toHaveText('Tổng: 75');
  // Text counts as a filled cell but not as a number.
  await goTo(page, 'C1:C4');
  await expect(page.getByTestId('status-count')).toHaveText('Số lượng: 4');
  await expect(page.getByTestId('status-sum')).toHaveText('Tổng: 75');
  // One filled cell among empty ones: nothing to summarise.
  await goTo(page, 'C4:C6');
  await expect(page.getByLabel('Hộp tên')).toHaveValue('C4:C6');
  await expect(page.getByTestId('status-sum')).toHaveCount(0);
  await expect(page.getByTestId('status-count')).toHaveCount(0);
  expect(errors).toEqual([]);
});
