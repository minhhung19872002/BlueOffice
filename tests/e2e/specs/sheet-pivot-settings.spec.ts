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

test('BlueSheet: group a pivot date field by month, outline layout, % of total, save and reopen', async ({
  page,
}) => {
  const errors: string[] = [];
  page.on('pageerror', (e) => errors.push(e.message));
  await page.goto('/sheet');
  await expect(page.getByRole('tab', { name: 'Sheet1' })).toBeVisible();
  await expect(page.getByTestId('sheet-canvas')).toHaveAttribute('data-rendered', /\d+/);

  // Source A1:C5: dates (DATE formulas), regions, revenue.
  const rows = [
    ['Ngày', 'Vùng', 'Doanh thu'],
    ['=DATE(2024,1,15)', 'Bắc', '10'],
    ['=DATE(2024,1,20)', 'Nam', '30'],
    ['=DATE(2024,2,10)', 'Bắc', '20'],
    ['=DATE(2024,3,5)', 'Nam', '40'],
  ];
  for (const [i, row] of rows.entries()) {
    await goTo(page, `A${i + 1}`);
    for (const v of row) {
      await page.keyboard.type(v);
      await page.keyboard.press('Tab');
    }
  }
  // Format the dates (dd/mm/yyyy) so the field list treats them as dates.
  await goTo(page, 'A2:A5');
  await page.getByRole('tab', { name: 'Trang đầu' }).click();
  await page.locator('[data-command="format.numberFormat"]').click();
  await page.locator('[data-menu="dd/mm/yyyy"]').click();
  await expect(await valueOf(page, 'C5')).toHaveValue('40');

  // Insert a pivot table on a new sheet: Ngày → Rows, Doanh thu → Values.
  await goTo(page, 'B2');
  await page.getByRole('tab', { name: 'Chèn' }).click();
  await page.locator('[data-command="insert.pivot"]').click();
  await expect(page.getByTestId('pivot-source')).toHaveValue('A1:C5');
  await page.getByTestId('pivot-ok').click();
  await expect(page.getByRole('tab', { name: 'Sheet2' })).toHaveAttribute('aria-selected', 'true');
  await expect(page.getByTestId('pivot-panel')).toBeVisible();
  await page.getByTestId('pivot-field-0').check();
  await expect(page.getByTestId('pivot-chip-rows-0')).toContainText('Ngày');
  await page.getByTestId('pivot-field-2').check();
  await expect(page.getByTestId('pivot-chip-values-0')).toContainText('Doanh thu');
  await expect(await valueOf(page, 'B4')).toHaveValue('10');

  // Field settings → group by month: Jan 40, Feb 20, Mar 40.
  await goTo(page, 'A3');
  await page.getByTestId('pivot-settings-rows-0').click();
  await expect(page.getByTestId('pivot-field-dialog')).toBeVisible();
  await page.getByTestId('pivot-group-by').selectOption('dates');
  await expect(page.getByTestId('pivot-group-months')).toBeChecked();
  await page.getByTestId('pivot-field-ok').click();
  await expect(page.getByTestId('pivot-field-dialog')).toBeHidden();
  await expect(await valueOf(page, 'A4')).toHaveValue('Thg 1');
  await expect(await valueOf(page, 'B4')).toHaveValue('40');
  await expect(await valueOf(page, 'A6')).toHaveValue('Thg 3');
  await expect(await valueOf(page, 'B7')).toHaveValue('100');

  // Outline layout from the ribbon (command pivot.layoutOutline).
  await goTo(page, 'A3');
  await page.getByRole('tab', { name: 'Bảng tổng hợp', exact: true }).click();
  const outline = page.locator('[data-command="pivot.layoutOutline"]');
  await outline.click();
  await expect(outline).toHaveAttribute('aria-pressed', 'true');

  // Value field settings → show values as % of the grand total (0.00 % format; the
  // formula bar shows the value).
  await page.getByTestId('pivot-settings-values-0').click();
  await expect(page.getByTestId('pivot-value-dialog')).toBeVisible();
  await page.getByTestId('pivot-show-as').selectOption('percentOfTotal');
  await page.getByTestId('pivot-value-ok').click();
  await expect(page.getByTestId('pivot-value-dialog')).toBeHidden();
  await expect(await valueOf(page, 'B4')).toHaveValue('0,4');
  await expect(await valueOf(page, 'B5')).toHaveValue('0,2');
  await expect(await valueOf(page, 'B7')).toHaveValue('1');

  // Save (Ctrl+S) and reopen the file.
  await page.getByLabel('Ô đang nhập').focus();
  const download = page.waitForEvent('download');
  await page.keyboard.press('Control+s');
  const path = await (await download).path();
  await page.reload();
  await expect(page.getByRole('tab', { name: 'Sheet1' })).toBeVisible();
  const chooser = page.waitForEvent('filechooser');
  await page.locator('[data-command="file.open"]').click();
  await (await chooser).setFiles(path);
  await page.getByRole('tab', { name: 'Sheet2' }).click();
  await expect(await valueOf(page, 'A4')).toHaveValue('Thg 1');
  await expect(await valueOf(page, 'B4')).toHaveValue('0,4');

  // The settings come back: outline layout, grouping by month, % of total.
  await goTo(page, 'A3');
  await expect(page.getByTestId('pivot-panel')).toBeVisible();
  await page.getByRole('tab', { name: 'Bảng tổng hợp', exact: true }).click();
  await expect(page.locator('[data-command="pivot.layoutOutline"]')).toHaveAttribute(
    'aria-pressed',
    'true',
  );
  await page.getByTestId('pivot-settings-values-0').click();
  await expect(page.getByTestId('pivot-show-as')).toHaveValue('percentOfTotal');
  await page.getByRole('button', { name: 'Huỷ' }).click();
  await page.getByTestId('pivot-settings-rows-0').click();
  await expect(page.getByTestId('pivot-group-by')).toHaveValue('dates');
  await expect(page.getByTestId('pivot-group-months')).toBeChecked();
  // The reopened pivot still recomputes: back to plain sums.
  await page.getByTestId('pivot-field-ok').click();
  await page.getByTestId('pivot-settings-values-0').click();
  await page.getByTestId('pivot-show-as').selectOption('normal');
  await page.getByTestId('pivot-value-ok').click();
  await expect(await valueOf(page, 'B4')).toHaveValue('40');
  expect(errors).toEqual([]);
});
