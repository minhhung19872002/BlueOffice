import { expect, test, type Page } from '@playwright/test';

// SHEET-FLASH-001 / SHEET-DATA-002 / SHEET-AUDIT-001 / SHEET-VIEW-001 / SHEET-SPARK-001
// through the real UI: the grey Flash Fill suggestion while typing (Enter accepts), the
// "replace the data there?" question of Text to Columns and of dragging cells, the
// fixed-width ruler, the precision-as-displayed question, Shift+F9 in manual mode, typing
// in a cell on a Page Layout page, and Sparkline › Edit data. Expected values are worked
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

async function typeColumn(page: Page, ref: string, values: string[]) {
  await goTo(page, ref);
  for (const v of values) {
    await page.keyboard.type(v);
    await page.keyboard.press('Enter');
  }
}

async function expectValue(page: Page, ref: string, value: string) {
  await goTo(page, ref);
  await expect(page.getByTestId('sheet-a11y-live')).toHaveText(
    new RegExp(`^${ref}: ${value.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')}(,|$)`),
  );
}

async function expectInput(page: Page, ref: string, value: string) {
  await goTo(page, ref);
  await expect(page.getByLabel('Thanh công thức')).toHaveValue(value);
}

const cmd = (page: Page, id: string) => page.locator(`[data-command="${id}"]`);

test('Flash Fill suggests the rest of the column while the second example is typed', async ({
  page,
}) => {
  const errors = await openSheet(page);
  await typeColumn(page, 'A1', ['Nguyễn Văn An', 'Trần Thị Bích', 'Lê Minh Tuấn', 'Phạm Hùng']);
  await typeColumn(page, 'B1', ['Nguyễn']);
  // B2: typing "Tr" begins "Trần" — the family names of rows 3 and 4 show in grey.
  await page.keyboard.type('Tr');
  const hint = page.getByTestId('flash-fill-hint');
  await expect(hint).toBeVisible();
  await expect(hint.locator('[data-row="2"]')).toHaveText('Lê');
  await expect(hint.locator('[data-row="3"]')).toHaveText('Phạm');
  // Text that leaves the pattern hides the suggestion; back on it, it returns.
  await page.keyboard.type('x');
  await expect(hint).toHaveCount(0);
  await page.keyboard.press('Backspace');
  await page.keyboard.type('ần');
  await expect(page.getByTestId('flash-fill-hint')).toBeVisible();
  // Enter keeps the typed entry and fills the suggestion.
  await page.keyboard.press('Enter');
  await expect(page.getByTestId('flash-fill-hint')).toHaveCount(0);
  await expectInput(page, 'B2', 'Trần');
  await expectInput(page, 'B3', 'Lê');
  await expectInput(page, 'B4', 'Phạm');
  // Undo takes the filled cells back, the typed entry stays.
  await page.keyboard.press('Control+z');
  await expectInput(page, 'B3', '');
  await expectInput(page, 'B2', 'Trần');
  expect(errors).toEqual([]);
});

