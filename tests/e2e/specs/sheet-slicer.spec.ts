import { expect, test, type Page } from '@playwright/test';

/** Insert › Table asks for the range and the header row first (Create Table dialog). */
async function confirmCreateTable(page: Page) {
  const dialog = page.getByRole('dialog', { name: 'Tạo bảng' });
  await dialog.getByRole('button', { name: 'Đồng ý' }).click();
  await expect(dialog).toBeHidden();
}

async function goTo(page: Page, ref: string) {
  const box = page.getByLabel('Hộp tên');
  await box.click();
  await box.fill(ref);
  await box.press('Enter');
  await expect(page.getByLabel('Ô đang nhập')).toBeFocused();
}

test('BlueSheet: a table slicer filters the table rows', async ({ page }) => {
  const errors: string[] = [];
  page.on('pageerror', (e) => errors.push(e.message));
  await page.goto('/sheet');
  await expect(page.getByTestId('sheet-canvas')).toHaveAttribute('data-rendered', /\d+/);
  const rows = [
    ['Vùng', 'Số'],
    ['Bắc', '1'],
    ['Nam', '2'],
    ['Bắc', '3'],
  ];
  for (const [i, [a, b]] of rows.entries()) {
    await goTo(page, `A${i + 1}`);
    await page.keyboard.type(a);
    await page.keyboard.press('Tab');
    await page.keyboard.type(b);
    await page.keyboard.press('Enter');
  }
  // Visible total of the numbers (SUBTOTAL 109 skips hidden rows).
  await goTo(page, 'D6');
  await page.keyboard.type('=SUBTOTAL(109,B2:B4)');
  await page.keyboard.press('Enter');
  await goTo(page, 'A1:B4');
  await page.getByRole('tab', { name: 'Chèn' }).click();
  await page.locator('[data-command="insert.table"]').click();
  await confirmCreateTable(page);
  await goTo(page, 'A2');
  await page.locator('[data-command="insert.slicer"]').click();
  const slicer = page.getByTestId('slicer');
  await expect(slicer).toBeVisible();
  await expect(slicer.getByRole('button', { name: 'Bắc' })).toHaveAttribute('aria-pressed', 'true');
  // Choosing "Nam" filters out the Bắc rows.
  await slicer.getByRole('button', { name: 'Nam' }).click();
  await expect(slicer.getByRole('button', { name: 'Bắc' })).toHaveAttribute(
    'aria-pressed',
    'false',
  );
  // (One filled cell shows no statistics in the status bar, so D6 is read from the
  // announcement of the active cell.)
  await goTo(page, 'D6');
  await expect(page.getByTestId('sheet-a11y-live')).toContainText('D6: 2,');
  // Ctrl+click adds Bắc back; the clear button shows every row.
  await slicer.getByRole('button', { name: 'Bắc' }).click({ modifiers: ['Control'] });
  await expect(slicer.getByRole('button', { name: 'Bắc' })).toHaveAttribute('aria-pressed', 'true');
  await slicer.getByTestId('slicer-clear').click({ force: true });
  await goTo(page, 'D7');
  await goTo(page, 'D6');
  await expect(page.getByTestId('sheet-a11y-live')).toContainText('D6: 6,');
  expect(errors).toEqual([]);
});
