import { expect, test, type Page } from '@playwright/test';

// BlueSheet "data" behaviour against the spreadsheet conventions people know from the
// desktop spreadsheet they use every day (docs/parity/office-behaviour-sheet-data.md): fill
// handle and its options, Ctrl+D / Ctrl+R, quick sort (region, header guess, "expand the
// selection?"), AutoFilter from the keyboard, formulas typed with a vi-VN regional setting
// (";" separators, decimal commas), refused syntax errors, defined names and the results of
// common functions on typed data. Everything goes through the real UI: keys on the grid,
// the fill handle dragged with the mouse, ribbon buttons, dialogs. Expected values are worked
// out by hand from the data typed in each test.

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

/** The announced value of `ref` (what the cell shows). */
async function expectValue(page: Page, ref: string, value: string) {
  await goTo(page, ref);
  await expect(page.getByTestId('sheet-a11y-live')).toHaveText(
    new RegExp(`^${ref}: ${value.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')}(,|$)`),
  );
}

async function expectFormula(page: Page, ref: string, formula: string) {
  await goTo(page, ref);
  await expect(page.getByLabel('Thanh công thức')).toHaveValue(formula);
}

async function expectEmpty(page: Page, ref: string) {
  await goTo(page, ref);
  await expect(page.getByLabel('Thanh công thức')).toHaveValue('');
}

/** Centre of cell `ref` on the screen (the cell editor sits on the active cell). */
async function centre(page: Page, ref: string) {
  await goTo(page, ref);
  const b = (await page.getByLabel('Ô đang nhập').boundingBox())!;
  return { x: b.x + b.width / 2, y: b.y + b.height / 2 };
}

/** Drags the fill handle of the selection `sel` to cell `to` (Ctrl held when `ctrl`). */
async function dragFill(page: Page, sel: string, to: string, ctrl = false) {
  const dest = await centre(page, to);
  await goTo(page, sel);
  const h = (await page.getByTestId('fill-handle').boundingBox())!;
  await page.mouse.move(h.x + h.width / 2, h.y + h.height / 2);
  await page.mouse.down();
  await page.mouse.move(dest.x, dest.y, { steps: 6 });
  if (ctrl) await page.keyboard.down('Control');
  await page.mouse.up();
  if (ctrl) await page.keyboard.up('Control');
}

