import { expect, test, type Page } from '@playwright/test';

// BlueSheet Home / Page Layout / View formatting commands and the sheet structure, driven
// through the real UI (keys on the grid, ribbon buttons, dialogs) with the expected results
// of the spreadsheet convention (docs/parity/office-behaviour-sheet-format.md):
//  * typed entries get the number format their writing implies (10% → 0%, 1.234 → #,##0,
//    15.000 ₫ → currency, 1e3 → scientific, 0 1/2 → fraction, 1/2 → a date this year);
//  * the font name / size boxes, size steps 11 → 12 → 14, Ctrl+2/3/4, Ctrl+Shift+~!#$^;
//  * borders on the edge of the selection, split buttons that repeat the last choice;
//  * merge asks before dropping values, indent left-aligns, increase / decrease decimal;
//  * Ctrl++ / Ctrl+- ask how cells shift, column width / row height dialogs, Ctrl+9 / Ctrl+0;
//  * a new sheet goes after the active one (Shift+F11: before it), Ctrl+PgUp / PgDn;
//  * Freeze panes toggles, the format painter stays on after a double click, Ctrl+wheel zooms.

async function openSheet(page: Page) {
  const errors: string[] = [];
  page.on('pageerror', (e) => errors.push(e.message));
  // A wide window: every group of the Home tab shows its buttons.
  await page.setViewportSize({ width: 1920, height: 1000 });
  await page.goto('/sheet');
  await expect(page.getByTestId('sheet-canvas')).toHaveAttribute('data-rendered', /\d+/);
  return errors;
}

async function goTo(page: Page, ref: string) {
  const box = page.getByLabel('Hộp tên');
  await box.click();
  await box.fill(ref);
  await box.press('Enter');
  await expect(page.getByLabel('Ô đang nhập')).toBeFocused();
}

async function typeAt(page: Page, ref: string, text: string) {
  await goTo(page, ref);
  await page.keyboard.type(text);
  await page.keyboard.press('Enter');
}

/** What the active cell announces: `A1: 10%, định dạng 0%`. */
async function said(page: Page, ref: string, text: string) {
  await goTo(page, ref);
  await expect(page.getByTestId('sheet-a11y-live')).toHaveText(text);
}

const cmd = (page: Page, id: string) => page.locator(`[data-command="${id}"]`).first();

/** A command of the Cells › Format menu (row height, column width, …). */
async function cellFormat(page: Page, id: string) {
  await cmd(page, 'cells.formatMenu').click();
  await page.getByTestId('menu-cellFormat').locator(`[data-menu="${id}"]`).click();
}

/** Opens Format Cells (Ctrl+1) on `ref` at `tab` and returns the dialog. */
async function formatCells(page: Page, ref: string, tab: string) {
  await goTo(page, ref);
  await page.keyboard.press('Control+1');
  const dialog = page.getByTestId('format-cells');
  await expect(dialog).toBeVisible();
  await dialog.getByRole('tab', { name: tab }).click();
  return dialog;
}

