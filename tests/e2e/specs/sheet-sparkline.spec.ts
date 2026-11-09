import { expect, test, type Page } from '@playwright/test';

async function goTo(page: Page, ref: string) {
  const box = page.getByLabel('Hộp tên');
  await box.click();
  await box.fill(ref);
  await box.press('Enter');
  await expect(page.getByLabel('Ô đang nhập')).toBeFocused();
}

test('BlueSheet: insert sparklines, change their type and clear them (undoable)', async ({
  page,
}) => {
  const errors: string[] = [];
  page.on('pageerror', (e) => errors.push(e.message));
  await page.goto('/sheet');
  await expect(page.getByTestId('sheet-canvas')).toHaveAttribute('data-rendered', /\d+/);
  const rows = [
    ['5', '8', '3', '9'],
    ['-2', '4', '-1', '6'],
  ];
  for (const [i, row] of rows.entries()) {
    await goTo(page, `A${i + 1}`);
    for (const v of row) {
      await page.keyboard.type(v);
      await page.keyboard.press('Tab');
    }
  }
  // Select the data, then Insert › Sparklines proposes the column on its right.
  await goTo(page, 'A1:D2');
  await page.getByRole('tab', { name: 'Chèn' }).click();
  await page.locator('[data-command="insert.sparklines"]').click();
  const dlg = page.getByTestId('sparkline-insert');
  await expect(dlg).toBeVisible();
  await expect(dlg.getByTestId('sparkline-data')).toHaveValue('A1:D2');
  await expect(dlg.getByTestId('sparkline-location')).toHaveValue('E1:E2');
  await dlg.getByTestId('sparkline-type-column').check();
  await dlg.getByTestId('sparkline-insert-ok').click();
  await expect(dlg).toBeHidden();

  // The contextual tab appears on a sparkline cell.
  await goTo(page, 'E2');
  const tab = page.getByRole('tab', { name: 'Biểu đồ thu nhỏ' });
  await expect(tab).toBeVisible();
  await tab.click();
  const column = page.locator('[data-command="sparkline.typeColumn"]');
  await expect(column).toHaveAttribute('aria-pressed', 'true');
  await page.locator('[data-command="sparkline.typeWinLoss"]').click();
  await expect(page.locator('[data-command="sparkline.typeWinLoss"]')).toHaveAttribute(
    'aria-pressed',
    'true',
  );
  await page.locator('[data-command="sparkline.negative"]').click();
  await expect(page.locator('[data-command="sparkline.negative"]')).toHaveAttribute(
    'aria-pressed',
    'true',
  );

  // Clear the group: the tab goes away; undo brings it back.
  await page.locator('[data-command="sparkline.clearGroup"]').click();
  await expect(tab).toBeHidden();
  await page.getByLabel('Ô đang nhập').focus();
  await page.keyboard.press('Control+z');
  await expect(tab).toBeVisible();
  expect(errors).toEqual([]);
});
