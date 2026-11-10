import { expect, test, type Page } from '@playwright/test';
import { readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const here = dirname(fileURLToPath(import.meta.url));
const salesCsv = readFileSync(
  join(here, '..', '..', 'corpus', 'sheet', 'query', 'sales.csv'),
  'utf8',
);

async function goTo(page: Page, ref: string) {
  const box = page.getByLabel('Hộp tên');
  await box.click();
  await box.fill(ref);
  await box.press('Enter');
  await expect(page.getByLabel('Ô đang nhập')).toBeFocused();
}

async function inputOf(page: Page, ref: string) {
  await goTo(page, ref);
  return page.getByLabel('Thanh công thức');
}

test('BlueSheet: import a CSV, add two steps, load, edit the source and refresh (SHEET-PQ-001)', async ({
  page,
}) => {
  const errors: string[] = [];
  page.on('pageerror', (e) => errors.push(e.message));
  await page.goto('/sheet');
  await expect(page.getByTestId('sheet-canvas')).toHaveAttribute('data-rendered', /\d+/);

  // Data › Get & Transform › From CSV/Text.
  await page.getByRole('tab', { name: 'Dữ liệu', exact: true }).click();
  const chooser = page.waitForEvent('filechooser');
  await page.locator('[data-command="data.fromCsv"]').click();
  await (
    await chooser
  ).setFiles({ name: 'sales.csv', mimeType: 'text/csv', buffer: Buffer.from(salesCsv, 'utf8') });
  const editor = page.getByTestId('query-editor');
  await expect(editor).toBeVisible();
  await expect(page.getByTestId('query-name')).toHaveValue('sales');
  // The delimiter was detected; headers and types are the proposed first steps.
  await expect(page.getByTestId('query-source-delimiter')).toHaveValue(';');
  await expect(editor.getByTestId('query-step-0')).toContainText('Dòng đầu thành tiêu đề');
  await expect(editor.getByTestId('query-step-1')).toContainText('Đổi kiểu');
  await expect(page.getByTestId('query-row-count')).toContainText('5 dòng, 6 cột');

  // Step 1 from the column menu of "Khu vực": filter out "Trung".
  await editor.getByTestId('query-col-2').click();
  await page.getByTestId('query-colmenu-filter').click();
  const form = page.getByTestId('query-form');
  await form.getByTestId('query-form-condition').selectOption('notEquals');
  await form.getByTestId('query-form-value').fill('Trung');
  await form.getByTestId('query-form-ok').click();
  await expect(editor.getByTestId('query-step-2')).toContainText('Lọc: Khu vực khác Trung');
  await expect(page.getByTestId('query-row-count')).toContainText('4 dòng, 6 cột');

  // Step 2 from the column menu of "Đơn giá": remove it.
  await editor.getByTestId('query-col-4').click();
  await page.getByTestId('query-colmenu-remove').click();
  await expect(editor.getByTestId('query-step-3')).toContainText('Bỏ cột: Đơn giá');
  await expect(page.getByTestId('query-row-count')).toContainText('4 dòng, 5 cột');

  // Close & Load: a new sheet "sales" with the table; the Queries pane lists the query.
  await page.getByTestId('query-load').click();
  await expect(editor).toBeHidden();
  await expect(page.getByRole('tab', { name: 'sales', exact: true })).toHaveAttribute(
    'aria-selected',
    'true',
  );
  const pane = page.getByTestId('queries-pane');
  await expect(pane.getByTestId('query-item-0')).toContainText('sales');
  await expect(pane.getByTestId('query-info-0')).toContainText('4 dòng');
  await expect(await inputOf(page, 'A1')).toHaveValue('Mã');
  await expect(await inputOf(page, 'D3')).toHaveValue('4');
  await expect(await inputOf(page, 'A6')).toHaveValue('');

  // The source file changes; Refresh asks for it again (the browser cannot re-read it).
  const edited =
    salesCsv.replace('A02;Lê;Nam;4;', 'A02;Lê;Nam;40;') + 'A05;Xoài;Nam;6;9;2024-03-01\r\n';
  await pane.getByTestId('query-refresh-0').click();
  const reload = page.getByTestId('query-reload');
  await expect(reload).toBeVisible();
  const again = page.waitForEvent('filechooser');
  await reload.getByTestId('query-reload-pick-0').click();
  await (
    await again
  ).setFiles({ name: 'sales.csv', mimeType: 'text/csv', buffer: Buffer.from(edited, 'utf8') });
  await expect(reload).toContainText('Đã chọn lại');
  await reload.getByTestId('query-reload-ok').click();
  await expect(reload).toBeHidden();
  await expect(pane.getByTestId('query-info-0')).toContainText('5 dòng');
  await expect(await inputOf(page, 'D3')).toHaveValue('40');
  await expect(await inputOf(page, 'A6')).toHaveValue('A05');

  // A formula on the table follows the refreshed data.
  await page.getByRole('tab', { name: 'Sheet1', exact: true }).click();
  await goTo(page, 'B2');
  await page.keyboard.type('=SUM(sales[Số lượng])');
  await page.keyboard.press('Enter');
  await expect(await inputOf(page, 'B2')).toHaveValue('=SUM(sales[Số lượng])');
  expect(errors).toEqual([]);
});