test.describe('Home › Number: typed entries', () => {
  test('a typed entry gets the format its writing implies (vi-VN)', async ({ page }) => {
    const errors = await openSheet(page);
    const cases: [string, string][] = [
      // 10% is 0,1 shown as a percentage.
      ['10%', 'A1: 10%, định dạng 0%'],
      ['12,5%', 'A2: 12,50%, định dạng 0.00%'],
      // Thousands separator of vi-VN.
      ['1.234', 'A3: 1.234, định dạng #,##0'],
      ['1.234,5', 'A4: 1.234,50, định dạng #,##0.00'],
      // The locale currency.
      ['15.000 ₫', 'A5: 15.000 ₫, định dạng #,##0 [$₫-42A]'],
      ['1e3', 'A6: 1,00E+03, định dạng 0.00E+00'],
      // A fraction needs its whole part (0 1/2); 1/2 alone is a date.
      ['0 1/2', 'A7:  1/2, định dạng # ?/?'],
      ['(5)', 'A8: -5'],
      ["'0123", 'A9: 0123'],
      ['1,5', 'A10: 1,5'],
    ];
    for (const [i, [typed]] of cases.entries()) await typeAt(page, `A${i + 1}`, typed);
    for (const [i, [, text]] of cases.entries()) await said(page, `A${i + 1}`, text);
    // 1/2 is 1 February of this year (day first in vi-VN), shown d-mmm.
    await typeAt(page, 'B1', '1/2');
    await said(page, 'B1', 'B1: 1-Thg 2, định dạng d-mmm');
    const year = new Date().getFullYear();
    await expect(page.getByLabel('Thanh công thức')).toHaveValue(`01/02/${year}`);
    // The number format box names the category of the active cell.
    await goTo(page, 'A1');
    await expect(page.getByTestId('sheet-number-format')).toHaveValue('percent');
    await goTo(page, 'A10');
    await expect(page.getByTestId('sheet-number-format')).toHaveValue('general');
    // A format chosen before typing is kept: 10% in a 0.000 cell is 0,100.
    await goTo(page, 'C1');
    await page.getByTestId('sheet-number-format').selectOption('number');
    await page.keyboard.type('10%');
    await page.keyboard.press('Enter');
    await said(page, 'C1', 'C1: 0,10, định dạng #,##0.00');
    expect(errors).toEqual([]);
  });

  test('number format buttons, shortcuts and increase / decrease decimal', async ({ page }) => {
    const errors = await openSheet(page);
    await typeAt(page, 'B2', '1234,5');
    await goTo(page, 'B2');
    // Ctrl+Shift+!: two decimals with thousands.
    await page.keyboard.press('Control+Shift+!');
    await said(page, 'B2', 'B2: 1.234,50, định dạng #,##0.00');
    // Increase decimal adds one place, decrease takes one away (twice: 1.234,5).
    await cmd(page, 'format.decimalInc').click();
    await said(page, 'B2', 'B2: 1.234,500, định dạng #,##0.000');
    await cmd(page, 'format.decimalDec').click();
    await cmd(page, 'format.decimalDec').click();
    await said(page, 'B2', 'B2: 1.234,5, định dạng #,##0.0');
    // Ctrl+Shift+~: General again; on General the decimals start from what is shown.
    await page.keyboard.press('Control+Shift+~');
    await said(page, 'B2', 'B2: 1234,5');
    await cmd(page, 'format.decimalInc').click();
    await said(page, 'B2', 'B2: 1234,50, định dạng 0.00');
    // Ctrl+Shift+% percent, Ctrl+Shift+^ scientific, Ctrl+Shift+# date.
    await typeAt(page, 'C2', '0,25');
    await goTo(page, 'C2');
    await page.keyboard.press('Control+Shift+%');
    await said(page, 'C2', 'C2: 25%, định dạng 0%');
    await page.keyboard.press('Control+Shift+^');
    await said(page, 'C2', 'C2: 2,50E-01, định dạng 0.00E+00');
    // The money button applies the accounting format, the comma button the comma style.
    await goTo(page, 'B2');
    await cmd(page, 'format.comma').click();
    await expect(page.getByTestId('sheet-a11y-live')).toContainText('#,##0.00_-');
    await cmd(page, 'format.currency').click();
    await expect(page.getByTestId('sheet-number-format')).toHaveValue('accounting');
    // One undo step per command.
    await page.keyboard.press('Control+z');
    await expect(page.getByTestId('sheet-a11y-live')).toContainText('#,##0.00_-');
    expect(errors).toEqual([]);
  });
});

