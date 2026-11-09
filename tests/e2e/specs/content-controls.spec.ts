import { expect, test } from '@playwright/test';

const cmd = (page: import('@playwright/test').Page, id: string) =>
  page.locator(`[data-command="${id}"]`);

test('inserts content controls and fills a check box, a list and a date', async ({ page }) => {
  const errors: string[] = [];
  page.on('pageerror', (e) => errors.push(e.message));
  await page.goto('/doc');
  await page.getByTestId('new-document').click();
  await expect(page.getByTestId('caret')).toBeVisible();
  await page.getByRole('tab', { name: 'Chèn' }).click();
  // A check box: the bar toggles it.
  await cmd(page, 'insert.control').click();
  await expect(page.getByTestId('control-dialog')).toBeVisible();
  await page.getByTestId('control-kind').selectOption('checkbox');
  await expect(page.getByTestId('control-placeholder')).toHaveCount(0);
  await page.getByTestId('control-ok').click();
  await expect(page.getByTestId('control-bar')).toBeVisible();
  await expect(page.getByTestId('control-kind-label')).toHaveText('Hộp kiểm');
  await expect(page.getByTestId('control-toggle')).toHaveAttribute('aria-pressed', 'false');
  await page.getByTestId('control-toggle').click();
  await expect(page.getByTestId('control-toggle')).toHaveAttribute('aria-pressed', 'true');
  // Typed after the check box, the text is outside it.
  await page.keyboard.press('End');
  await page.keyboard.insertText(' Loại ');
  await expect(page.getByTestId('control-bar')).toHaveCount(0);
  // A drop-down list with two items: picking one replaces the placeholder.
  await cmd(page, 'insert.control').click();
  await page.getByTestId('control-kind').selectOption('dropDown');
  await expect(page.getByTestId('control-placeholder')).toHaveValue('Chọn một mục.');
  await page.getByTestId('control-items').fill('Hợp đồng\nPhụ lục');
  await page.getByTestId('control-ok').click();
  await expect(page.getByTestId('control-kind-label')).toHaveText('Danh sách thả xuống');
  await expect(page.getByTestId('status-words')).toHaveText('5 từ');
  await page.getByTestId('control-choose').selectOption('Phụ lục');
  await expect(page.getByTestId('status-words')).toHaveText('4 từ');
  // A date picker.
  await page.keyboard.press('End');
  await page.keyboard.insertText(' ngày ');
  await cmd(page, 'insert.control').click();
  await page.getByTestId('control-kind').selectOption('date');
  await page.getByTestId('control-ok').click();
  await page.getByTestId('control-date').fill('2026-11-09');
  await expect(page.getByTestId('control-date')).toHaveValue('2026-11-09');
  expect(errors).toEqual([]);
});
