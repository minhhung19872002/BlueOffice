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

test('BlueSheet: create a pivot table, change the aggregation, save and reopen', async ({
  page,
}) => {
  const errors: string[] = [];
  page.on('pageerror', (e) => errors.push(e.message));
  await page.goto('/sheet');
  await expect(page.getByRole('tab', { name: 'Sheet1' })).toBeVisible();
  await expect(page.getByTestId('sheet-canvas')).toHaveAttribute('data-rendered', /\d+/);

  // Source data A1:C5 (header row + 4 records).
  const rows = [
    ['Vùng', 'Sản phẩm', 'Doanh thu'],
    ['Bắc', 'Gạo', '10'],
    ['Nam', 'Gạo', '5'],
    ['Bắc', 'Chè', '7'],
    ['Nam', 'Chè', '1'],
  ];
  for (const [i, row] of rows.entries()) {
    await goTo(page, `A${i + 1}`);
    for (const v of row) {
      await page.keyboard.type(v);
      await page.keyboard.press('Tab');
    }
  }
  await expect(await valueOf(page, 'C5')).toHaveValue('1');

  // Insert → Pivot table: the dialog proposes the current region.
  await goTo(page, 'B2');
  await page.getByRole('tab', { name: 'Chèn' }).click();
  await page.locator('[data-command="insert.pivot"]').click();
  await expect(page.getByTestId('pivot-dialog')).toBeVisible();
  await expect(page.getByTestId('pivot-source')).toHaveValue('A1:C5');
  await page.getByTestId('pivot-ok').click();
  await expect(page.getByRole('tab', { name: 'Sheet2' })).toHaveAttribute('aria-selected', 'true');
  const panel = page.getByTestId('pivot-panel');
  await expect(panel).toBeVisible();

  // Tick "Vùng" (→ Rows) and "Doanh thu" (numbers → Values, Sum).
  await page.getByTestId('pivot-field-0').check();
  await expect(page.getByTestId('pivot-chip-rows-0')).toContainText('Vùng');
  await page.getByTestId('pivot-field-2').check();
  await expect(page.getByTestId('pivot-chip-values-0')).toContainText('Tổng của Doanh thu');
  await expect(await valueOf(page, 'A4')).toHaveValue('Bắc');
  await expect(await valueOf(page, 'B4')).toHaveValue('17');
  await expect(await valueOf(page, 'B6')).toHaveValue('23');

  // Change the value aggregation to Average.
  await goTo(page, 'A3');
  await expect(panel).toBeVisible();
  await page.getByTestId('pivot-fn-0').selectOption('average');
  await expect(page.getByTestId('pivot-chip-values-0')).toContainText(
    'Trung bình của Doanh thu',
  );
  await expect(await valueOf(page, 'B4')).toHaveValue('8,5');
  await expect(await valueOf(page, 'B5')).toHaveValue('3');

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
  await expect(await valueOf(page, 'B4')).toHaveValue('8,5');
  await goTo(page, 'A3');
  await expect(panel).toBeVisible();
  await expect(page.getByTestId('pivot-chip-rows-0')).toContainText('Vùng');
  await expect(page.getByTestId('pivot-fn-0')).toHaveValue('average');
  // The reopened pivot still recomputes (Max) from its source.
  await page.getByTestId('pivot-fn-0').selectOption('max');
  await expect(await valueOf(page, 'B4')).toHaveValue('10');
  expect(errors).toEqual([]);
});