test.describe('Home › Font', () => {
  test('font name and size boxes, size steps, Ctrl+2/3/4 and the mixed selection', async ({
    page,
  }) => {
    const errors = await openSheet(page);
    await typeAt(page, 'A1', 'Tiêu đề');
    await typeAt(page, 'A2', 'Dòng');
    await goTo(page, 'A1');
    const name = page.getByTestId('sheet-font-name');
    const size = page.getByTestId('sheet-font-size');
    await expect(size).toHaveValue('11');
    // Typing a font name and Enter applies it and gives the focus back to the sheet.
    await name.click();
    await name.fill('Arial');
    await name.press('Enter');
    await expect(page.getByLabel('Ô đang nhập')).toBeFocused();
    await expect(name).toHaveValue('Arial');
    await size.click();
    await size.fill('13');
    await size.press('Enter');
    await expect(size).toHaveValue('13');
    // Increase font size goes to the next size of the list (14), then 16; decrease: 14.
    await cmd(page, 'format.fontGrow').click();
    await expect(size).toHaveValue('14');
    await cmd(page, 'format.fontGrow').click();
    await expect(size).toHaveValue('16');
    await cmd(page, 'format.fontShrink').click();
    await expect(size).toHaveValue('14');
    // Another cell shows its own font.
    await goTo(page, 'A2');
    await expect(name).toHaveValue('Calibri');
    await expect(size).toHaveValue('11');
    // Ctrl+B on A1:A2 with the active cell bold removes bold from both (A1 bold first).
    await goTo(page, 'A1');
    await page.keyboard.press('Control+2');
    await said(page, 'A1', 'A1: Tiêu đề, đậm');
    await goTo(page, 'A1:A2');
    await expect(cmd(page, 'format.bold')).toHaveAttribute('aria-pressed', 'true');
    await page.keyboard.press('Control+b');
    await said(page, 'A1', 'A1: Tiêu đề');
    await said(page, 'A2', 'A2: Dòng');
    // ... and again bolds both.
    await goTo(page, 'A1:A2');
    await page.keyboard.press('Control+b');
    await said(page, 'A2', 'A2: Dòng, đậm');
    // Ctrl+3 italic, Ctrl+4 underline toggle.
    await page.keyboard.press('Control+3');
    await page.keyboard.press('Control+4');
    await said(page, 'A2', 'A2: Dòng, đậm, nghiêng, gạch chân');
    await page.keyboard.press('Control+4');
    await said(page, 'A2', 'A2: Dòng, đậm, nghiêng');
    expect(errors).toEqual([]);
  });

  test('fill and font colour split buttons repeat the last colour', async ({ page }) => {
    const errors = await openSheet(page);
    await typeAt(page, 'B2', 'x');
    await goTo(page, 'B2');
    // Yellow until another colour is picked.
    const fillBar = cmd(page, 'format.fillColorLast').locator('[data-color]');
    await expect(fillBar).toHaveAttribute('data-color', 'FFFF00');
    await cmd(page, 'format.fillColorLast').click();
    let dialog = await formatCells(page, 'B2', 'Tô nền');
    await expect(dialog.getByTestId('fill-color-input')).toHaveValue('#ffff00');
    await page.keyboard.press('Escape');
    // Pick another fill from the palette: the button takes it and applies it elsewhere.
    await goTo(page, 'C3');
    await cmd(page, 'format.fillColor').click();
    const palette = page.getByTestId('color-menu');
    const picked = await palette.locator('[data-color]').nth(3).getAttribute('data-color');
    await palette.locator('[data-color]').nth(3).click();
    await expect(fillBar).toHaveAttribute('data-color', picked!);
    await goTo(page, 'D4');
    await cmd(page, 'format.fillColorLast').click();
    dialog = await formatCells(page, 'D4', 'Tô nền');
    await expect(dialog.getByTestId('fill-color-input')).toHaveValue(`#${picked!.toLowerCase()}`);
    await page.keyboard.press('Escape');
    // Font colour: red by default.
    await expect(cmd(page, 'format.fontColorLast').locator('[data-color]')).toHaveAttribute(
      'data-color',
      'FF0000',
    );
    expect(errors).toEqual([]);
  });
});

