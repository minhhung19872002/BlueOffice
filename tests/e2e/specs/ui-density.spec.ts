import { expect, test } from '@playwright/test';

test('density: touch makes ribbon buttons ≥ 40px targets, compact makes them smaller', async ({
  page,
}) => {
  await page.goto('/doc');
  await page.getByRole('tab', { name: 'Xem', exact: true }).click();
  const button = page.getByRole('button', { name: 'Mật độ: Thường' });
  const heightOf = () =>
    page.evaluate(() => {
      const els = Array.from(document.querySelectorAll('[role="tabpanel"] button'));
      return Math.min(...els.map((e) => e.getBoundingClientRect().height).filter((h) => h > 0));
    });
  const normal = await heightOf();
  await button.click();
  await expect(page.locator('html')).toHaveAttribute('data-density', 'compact');
  const compact = await heightOf();
  await page.getByRole('button', { name: 'Mật độ: Gọn' }).click();
  await expect(page.locator('html')).toHaveAttribute('data-density', 'touch');
  const touch = await heightOf();
  expect(compact).toBeLessThan(normal);
  expect(touch).toBeGreaterThanOrEqual(40);
  // Remembered across reloads.
  await page.reload();
  await expect(page.locator('html')).toHaveAttribute('data-density', 'touch');
});
