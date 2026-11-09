import { expect, test } from '@playwright/test';

// CORE-SYMBOL-001: the shared symbol picker — search by a Vietnamese keyword and by code point,
// insert into a BlueSheet cell, and remember the recently used character.
test('BlueSheet: inserts symbols found by keyword and by code point', async ({ page }) => {
  const errors: string[] = [];
  page.on('pageerror', (e) => errors.push(e.message));
  await page.goto('/sheet');
  await expect(page.getByTestId('sheet-canvas')).toHaveAttribute('data-rendered', /\d+/);
  await page.keyboard.type('Giá ');

  await page.getByRole('tab', { name: 'Chèn' }).click();
  await page.locator('[data-command="insert.symbol"]').click();
  const picker = page.getByTestId('symbol-picker');
  await expect(picker).toBeVisible();
  // "dong" (accents ignored) finds the đồng sign U+20AB.
  await picker.getByTestId('symbol-search').fill('dong');
  const dong = picker.getByTestId('symbol-grid').getByRole('gridcell', { name: /U\+20AB/ });
  await expect(dong).toBeVisible();
  await dong.dblclick();
  // By code point: U+00B0 degree sign.
  await picker.getByTestId('symbol-search').fill('U+00B0');
  const degree = picker.getByTestId('symbol-grid').getByRole('gridcell', { name: /U\+00B0/ });
  await degree.dblclick();
  // The recent row now offers both characters.
  await picker.getByTestId('symbol-search').fill('');
  await expect(picker.getByTestId('symbol-recent')).toHaveCount(2);
  await page.keyboard.press('Escape');
  await expect(picker).toHaveCount(0);

  await page.keyboard.press('Enter');
  const box = page.getByLabel('Hộp tên');
  await box.click();
  await box.fill('A1');
  await box.press('Enter');
  await expect(page.getByLabel('Thanh công thức')).toHaveValue('Giá ₫°');
  expect(errors).toEqual([]);
});