test.describe('Home › Borders and alignment', () => {
  test('a border goes on the edge of the range; the button repeats the last border', async ({
    page,
  }) => {
    const errors = await openSheet(page);
    await goTo(page, 'B2:C3');
    await cmd(page, 'format.borders').click();
    const menu = page.getByTestId('menu-borders');
    await expect(menu.locator('[data-menu="more"]')).toBeVisible();
    await menu.locator('[data-menu="bottom"]').click();
    // top / right / bottom / left of the cells: only the bottom row of the range.
    const borders = async (ref: string) => {
      const d = await formatCells(page, ref, 'Viền');
      const text = await d.locator('p').last().textContent();
      await page.keyboard.press('Escape');
      return text;
    };
    expect(await borders('C3')).toBe('– / – / thin / –');
    expect(await borders('B3')).toBe('– / – / thin / –');
    expect(await borders('B2')).toBe('Không');
    // The split button now draws a bottom border, here under E5.
    await expect(cmd(page, 'format.bordersLast')).toHaveAttribute('aria-label', /Viền dưới/);
    await goTo(page, 'E5');
    await cmd(page, 'format.bordersLast').click();
    expect(await borders('E5')).toBe('– / – / thin / –');
    // Thick outside borders on E7:F8: the outer edges only, medium line.
    await goTo(page, 'E7:F8');
    await cmd(page, 'format.borders').click();
    await page.getByTestId('menu-borders').locator('[data-menu="thickOutline"]').click();
    expect(await borders('E7')).toBe('medium / – / – / medium');
    expect(await borders('F8')).toBe('– / medium / medium / –');
    // Ctrl+Shift+_ removes them.
    await goTo(page, 'E7:F8');
    await page.keyboard.press('Control+Shift+_');
    expect(await borders('E7')).toBe('Không');
    // More borders… opens Format Cells on its border tab.
    await cmd(page, 'format.borders').click();
    await page.getByTestId('menu-borders').locator('[data-menu="more"]').click();
    await expect(
      page.getByTestId('format-cells').getByRole('tab', { name: 'Viền' }),
    ).toHaveAttribute('aria-selected', 'true');
    await page.keyboard.press('Escape');
    expect(errors).toEqual([]);
  });

  test('merge & centre asks before dropping values and toggles; indent aligns left', async ({
    page,
  }) => {
    const errors = await openSheet(page);
    await typeAt(page, 'B2', 'trái');
    await typeAt(page, 'C2', 'phải');
    await goTo(page, 'B2:C2');
    await cmd(page, 'format.mergeCenter').click();
    const warn = page.getByTestId('merge-warning');
    await expect(warn).toBeVisible();
    // Cancel: nothing changes.
    await warn.getByRole('button', { name: 'Huỷ' }).click();
    await said(page, 'C2', 'C2: phải');
    await goTo(page, 'B2:C2');
    await cmd(page, 'format.mergeCenter').click();
    await page.getByTestId('merge-warning-ok').click();
    await said(page, 'B2', 'B2: trái, ô đã gộp');
    await expect(cmd(page, 'format.alignCenter')).toHaveAttribute('aria-pressed', 'true');
    await expect(cmd(page, 'format.mergeCenter')).toHaveAttribute('aria-pressed', 'true');
    // Again: unmerged.
    await cmd(page, 'format.mergeCenter').click();
    await said(page, 'B2', 'B2: trái');
    // Empty neighbours: merged at once.
    await goTo(page, 'E4:F4');
    await cmd(page, 'format.mergeCenter').click();
    await expect(warn).toHaveCount(0);
    await said(page, 'E4', 'E4: trống, ô đã gộp');
    // Increase indent on a General cell makes it left-aligned with the indent.
    await typeAt(page, 'B6', '125');
    await goTo(page, 'B6');
    await cmd(page, 'format.indentInc').click();
    await expect(cmd(page, 'format.alignLeft')).toHaveAttribute('aria-pressed', 'true');
    const d = await formatCells(page, 'B6', 'Căn lề');
    await expect(d.getByLabel('Thụt lề')).toHaveValue('1');
    await page.keyboard.press('Escape');
    // Orientation › Rotate text up: 90°.
    await cmd(page, 'format.orientation').click();
    await page.getByTestId('menu-orientation').locator('[data-menu="up"]').click();
    const a = await formatCells(page, 'B6', 'Căn lề');
    await expect(a.getByTestId('format-cells-rotation')).toHaveValue('90');
    await page.keyboard.press('Escape');
    expect(errors).toEqual([]);
  });
});