test.describe('Trang đầu › Điền (fill handle, Ctrl+D / Ctrl+R)', () => {
  test('dragging the fill handle continues series and copies a single number', async ({ page }) => {
    const errors = await openSheet(page);
    await typeRows(page, 'A1', [
      ['1', '5', 'Thứ Hai', 'Tháng 1', 'Quý 3', '30/12/2026', 'Sản phẩm 9', '=A1*2'],
      ['2'],
    ]);
    // Two numbers: their step continues (1, 2 → 3, 4, 5).
    await dragFill(page, 'A1:A2', 'A5');
    await expect(page.getByLabel('Hộp tên')).toHaveValue('A1:A5');
    await expectValue(page, 'A3', '3');
    await expectValue(page, 'A5', '5');
    // One number is copied …
    await dragFill(page, 'B1', 'B3');
    await expectValue(page, 'B3', '5');
    // … and counts up when Ctrl is held on release.
    await dragFill(page, 'B1', 'B3', true);
    await expectValue(page, 'B2', '6');
    await expectValue(page, 'B3', '7');
    // Lists: weekdays, months, quarters (cyclic: Quý 3, Quý 4, Quý 1).
    await dragFill(page, 'C1:H1', 'H3');
    await expectValue(page, 'C3', 'Thứ Tư');
    await expectValue(page, 'D3', 'Tháng 3');
    await expectValue(page, 'E2', 'Quý 4');
    await expectValue(page, 'E3', 'Quý 1');
    // A date steps by day across the year end; a text with a number counts up.
    await expectValue(page, 'F3', '01/01/2027');
    await expectValue(page, 'G3', 'Sản phẩm 11');
    // A formula's relative references follow: H3 = A3*2 = 6.
    await expectFormula(page, 'H3', '=A3*2');
    await expectValue(page, 'H3', '6');
    // One undo step takes the whole fill back.
    await page.keyboard.press('Control+z');
    await expectEmpty(page, 'C2');
    await expectEmpty(page, 'H3');
    expect(errors).toEqual([]);
  });

  test('Auto Fill Options switch the last fill between copy, series and formats', async ({
    page,
  }) => {
    const errors = await openSheet(page);
    await typeRows(page, 'A1', [['1'], ['2']]);
    await dragFill(page, 'A1:A2', 'A6');
    const button = page.getByTestId('autofill-options');
    await expect(button).toBeVisible();
    await button.click();
    const menu = page.getByTestId('autofill-options-menu');
    await expect(menu.locator('[data-paste-option]')).toHaveText([
      /Sao chép ô/,
      /Điền chuỗi/,
      /Chỉ điền định dạng/,
      /Điền không kèm định dạng/,
    ]);
    // Copy cells: 1, 2, 1, 2, 1, 2.
    await menu.locator('[data-paste-option="copy"]').click();
    await expectValue(page, 'A3', '1');
    await expectValue(page, 'A6', '2');
    // Undo once leaves the two typed numbers only (the switch replaced the fill).
    await page.keyboard.press('Control+z');
    await expectEmpty(page, 'A3');
    await expectValue(page, 'A2', '2');
    // Redo brings the copy back.
    await page.keyboard.press('Control+y');
    await expectValue(page, 'A6', '2');
    expect(errors).toEqual([]);
  });

  test('double-clicking the fill handle fills down as far as the column next to it', async ({
    page,
  }) => {
    const errors = await openSheet(page);
    await typeRows(page, 'A1', [['3', '=A1*10'], ['4'], ['5'], ['6']]);
    await goTo(page, 'B1');
    const h = (await page.getByTestId('fill-handle').boundingBox())!;
    await page.mouse.dblclick(h.x + h.width / 2, h.y + h.height / 2);
    await expect(page.getByLabel('Hộp tên')).toHaveValue('B1:B4');
    await expectValue(page, 'B4', '60');
    await expectEmpty(page, 'B5');
    expect(errors).toEqual([]);
  });

  test('Ctrl+D / Ctrl+R copy the first cell, or the cell above / on the left of one cell', async ({
    page,
  }) => {
    const errors = await openSheet(page);
    await typeRows(page, 'A1', [['7', '=A1+1']]);
    // A range: the first row is copied down.
    await goTo(page, 'A1:A3');
    await page.keyboard.press('Control+d');
    await expectValue(page, 'A3', '7');
    // One cell: the cell above is copied into it, references adjusted (B2 = A2+1).
    await goTo(page, 'B2');
    await page.keyboard.press('Control+d');
    await expectFormula(page, 'B2', '=A2+1');
    await expectValue(page, 'B2', '8');
    // Ctrl+R in one cell copies the one on its left (C1 = B1+1 = 9).
    await goTo(page, 'C1');
    await page.keyboard.press('Control+r');
    await expectFormula(page, 'C1', '=B1+1');
    await expectValue(page, 'C1', '9');
    // One undo step each.
    await page.keyboard.press('Control+z');
    await expectEmpty(page, 'C1');
    expect(errors).toEqual([]);
  });

  test('Fill › Series: linear step with a stop value, working days', async ({ page }) => {
    // Wide window: the Editing group shows its buttons.
    await page.setViewportSize({ width: 1920, height: 1000 });
    const errors = await openSheet(page);
    // 06/11/2026 is a Friday.
    await typeRows(page, 'A1', [['5', '06/11/2026']]);
    await goTo(page, 'A1:A8');
    await page.locator('[data-command="edit.fillSeries"]').click();
    const dialog = page.getByTestId('series-dialog');
    // A column selected: series in columns, linear, step 1.
    await expect(page.getByTestId('series-in-columns')).toBeChecked();
    await expect(page.getByTestId('series-type-linear')).toBeChecked();
    await page.getByTestId('series-step').fill('2,5');
    await page.getByTestId('series-stop').fill('15');
    await page.getByTestId('series-ok').click();
    await expect(dialog).toHaveCount(0);
    // 5; 7,5; 10; 12,5; 15 — then the stop value is reached.
    await expectValue(page, 'A2', '7,5');
    await expectValue(page, 'A5', '15');
    await expectEmpty(page, 'A6');
    // Dates: the type is proposed; working days skip the weekend (Mon 09, Tue 10).
    await goTo(page, 'B1:B3');
    await page.locator('[data-command="edit.fillSeries"]').click();
    await expect(page.getByTestId('series-type-date')).toBeChecked();
    await page.getByTestId('series-unit-weekday').check();
    await page.getByTestId('series-ok').click();
    await expectValue(page, 'B2', '09/11/2026');
    await expectValue(page, 'B3', '10/11/2026');
    // One undo step.
    await page.keyboard.press('Control+z');
    await expectEmpty(page, 'B2');
    expect(errors).toEqual([]);
  });
});