test('Text to Columns: fixed-width ruler and the question before replacing data', async ({
  page,
}) => {
  const errors = await openSheet(page);
  await typeColumn(page, 'A1', ['An  12', 'Bo  7']);
  await typeColumn(page, 'B1', ['cũ']);
  await goTo(page, 'A1:A2');
  await page.getByRole('tab', { name: 'Dữ liệu' }).click();
  await cmd(page, 'data.textToColumns').click();
  const dlg = page.getByTestId('split-dialog');
  await dlg.getByTestId('split-fixed').check();
  await dlg.getByTestId('split-next').click();
  // The break the text suggests (character 4) is proposed on the ruler.
  const ruler = dlg.getByTestId('split-ruler');
  await expect(ruler).toContainText('An  12');
  await expect(dlg.getByTestId('split-break-4')).toBeVisible();
  await expect(dlg.getByTestId('split-breaks')).toHaveValue('4');
  // A click on the text at character 1 adds a break; double-click removes it again.
  const four = (await dlg.getByTestId('split-break-4').boundingBox())!;
  const text = (await ruler.locator('pre').boundingBox())!;
  const ch = (four.x + four.width / 2 - text.x) / 4;
  const y = text.y + text.height / 2;
  await page.mouse.click(text.x + ch * 1, y);
  await expect(dlg.getByTestId('split-breaks')).toHaveValue('1, 4');
  await expect(dlg.getByTestId('split-break-1')).toBeVisible();
  await page.mouse.dblclick(text.x + ch * 1, y);
  await expect(dlg.getByTestId('split-breaks')).toHaveValue('4');
  // Drag the line at 4 to 3.
  const b = (await dlg.getByTestId('split-break-4').boundingBox())!;
  await page.mouse.move(b.x + b.width / 2, y);
  await page.mouse.down();
  await page.mouse.move(text.x + ch * 3, y, { steps: 3 });
  await page.mouse.up();
  await expect(dlg.getByTestId('split-breaks')).toHaveValue('3');
  await expect(dlg.getByTestId('split-break-3')).toBeVisible();
  // Finish: B1 holds data, so the wizard asks first; Cancel keeps everything.
  await dlg.getByTestId('split-ok').click();
  const ask = page.getByTestId('replace-warning');
  await expect(ask).toBeVisible();
  await ask.getByRole('button', { name: 'Hủy' }).click();
  await expect(ask).toBeHidden();
  await dlg.getByTestId('split-ok').click();
  await page.getByTestId('replace-warning-ok').click();
  await expect(dlg).toBeHidden();
  await expectInput(page, 'A1', 'An');
  await expectInput(page, 'B1', '12');
  await expectInput(page, 'B2', '7');
  expect(errors).toEqual([]);
});

test('dragging cells onto data asks before replacing it', async ({ page }) => {
  const errors = await openSheet(page);
  await typeColumn(page, 'A1', ['mới']);
  await typeColumn(page, 'B1', ['cũ']);
  await goTo(page, 'A1');
  // Grab the right edge of the selected A1 (above the fill handle) and drop one column on.
  const h = (await page.getByTestId('fill-handle').boundingBox())!;
  const x = h.x + h.width / 2;
  const y = h.y - 9;
  await page.mouse.move(x, y);
  await page.mouse.down();
  await page.mouse.move(x + 30, y, { steps: 3 });
  await page.mouse.move(x + 66, y, { steps: 3 });
  await page.mouse.up();
  const ask = page.getByTestId('replace-warning');
  await expect(ask).toBeVisible();
  await page.getByTestId('replace-warning-ok').click();
  await expectInput(page, 'B1', 'mới');
  await expectInput(page, 'A1', '');
  // One undo step.
  await page.keyboard.press('Control+z');
  await expectInput(page, 'B1', 'cũ');
  await expectInput(page, 'A1', 'mới');
  expect(errors).toEqual([]);
});

test('precision as displayed asks first; Shift+F9 calculates the sheet in manual mode', async ({
  page,
}) => {
  const errors = await openSheet(page);
  await typeColumn(page, 'A1', ['1']);
  await typeColumn(page, 'B1', ['=A1*2']);
  await page.getByRole('tab', { name: 'Công thức' }).click();
  await cmd(page, 'formulas.calcOptions').first().click();
  const opts = page.getByTestId('calc-options');
  await expect(opts).toBeVisible();
  await opts.getByTestId('calc-precision').check();
  const ask = page.getByTestId('calc-precision-confirm');
  await expect(ask).toBeVisible();
  await ask.getByRole('button', { name: 'Hủy' }).click();
  await expect(opts.getByTestId('calc-precision')).not.toBeChecked();
  await opts.getByTestId('calc-mode-manual').check();
  await opts.getByTestId('calc-ok').click();
  await expect(opts).toBeHidden();
  // Manual: A1 = 5 leaves B1 at 2 until Shift+F9.
  await typeColumn(page, 'A1', ['5']);
  await expectValue(page, 'B1', '2');
  await page.keyboard.press('Shift+F9');
  await expectValue(page, 'B1', '10');
  expect(errors).toEqual([]);
});