test.describe('Home › Cells', () => {
  test('Ctrl++ / Ctrl+- ask how the cells shift; whole rows go at once', async ({ page }) => {
    const errors = await openSheet(page);
    for (const [ref, v] of [
      ['A1', '1'],
      ['A2', '2'],
      ['A3', '3'],
      ['B1', '10'],
      ['B2', '20'],
      ['B3', '30'],
    ])
      await typeAt(page, ref, v);
    await typeAt(page, 'D1', '=SUM(A1:A3)');
    // Insert a cell at A2: shift down is proposed.
    await goTo(page, 'A2');
    await page.keyboard.press('Control+Shift+=');
    const dialog = page.getByTestId('cells-dialog');
    await expect(dialog).toBeVisible();
    await expect(dialog.locator('[data-shift="down"]')).toBeChecked();
    await page.getByTestId('cells-dialog-ok').click();
    await said(page, 'A2', 'A2: trống');
    await said(page, 'A4', 'A4: 3');
    // Column B did not move; the sum follows its cells.
    await said(page, 'B2', 'B2: 20');
    await said(page, 'D1', 'D1: 6, công thức =SUM(A1:A4)');
    // One undo step.
    await page.keyboard.press('Control+z');
    await said(page, 'A2', 'A2: 2');
    // Delete B1, shift left: C1 (empty) comes in; Ctrl+- proposes "up", pick "left".
    await goTo(page, 'B2');
    await page.keyboard.press('Control+-');
    await expect(dialog.locator('[data-shift="up"]')).toBeChecked();
    await dialog.locator('[data-shift="left"]').check();
    await page.getByTestId('cells-dialog-ok').click();
    await said(page, 'B2', 'B2: trống');
    await said(page, 'B3', 'B3: 30');
    // "Entire row" from the dialog inserts a row.
    await goTo(page, 'A1');
    await page.keyboard.press('Control+Shift+=');
    await dialog.locator('[data-shift="row"]').check();
    await page.getByTestId('cells-dialog-ok').click();
    await said(page, 'A2', 'A2: 1');
    await said(page, 'B2', 'B2: 10');
    // The cell menu offers the same commands.
    await goTo(page, 'C5');
    const active = page.getByLabel('Ô đang nhập');
    const box = (await active.boundingBox())!;
    await page.mouse.click(box.x + box.width / 2, box.y + box.height / 2, { button: 'right' });
    await expect(page.locator('[data-menu-command="cells.insert"]')).toBeVisible();
    await expect(page.locator('[data-menu-command="cells.delete"]')).toBeVisible();
    await page.keyboard.press('Escape');
    expect(errors).toEqual([]);
  });

  test('column width and row height dialogs; Ctrl+9 / Ctrl+Shift+( hide and unhide', async ({
    page,
  }) => {
    const errors = await openSheet(page);
    await goTo(page, 'B2');
    await cellFormat(page, 'cells.colWidth');
    const dialog = page.getByTestId('size-dialog');
    // The default column: 8,43 characters.
    await expect(dialog.getByTestId('size-dialog-value')).toHaveValue('8,43');
    await dialog.getByTestId('size-dialog-value').fill('20');
    await page.keyboard.press('Enter');
    await expect(dialog).toHaveCount(0);
    await cellFormat(page, 'cells.colWidth');
    await expect(page.getByTestId('size-dialog-value')).toHaveValue('20');
    await page.keyboard.press('Escape');
    // Row height in points (15 by default).
    await cellFormat(page, 'cells.rowHeight');
    await expect(page.getByTestId('size-dialog-value')).toHaveValue('15');
    await page.getByTestId('size-dialog-value').fill('30');
    await page.getByTestId('size-dialog-ok').click();
    await cellFormat(page, 'cells.rowHeight');
    await expect(page.getByTestId('size-dialog-value')).toHaveValue('30');
    await page.keyboard.press('Escape');
    // Ctrl+9 hides row 3: Down from row 2 lands on row 4.
    await goTo(page, 'A3');
    await page.keyboard.press('Control+9');
    await goTo(page, 'A2');
    await page.keyboard.press('ArrowDown');
    await expect(page.getByLabel('Hộp tên')).toHaveValue('A4');
    // Ctrl+Shift+( on rows 2–4 shows it again.
    await goTo(page, 'A2:A4');
    await page.keyboard.press('Control+Shift+(');
    await goTo(page, 'A2');
    await page.keyboard.press('ArrowDown');
    await expect(page.getByLabel('Hộp tên')).toHaveValue('A3');
    expect(errors).toEqual([]);
  });

  test('Clear › Clear all removes contents and formats in one step', async ({ page }) => {
    const errors = await openSheet(page);
    await typeAt(page, 'B2', '10%');
    await goTo(page, 'B2');
    await page.keyboard.press('Control+b');
    await said(page, 'B2', 'B2: 10%, đậm, định dạng 0%');
    await cmd(page, 'edit.clearMenu').click();
    await page.getByTestId('menu-clear').locator('[data-menu="edit.clearAll"]').click();
    await said(page, 'B2', 'B2: trống');
    // Typing 5 now gives a plain 5 (the percent format went too).
    await typeAt(page, 'B2', '5');
    await said(page, 'B2', 'B2: 5');
    await page.keyboard.press('Control+z');
    await page.keyboard.press('Control+z');
    await said(page, 'B2', 'B2: 10%, đậm, định dạng 0%');
    expect(errors).toEqual([]);
  });

  test('the format painter stays on after a double click until Esc', async ({ page }) => {
    const errors = await openSheet(page);
    await typeAt(page, 'B2', 'mẫu');
    await typeAt(page, 'D2', 'một');
    await typeAt(page, 'D4', 'hai');
    await typeAt(page, 'D6', 'ba');
    await goTo(page, 'B2');
    await page.keyboard.press('Control+b');
    await cmd(page, 'format.painter').dblclick();
    await expect(page.getByTestId('sheet-status')).toHaveText(/định dạng/i);
    const click = async (ref: string) => {
      await goTo(page, ref);
      const b = (await page.getByLabel('Ô đang nhập').boundingBox())!;
      return { x: b.x + b.width / 2, y: b.y + b.height / 2 };
    };
    const d2 = await click('D2');
    const d4 = await click('D4');
    await page.mouse.click(d2.x, d2.y);
    await page.mouse.click(d4.x, d4.y);
    await said(page, 'D2', 'D2: một, đậm');
    await said(page, 'D4', 'D4: hai, đậm');
    // Still painting until Esc.
    await expect(cmd(page, 'format.painter')).toHaveAttribute('aria-pressed', 'true');
    await page.keyboard.press('Escape');
    await expect(cmd(page, 'format.painter')).toHaveAttribute('aria-pressed', 'false');
    const d6 = await click('D6');
    await page.mouse.click(d6.x, d6.y);
    await said(page, 'D6', 'D6: ba');
    expect(errors).toEqual([]);
  });
});