/** The class list used by the sort and filter tests (B2:C7, header in row 2). */
async function classList(page: Page) {
  await typeRows(page, 'B2', [
    ['Tên', 'Điểm'],
    ['Lan', '7'],
    ['An', '9'],
    ['Bình', '5'],
    ['Châu', '8'],
    ['Dũng', '6'],
  ]);
}

test.describe('Dữ liệu › Sắp xếp', () => {
  test('A→Z in one cell sorts its block and keeps the header row', async ({ page }) => {
    const errors = await openSheet(page);
    await classList(page);
    await page.getByRole('tab', { name: 'Dữ liệu' }).click();
    // Active cell in the Điểm column: rows sorted by score, names moving with them.
    await goTo(page, 'C4');
    await page.locator('[data-command="data.sortAsc"]').click();
    await expectValue(page, 'B2', 'Tên');
    await expectValue(page, 'B3', 'Bình');
    await expectValue(page, 'C3', '5');
    await expectValue(page, 'B7', 'An');
    await expectValue(page, 'C7', '9');
    // Z→A on the names.
    await goTo(page, 'B5');
    await page.locator('[data-command="data.sortDesc"]').click();
    await expectValue(page, 'B3', 'Lan');
    await expectValue(page, 'C3', '7');
    await expectValue(page, 'B7', 'An');
    // Numbers only: no header, every row sorts.
    await typeRows(page, 'F2', [['3'], ['1'], ['2']]);
    await goTo(page, 'F3');
    await page.locator('[data-command="data.sortAsc"]').click();
    await expectValue(page, 'F2', '1');
    await expectValue(page, 'F4', '3');
    expect(errors).toEqual([]);
  });

  test('one column of a wider list asks to expand the selection first', async ({ page }) => {
    const errors = await openSheet(page);
    await classList(page);
    await page.getByRole('tab', { name: 'Dữ liệu' }).click();
    await goTo(page, 'B3:B7');
    await page.locator('[data-command="data.sortAsc"]').click();
    const warning = page.getByTestId('sort-warning');
    await expect(warning).toBeVisible();
    // Expand the selection is the default: the rows stay together.
    await expect(page.getByTestId('sort-warning-expand')).toBeChecked();
    await page.getByTestId('sort-warning-ok').click();
    await expect(warning).toHaveCount(0);
    await expectValue(page, 'B3', 'An');
    await expectValue(page, 'C3', '9');
    await expectValue(page, 'B2', 'Tên');
    await page.keyboard.press('Control+z');
    await expectValue(page, 'B3', 'Lan');
    // Continue with the current selection: only the names move.
    await goTo(page, 'B3:B7');
    await page.locator('[data-command="data.sortAsc"]').click();
    await page.getByTestId('sort-warning-current').check();
    await page.getByTestId('sort-warning-ok').click();
    await expectValue(page, 'B3', 'An');
    await expectValue(page, 'C3', '7');
    expect(errors).toEqual([]);
  });

  test('Alt+Shift+→ groups the selected rows; hiding the detail skips them', async ({ page }) => {
    const errors = await openSheet(page);
    await typeRows(page, 'A1', [['Nhóm'], ['1'], ['2'], ['3'], ['=SUM(A2:A4)']]);
    // Cells (not whole rows) selected: the rows are grouped.
    await goTo(page, 'A2:A4');
    await page.keyboard.press('Alt+Shift+ArrowRight');
    await page.getByRole('tab', { name: 'Dữ liệu' }).click();
    await page.locator('[data-command="data.collapseOutline"]').click();
    // Rows 2–4 are hidden: Down from A1 lands on A5.
    await goTo(page, 'A1');
    await page.keyboard.press('ArrowDown');
    await expect(page.getByLabel('Hộp tên')).toHaveValue('A5');
    await page.locator('[data-command="data.expandOutline"]').click();
    await goTo(page, 'A1');
    await page.keyboard.press('ArrowDown');
    await expect(page.getByLabel('Hộp tên')).toHaveValue('A2');
    expect(errors).toEqual([]);
  });

  test('the Sort dialog guesses the header row', async ({ page }) => {
    const errors = await openSheet(page);
    await classList(page);
    await page.getByRole('tab', { name: 'Dữ liệu' }).click();
    await goTo(page, 'C3');
    await page.locator('[data-command="data.sort"]').click();
    const header = page.getByRole('dialog').getByRole('checkbox').first();
    await expect(header).toBeChecked();
    // The first level offers the column labels.
    await expect(page.getByTestId('sort-col-0').locator('option:checked')).toHaveText('Điểm');
    await page.keyboard.press('Escape');
    expect(errors).toEqual([]);
  });
});