test('Page Layout view: empty sheet shows a page of cells, typing edits the cell in place', async ({
  page,
}) => {
  const errors = await openSheet(page);
  await page.getByRole('tab', { name: 'Xem', exact: true }).click();
  await cmd(page, 'view.pageLayout').click();
  // An empty sheet still has a page of cells to type in.
  const first = page.getByTestId('page-layout-page-0');
  await expect(first).toBeVisible();
  const box = (await first.boundingBox())!;
  // C3: left margin 0.7 in = 67 px, top 0.75 in = 72 px, columns 64 px, rows 20 px.
  await page.mouse.click(box.x + 67 + 2 * 64 + 20, box.y + 72 + 2 * 20 + 8);
  await expect(page.getByLabel('Hộp tên')).toHaveValue('C3');
  await page.keyboard.type('Xin');
  // The editor sits on the page over C3.
  const editor = page.getByTestId('page-layout-cell-editor');
  await expect(editor).toBeVisible();
  await expect(editor).toBeFocused();
  await expect(editor).toHaveValue('Xin');
  await page.keyboard.type(' chào');
  await page.keyboard.press('Enter');
  await expect(editor).toHaveCount(0);
  await expect(page.getByLabel('Hộp tên')).toHaveValue('C4');
  await expect(page.getByTestId('page-layout-view')).toBeFocused();
  // Double-click C3 edits its content in place; Esc gives the change up.
  await page.mouse.dblclick(box.x + 67 + 2 * 64 + 20, box.y + 72 + 2 * 20 + 8);
  await expect(page.getByTestId('page-layout-cell-editor')).toHaveValue('Xin chào');
  await page.keyboard.type('!');
  await page.keyboard.press('Escape');
  await cmd(page, 'view.normal').click();
  await expectInput(page, 'C3', 'Xin chào');
  expect(errors).toEqual([]);
});

test('Sparkline › Edit data moves the group and switches rows and columns', async ({ page }) => {
  const errors = await openSheet(page);
  // A1:C3, a 3 × 3 block: rows or columns fit three sparklines.
  for (const [i, row] of [
    ['1', '2', '3'],
    ['4', '5', '6'],
    ['7', '8', '9'],
  ].entries()) {
    await goTo(page, `A${i + 1}`);
    for (const v of row) {
      await page.keyboard.type(v);
      await page.keyboard.press('Tab');
    }
  }
  await goTo(page, 'A1:C3');
  await page.getByRole('tab', { name: 'Chèn' }).click();
  await cmd(page, 'insert.sparklines').click();
  await page.getByTestId('sparkline-insert-ok').click();
  await goTo(page, 'D1');
  await page.getByRole('tab', { name: 'Biểu đồ thu nhỏ' }).click();
  // Edit data: the group's block and cells are proposed; move it to F1:F3.
  await cmd(page, 'sparkline.editData').click();
  const dlg = page.getByTestId('sparkline-edit-data');
  await expect(dlg.getByTestId('sparkline-edit-data-range')).toHaveValue('Sheet1!A1:C3');
  await expect(dlg.getByTestId('sparkline-edit-data-location')).toHaveValue('D1:D3');
  await dlg.getByTestId('sparkline-edit-data-location').fill('F1:F3');
  await dlg.getByTestId('sparkline-edit-data-ok').click();
  await expect(dlg).toBeHidden();
  // D1 no longer has a sparkline (no contextual tab), F2 does.
  await goTo(page, 'D1');
  await expect(page.getByRole('tab', { name: 'Biểu đồ thu nhỏ' })).toHaveCount(0);
  await goTo(page, 'F2');
  await page.getByRole('tab', { name: 'Biểu đồ thu nhỏ' }).click();
  // This sparkline's data: F2 reads row 2 of the block.
  await cmd(page, 'sparkline.editSingle').click();
  await expect(page.getByTestId('sparkline-edit-data-range')).toHaveValue('Sheet1!A2:C2');
  await page.getByRole('button', { name: 'Hủy' }).click();
  // Switch rows and columns: F2 reads column B; undo brings rows back.
  await cmd(page, 'sparkline.switch').click();
  await cmd(page, 'sparkline.editSingle').click();
  await expect(page.getByTestId('sparkline-edit-data-range')).toHaveValue('Sheet1!B1:B3');
  await page.getByRole('button', { name: 'Hủy' }).click();
  await page.getByLabel('Ô đang nhập').focus();
  await page.keyboard.press('Control+z');
  await cmd(page, 'sparkline.editSingle').click();
  await expect(page.getByTestId('sparkline-edit-data-range')).toHaveValue('Sheet1!A2:C2');
  expect(errors).toEqual([]);
});