test.describe('Format as table', () => {
  test('Ctrl+T takes the data around the cell and guesses the header row', async ({ page }) => {
    const errors = await openSheet(page);
    for (const [ref, v] of [
      ['A1', 'Tên'],
      ['B1', 'Số lượng'],
      ['A2', 'bút'],
      ['B2', '10'],
      ['A3', 'vở'],
      ['B3', '20'],
    ])
      await typeAt(page, ref, v);
    await goTo(page, 'B2');
    await page.keyboard.press('Control+t');
    const dialog = page.getByTestId('create-table');
    await expect(dialog).toBeVisible();
    // The current region, and its first row holds names.
    await expect(page.getByTestId('create-table-range')).toHaveValue('A1:B3');
    await expect(page.getByTestId('create-table-headers')).toBeChecked();
    await page.getByTestId('create-table-ok').click();
    await goTo(page, 'A2');
    await expect(page.getByRole('tab', { name: 'Thiết kế bảng' })).toBeVisible();
    await said(page, 'A1', 'A1: Tên');
    // Numbers in the first row: no names; the table gets a row of names above the data.
    for (const [ref, v] of [
      ['E1', '1'],
      ['F1', '2'],
      ['E2', '3'],
      ['F2', '4'],
    ])
      await typeAt(page, ref, v);
    await goTo(page, 'E1');
    await page.keyboard.press('Control+l');
    await expect(page.getByTestId('create-table-range')).toHaveValue('E1:F2');
    await expect(page.getByTestId('create-table-headers')).not.toBeChecked();
    await page.getByTestId('create-table-ok').click();
    await said(page, 'E1', 'E1: Cột1');
    await said(page, 'F1', 'F1: Cột2');
    await said(page, 'E2', 'E2: 1');
    await said(page, 'F3', 'F3: 4');
    // One undo step takes the table and the names back.
    await page.keyboard.press('Control+z');
    await said(page, 'E1', 'E1: 1');
    await said(page, 'F2', 'F2: 4');
    expect(errors).toEqual([]);
  });
});