test.describe('Dữ liệu › Bộ lọc', () => {
  test('Ctrl+Shift+L, Alt+Down, search and the records found', async ({ page }) => {
    const errors = await openSheet(page);
    await typeRows(page, 'A1', [
      ['Tên', 'Lớp', 'Điểm'],
      ['Lan', '10A', '7'],
      ['An', '10B', '9'],
      ['Bình', '10A', '5'],
      ['Châu', '10B', '8'],
      ['Dũng', '10A', '6'],
    ]);
    await typeRows(page, 'C8', [['=SUBTOTAL(9;C2:C6)', '=SUM(C2:C6)']]);
    await goTo(page, 'B3');
    await page.keyboard.press('Control+Shift+L');
    await goTo(page, 'B1');
    await page.keyboard.press('Alt+ArrowDown');
    const menu = page.getByTestId('filter-menu');
    await expect(menu).toBeVisible();
    // Search "A": only the matching items stay ticked when OK is pressed.
    await page.getByTestId('filter-search').fill('A');
    await page.getByTestId('filter-ok').click();
    await expect(menu).toHaveCount(0);
    await expect(page.getByTestId('sheet-status')).toHaveText('Tìm thấy 3 trong 5 bản ghi');
    await expect(page.getByTestId('status-filter')).toBeVisible();
    // SUBTOTAL leaves out the rows the filter hides (7 + 5 + 6), SUM does not.
    await expectValue(page, 'C8', '18');
    await expectValue(page, 'D8', '35');
    // Clear the column's filter from its menu.
    await goTo(page, 'B1');
    await page.keyboard.press('Alt+ArrowDown');
    await menu.getByRole('button', { name: 'Xoá bộ lọc cột' }).click();
    await expectValue(page, 'C8', '35');
    expect(errors).toEqual([]);
  });

  test('sorting from the filter menu orders the filtered list below its header', async ({
    page,
  }) => {
    const errors = await openSheet(page);
    await classList(page);
    await goTo(page, 'C4');
    await page.keyboard.press('Control+Shift+L');
    await goTo(page, 'C2');
    await page.keyboard.press('Alt+ArrowDown');
    await page.getByTestId('filter-menu').getByRole('button', { name: 'Sắp xếp Z → A' }).click();
    await expectValue(page, 'B2', 'Tên');
    await expectValue(page, 'B3', 'An');
    await expectValue(page, 'C7', '5');
    expect(errors).toEqual([]);
  });
});

