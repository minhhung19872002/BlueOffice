import { expect, test, type Page } from '@playwright/test';

async function goTo(page: Page, ref: string) {
  const box = page.getByLabel('Hộp tên');
  await box.click();
  await box.fill(ref);
  await box.press('Enter');
  await expect(page.getByLabel('Ô đang nhập')).toBeFocused();
}

test('BlueSheet: Goal Seek finds an input and applies it as one undo step', async ({ page }) => {
  const errors: string[] = [];
  page.on('pageerror', (e) => errors.push(e.message));
  await page.goto('/sheet');
  await expect(page.getByTestId('sheet-canvas')).toHaveAttribute('data-rendered', /\d+/);
  // A1 = 1, B1 = 2*A1+3.
  await page.keyboard.type('1');
  await page.keyboard.press('Tab');
  await page.keyboard.type('=2*A1+3');
  await page.keyboard.press('Enter');
  await goTo(page, 'B1');
  await page.getByRole('tab', { name: 'Dữ liệu' }).click();
  await page.locator('[data-command="data.goalSeek"]').click();
  await expect(page.getByTestId('goalseek-dialog')).toBeVisible();
  await expect(page.getByTestId('goalseek-target')).toHaveValue('B1');
  await page.getByTestId('goalseek-goal').fill('11');
  await page.getByTestId('goalseek-changing').fill('A1');
  await page.getByTestId('goalseek-find').click();
  await expect(page.getByTestId('goalseek-result')).toContainText('Đã tìm thấy');
  await page.getByTestId('goalseek-apply').click();
  await expect(page.getByTestId('goalseek-dialog')).toHaveCount(0);
  await goTo(page, 'A1');
  await expect(page.getByLabel('Thanh công thức')).toHaveValue('4');
  // Undo restores the input.
  await page.keyboard.press('Control+z');
  await goTo(page, 'A1');
  await expect(page.getByLabel('Thanh công thức')).toHaveValue('1');
  expect(errors).toEqual([]);
});