test.describe('Sheets, View and Format Cells', () => {
  test('new sheets go after the active one (Shift+F11 before); Ctrl+PgUp / PgDn', async ({
    page,
  }) => {
    const errors = await openSheet(page);
    const tabs = page.getByRole('tablist', { name: /trang tính/i }).getByRole('tab');
    await page.getByRole('button', { name: 'Thêm trang tính' }).last().click();
    await expect(tabs).toHaveText(['Sheet1', 'Sheet2']);
    await expect(tabs.nth(1)).toHaveAttribute('aria-selected', 'true');
    // Back on Sheet1 (Ctrl+PgUp), + puts Sheet3 right after it.
    await page.keyboard.press('Control+PageUp');
    await expect(tabs.nth(0)).toHaveAttribute('aria-selected', 'true');
    await page.getByRole('button', { name: 'Thêm trang tính' }).last().click();
    await expect(tabs).toHaveText(['Sheet1', 'Sheet3', 'Sheet2']);
    await expect(tabs.nth(1)).toHaveAttribute('aria-selected', 'true');
    // Shift+F11 inserts before the active sheet.
    await page.keyboard.press('Shift+F11');
    await expect(tabs).toHaveText(['Sheet1', 'Sheet4', 'Sheet3', 'Sheet2']);
    await expect(tabs.nth(1)).toHaveAttribute('aria-selected', 'true');
    // One undo step takes it away again.
    await page.keyboard.press('Control+z');
    await expect(tabs).toHaveText(['Sheet1', 'Sheet3', 'Sheet2']);
    // Ctrl+PgDn walks to the right.
    await page.keyboard.press('Control+PageDown');
    await page.keyboard.press('Control+PageDown');
    await expect(tabs.nth(2)).toHaveAttribute('aria-selected', 'true');
    // Double click renames.
    await tabs.nth(2).dblclick();
    await page.keyboard.press('Control+a');
    await page.keyboard.type('Báo cáo');
    await page.keyboard.press('Enter');
    await expect(tabs.nth(2)).toHaveText('Báo cáo');
    expect(errors).toEqual([]);
  });

  test('freeze panes toggles; Ctrl+wheel zooms; Format Cells has the protection tab', async ({
    page,
  }) => {
    const errors = await openSheet(page);
    await goTo(page, 'B3');
    await page.getByRole('tab', { name: 'Xem', exact: true }).click();
    const freeze = cmd(page, 'view.freezePanes');
    await expect(freeze).toContainText('Cố định ô');
    await freeze.click();
    // Frozen: the same button now unfreezes.
    await expect(freeze).toContainText('Bỏ cố định');
    await freeze.click();
    await expect(freeze).toContainText('Cố định ô');
    // Ctrl+wheel over the grid: 100 % → 110 %, and back.
    const canvas = page.getByTestId('sheet-canvas');
    const b = (await canvas.boundingBox())!;
    await page.mouse.move(b.x + 200, b.y + 200);
    await page.keyboard.down('Control');
    await page.mouse.wheel(0, -100);
    await page.keyboard.up('Control');
    await expect(page.getByText('110%', { exact: true })).toBeVisible();
    await page.keyboard.down('Control');
    await page.mouse.wheel(0, 100);
    await page.keyboard.up('Control');
    await expect(page.getByText('100%', { exact: true })).toBeVisible();
    // Format Cells: six tabs; Ctrl+Shift+F opens it on Font.
    await goTo(page, 'B3');
    await page.keyboard.press('Control+Shift+F');
    const dialog = page.getByTestId('format-cells');
    await expect(dialog.getByRole('tab')).toHaveText([
      'Số',
      'Căn lề',
      'Phông chữ',
      'Viền',
      'Tô nền',
      'Bảo vệ',
    ]);
    await expect(dialog.getByRole('tab', { name: 'Phông chữ' })).toHaveAttribute(
      'aria-selected',
      'true',
    );
    await dialog.getByRole('tab', { name: 'Bảo vệ' }).click();
    await expect(dialog.getByTestId('format-cells-locked')).toBeChecked();
    await dialog.getByTestId('format-cells-hidden').check();
    await dialog.getByTestId('format-cells-ok').click();
    const again = await formatCells(page, 'B3', 'Bảo vệ');
    await expect(again.getByTestId('format-cells-hidden')).toBeChecked();
    await page.keyboard.press('Escape');
    expect(errors).toEqual([]);
  });
});