test.describe('Chèn › Biểu đồ, Bảng tổng hợp', () => {
  test('Alt+F1 charts the data around the active cell; the pivot dialog defaults', async ({
    page,
  }) => {
    const errors = await openSheet(page);
    await classList(page);
    await goTo(page, 'C4');
    await page.keyboard.press('Alt+F1');
    // The new chart is selected (its frame and its contextual tabs show).
    await expect(page.getByTestId('chart-selection')).toBeVisible();
    await page.keyboard.press('Control+z');
    await expect(page.getByTestId('chart-selection')).toHaveCount(0);
    // Insert › PivotTable on one cell of the list: the whole list, on a new sheet.
    await goTo(page, 'B4');
    await page.getByRole('tab', { name: 'Chèn' }).click();
    await page.locator('[data-command="insert.pivot"]').click();
    const dialog = page.getByRole('dialog');
    await expect(dialog).toContainText('2 trường, 5 dòng dữ liệu');
    await expect(dialog.getByRole('radio').first()).toBeChecked();
    await page.keyboard.press('Escape');
    expect(errors).toEqual([]);
  });
});

test.describe('Công thức', () => {
  test('formulas typed with ";" and decimal commas work; syntax errors are not entered', async ({
    page,
  }) => {
    const errors = await openSheet(page);
    await typeRows(page, 'A1', [['10', '0']]);
    await typeRows(page, 'A3', [
      ['=SUM(A1;B1)', '=IF(A1>5;"lớn";"nhỏ")', '=ROUND(2,567;2)', '=A1*1,5', '=SUM(A1,B1'],
    ]);
    await expectValue(page, 'A3', '10');
    await expectFormula(page, 'A3', '=SUM(A1,B1)');
    await expectValue(page, 'B3', 'lớn');
    await expectValue(page, 'C3', '2,57');
    await expectValue(page, 'D3', '15');
    // The missing closing parenthesis is added.
    await expectFormula(page, 'E3', '=SUM(A1,B1)');
    // Error values show in the cell.
    await typeRows(page, 'A5', [['=A1/B1']]);
    await expectValue(page, 'A5', '#DIV/0!');
    // A formula that cannot be read stays in the editor with a message.
    await goTo(page, 'A6');
    await page.keyboard.type('=A1+*2');
    await page.keyboard.press('Enter');
    await expect(page.getByRole('dialog')).toContainText('Công thức có lỗi');
    await page.getByRole('dialog').getByRole('button').last().click();
    await expect(page.getByLabel('Thanh công thức')).toHaveValue('=A1+*2');
    await page.getByLabel('Ô đang nhập').focus();
    await page.keyboard.press('Escape');
    await expectEmpty(page, 'A6');
    expect(errors).toEqual([]);
  });

  test('background error checking marks cells; the warning button fixes or ignores', async ({
    page,
  }) => {
    const errors = await openSheet(page);
    await typeRows(page, 'A1', [['=1/0', "'123", '=A1']]);
    const marks = page.getByTestId('error-mark');
    // The error result and the number stored as text are marked (C1 = A1 shows the error
    // too); a formula reading an empty cell is not (that rule is off by default).
    await expect(page.locator('[data-testid="error-mark"][data-cell="A1"]')).toHaveCount(1);
    await expect(page.locator('[data-testid="error-mark"][data-cell="B1"]')).toHaveCount(1);
    await typeRows(page, 'A3', [['=Z99']]);
    await expect(page.locator('[data-testid="error-mark"][data-cell="A3"]')).toHaveCount(0);
    // The active marked cell gets the warning button; its menu converts the text to a number.
    await goTo(page, 'B1');
    await page.getByTestId('error-tag').click();
    const menu = page.getByTestId('error-tag-menu');
    await expect(menu).toContainText('Số được lưu dạng văn bản');
    await menu.locator('[data-action="convert"]').click();
    await expect(page.locator('[data-testid="error-mark"][data-cell="B1"]')).toHaveCount(0);
    await expectFormula(page, 'B1', '123');
    // Ignore: the mark goes away and stays away.
    await goTo(page, 'A1');
    await page.getByTestId('error-tag').click();
    await menu.locator('[data-action="ignore"]').click();
    await expect(page.locator('[data-testid="error-mark"][data-cell="A1"]')).toHaveCount(0);
    await expect(marks).toHaveCount(1);
    expect(errors).toEqual([]);
  });

  test('Show Formulas (Ctrl+`) toggles; inputs recalculate their formulas', async ({ page }) => {
    const errors = await openSheet(page);
    await typeRows(page, 'A1', [['2', '3', '=A1*B1']]);
    await expectValue(page, 'C1', '6');
    await typeRows(page, 'A1', [['5']]);
    await expectValue(page, 'C1', '15');
    await page.getByRole('tab', { name: 'Công thức' }).click();
    const show = page.locator('[data-command="view.showFormulas"]').first();
    await page.getByLabel('Ô đang nhập').focus();
    await page.keyboard.press('Control+`');
    await expect(show).toHaveAttribute('aria-pressed', 'true');
    await page.keyboard.press('Control+`');
    await expect(show).toHaveAttribute('aria-pressed', 'false');
    expect(errors).toEqual([]);
  });

  test('Name Manager (Ctrl+F3), Create from Selection (Ctrl+Shift+F3) and the Name Box', async ({
    page,
  }) => {
    const errors = await openSheet(page);
    await typeRows(page, 'A1', [
      ['Mặt hàng', 'Đơn giá'],
      ['Bút', '5'],
      ['Vở', '12'],
      ['Thước', '3'],
    ]);
    // Ctrl+F3 → New: Thuế = 0,1 (decimal comma).
    await goTo(page, 'D1');
    await page.keyboard.press('Control+F3');
    await expect(page.getByTestId('name-manager')).toBeVisible();
    await page.getByTestId('name-manager-new').click();
    await page.getByTestId('name-edit-name').fill('Thuế');
    await page.getByTestId('name-edit-ref').fill('=0,1');
    await page.getByTestId('name-edit-ok').click();
    await expect(
      page.getByTestId('name-manager-list').locator('tr[data-name="Thuế"]'),
    ).toContainText('=0.1');
    // A name that reads as a cell reference is refused.
    await page.getByTestId('name-manager-new').click();
    await page.getByTestId('name-edit-name').fill('B2');
    await page.getByTestId('name-edit-ok').click();
    await expect(page.getByTestId('name-edit-error')).toBeVisible();
    await page.getByTestId('name-edit-dialog').getByRole('button').first().click();
    await page.getByTestId('name-manager-close').click();
    // Create from selection: the top row labels name the columns below.
    await goTo(page, 'A1:B4');
    await page.keyboard.press('Control+Shift+F3');
    await expect(page.getByTestId('create-names-top')).toBeChecked();
    await expect(page.getByTestId('create-names-left')).toBeChecked();
    await page.getByTestId('create-names-left').uncheck();
    await page.getByTestId('create-names-ok').click();
    await typeRows(page, 'D2', [['=SUM(Đơn_giá)*(1+Thuế)', '=MAX(Đơn_giá)']]);
    // (5 + 12 + 3) × 1,1 = 22
    await expectValue(page, 'D2', '22');
    await expectValue(page, 'E2', '12');
    // The Name Box names the selection; the name then selects it again.
    await goTo(page, 'B2:B3');
    const box = page.getByLabel('Hộp tên');
    await box.click();
    await box.fill('Hai_dòng');
    await box.press('Enter');
    await typeRows(page, 'D4', [['=SUM(Hai_dòng)']]);
    await expectValue(page, 'D4', '17');
    await goTo(page, 'Hai_dòng');
    await expect(box).toHaveValue('B2:B3');
    expect(errors).toEqual([]);
  });

  test('common functions on typed data give the expected results', async ({ page }) => {
    test.setTimeout(240_000);
    const errors = await openSheet(page);
    await typeRows(page, 'A1', [
      ['Tên', 'Lớp', 'Điểm'],
      ['Lan', 'A', '7'],
      ['An', 'B', '9'],
      ['Bình', 'A', '5'],
      ['Châu', 'B', '8'],
      ['Dũng', 'A', '6'],
    ]);
    // [formula, expected display] — worked out by hand on A1:C6.
    const cases: [string, string][] = [
      ['=SUM(C2:C6)', '35'],
      ['=AVERAGE(C2:C6)', '7'],
      ['=COUNT(C1:C6)', '5'],
      ['=COUNTA(A1:A6)', '6'],
      ['=IF(C2>=7;"Đạt";"Chưa đạt")', 'Đạt'],
      ['=IFS(C3>=9;"Giỏi";C3>=7;"Khá";TRUE;"TB")', 'Giỏi'],
      ['=SUMIF(B2:B6;"A";C2:C6)', '18'],
      ['=SUMIFS(C2:C6;B2:B6;"B";C2:C6;">8")', '9'],
      ['=COUNTIF(B2:B6;"A")', '3'],
      ['=COUNTIFS(B2:B6;"A";C2:C6;">5")', '2'],
      ['=AVERAGEIF(B2:B6;"B";C2:C6)', '8,5'],
      ['=VLOOKUP("Bình";A2:C6;3;FALSE)', '5'],
      ['=HLOOKUP("Điểm";A1:C6;3;FALSE)', '9'],
      ['=XLOOKUP("Châu";A2:A6;C2:C6)', '8'],
      ['=INDEX(C2:C6;MATCH("An";A2:A6;0))', '9'],
      ['=ROUND(2,567;2)', '2,57'],
      ['=ROUNDUP(2,121;1)', '2,2'],
      ['=ROUNDDOWN(2,99;0)', '2'],
      ['=LEFT("BlueSheet";4)', 'Blue'],
      ['=RIGHT("BlueSheet";5)', 'Sheet'],
      ['=MID("BlueSheet";5;3)', 'She'],
      ['=LEN("Hà Nội")', '6'],
      ['=TRIM("  a   b  ")', 'a b'],
      ['=CONCAT(A2;"-";B2)', 'Lan-A'],
      ['=TEXT(0,256;"0%")', '26%'],
      ['=YEAR(DATE(2026;2;15))', '2026'],
      ['=MONTH(DATE(2026;2;15))', '2'],
      ['=DAY(DATE(2026;2;15))', '15'],
      ['=DATEDIF(DATE(2026;1;1);DATE(2026;3;15);"d")', '73'],
      ['=NETWORKDAYS(DATE(2026;11;2);DATE(2026;11;13))', '10'],
      ['=IFERROR(1/0;"lỗi")', 'lỗi'],
      ['=DATE(2026;2;15)', '15/02/2026'],
      ['=TODAY()-TODAY()', '0'],
    ];
    for (const [i, [f]] of cases.entries()) {
      await goTo(page, `E${i + 1}`);
      await page.keyboard.type(f);
      await page.keyboard.press('Enter');
    }
    for (const [i, [f, want]] of cases.entries()) {
      await test.step(f, async () => {
        await expectValue(page, `E${i + 1}`, want);
      });
    }
    // Dynamic arrays spill: UNIQUE, FILTER, SORT.
    await typeRows(page, 'G1', [['=UNIQUE(B2:B6)', '=FILTER(A2:A6;C2:C6>7)', '=SORT(C2:C6)']]);
    await expectValue(page, 'G1', 'A');
    await expectValue(page, 'G2', 'B');
    await expectValue(page, 'H1', 'An');
    await expectValue(page, 'H2', 'Châu');
    await expectValue(page, 'I1', '5');
    await expectValue(page, 'I5', '9');
    expect(errors).toEqual([]);
  });
});
