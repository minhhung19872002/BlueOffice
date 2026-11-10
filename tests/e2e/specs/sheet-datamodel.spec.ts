import { expect, test, type Page } from '@playwright/test';

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

async function typeRows(page: Page, at: string, rows: string[][]) {
  const col = at.charCodeAt(0);
  for (const [i, row] of rows.entries()) {
    await goTo(page, `${String.fromCharCode(col)}${i + 1}`);
    for (const v of row) {
      await page.keyboard.type(v);
      await page.keyboard.press('Tab');
    }
  }
}

test('BlueSheet: two tables, a relationship, a measure and a pivot by dimension (SHEET-DM-001)', async ({
  page,
}) => {
  const errors: string[] = [];
  page.on('pageerror', (e) => errors.push(e.message));
  await page.goto('/sheet');
  await expect(page.getByTestId('sheet-canvas')).toHaveAttribute('data-rendered', /\d+/);

  // Products (dimension) A1:B4 and Sales (facts) D1:E6, each made a table.
  await typeRows(page, 'A', [
    ['Mã', 'Nhóm'],
    ['1', 'Gạo'],
    ['2', 'Chè'],
    ['3', 'Chè'],
  ]);
  await typeRows(page, 'D', [
    ['Mã SP', 'SL'],
    ['1', '5'],
    ['2', '7'],
    ['3', '1'],
    ['1', '2'],
    ['2', '4'],
  ]);
  await expect(await valueOf(page, 'E6')).toHaveValue('4');
  await page.getByRole('tab', { name: 'Chèn' }).click();
  await goTo(page, 'A1:B4');
  await page.locator('[data-command="insert.table"]').click();
  await goTo(page, 'D1:E6');
  await page.locator('[data-command="insert.table"]').click();

  // Data › Data model › Manage: both tables join the model.
  await page.getByRole('tab', { name: 'Dữ liệu', exact: true }).click();
  await page.locator('[data-command="data.modelTables"]').click();
  const tables = page.getByTestId('model-tables-dialog');
  await expect(tables).toBeVisible();
  const boxes = tables.locator('input[type="checkbox"]');
  await expect(boxes).toHaveCount(2);
  await boxes.nth(0).click();
  await expect(boxes.nth(0)).toBeChecked();
  await boxes.nth(1).click();
  await expect(boxes.nth(1)).toBeChecked();
  await page.getByTestId('model-close').click();
  await expect(tables).toBeHidden();

  // Relationships: Products[Mã] ↔ Sales[Mã SP] is detected one to many and stored the other
  // way round (Sales is the many side).
  await page.locator('[data-command="data.modelRelationships"]').click();
  const rel = page.getByTestId('model-rel-dialog');
  await expect(rel).toBeVisible();
  await rel.getByTestId('model-rel-add').click();
  await expect(rel.getByTestId('model-rel-from-column')).toHaveValue('Mã');
  await expect(rel.getByTestId('model-rel-to-column')).toHaveValue('Mã SP');
  await expect(rel.getByTestId('model-rel-cardinality')).toContainText('Một – nhiều');
  await rel.getByTestId('model-rel-save').click();
  await expect(rel.getByTestId('model-rel-0')).toContainText('[Mã SP] (nhiều)');
  await expect(rel.getByTestId('model-rel-0')).toContainText('[Mã] (một)');
  await page.getByTestId('model-close').click();

  // Measures: the formula box suggests the columns and reports syntax errors.
  await page.locator('[data-command="data.modelMeasures"]').click();
  const dlg = page.getByTestId('model-measure-dialog');
  await expect(dlg).toBeVisible();
  await dlg.getByTestId('model-measure-new').click();
  const salesTable = await dlg
    .getByTestId('model-measure-table')
    .locator('option')
    .nth(1)
    .getAttribute('value');
  expect(salesTable).toBeTruthy();
  await dlg.getByTestId('model-measure-table').selectOption(salesTable ?? '');
  await dlg.getByTestId('model-measure-name').fill('Tổng SL');
  const box = dlg.getByTestId('model-measure-expression');
  await box.click();
  await page.keyboard.type(`SUM(${salesTable}[`);
  await dlg.getByTestId('model-suggest-SL').click();
  await expect(box).toHaveValue(`SUM(${salesTable}[SL]`);
  await expect(dlg.getByTestId('model-measure-check')).toContainText('Lỗi');
  await expect(dlg.getByTestId('model-measure-save')).toBeDisabled();
  await box.press('End');
  await page.keyboard.type(')');
  await expect(dlg.getByTestId('model-measure-check')).toHaveText('Không có lỗi.');
  await dlg.getByTestId('model-measure-save').click();
  await expect(dlg.getByTestId('model-measure-list')).toContainText('[Tổng SL]');
  await page.getByTestId('model-close').click();

  // Pivot table from the model: fields grouped by table; rows from the dimension, the
  // measure from the fact table.
  await page.locator('[data-command="data.modelPivot"]').click();
  await expect(page.getByRole('tab', { name: 'Sheet2' })).toHaveAttribute('aria-selected', 'true');
  const panel = page.getByTestId('pivot-panel');
  await expect(panel).toBeVisible();
  await expect(panel.locator('[data-testid^="pivot-group-"]')).toHaveCount(2);
  // Fields: Products[Mã], Products[Nhóm], Sales[Mã SP], Sales[SL], [Tổng SL].
  await page.getByTestId('pivot-field-1').check();
  await expect(page.getByTestId('pivot-chip-rows-0')).toContainText('Nhóm');
  await page.getByTestId('pivot-field-4').check();
  await expect(page.getByTestId('pivot-chip-values-0')).toContainText('Tổng SL');
  // Chè: products 2, 3 → 7 + 1 + 4; Gạo: product 1 → 5 + 2; total 19.
  await expect(await valueOf(page, 'A4')).toHaveValue('Chè');
  await expect(await valueOf(page, 'B4')).toHaveValue('12');
  await expect(await valueOf(page, 'A5')).toHaveValue('Gạo');
  await expect(await valueOf(page, 'B5')).toHaveValue('7');
  await expect(await valueOf(page, 'B6')).toHaveValue('19');
  expect(errors).toEqual([]);
});
