import { expect, test } from '@playwright/test';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const file = join(
  dirname(fileURLToPath(import.meta.url)),
  '..',
  '..',
  'corpus',
  'sheet',
  'timeline-1.xlsx',
);

test('BlueSheet: a timeline filters its pivot table by period', async ({ page }) => {
  const errors: string[] = [];
  page.on('pageerror', (e) => errors.push(e.message));
  await page.goto('/sheet');
  const root = page.locator('[data-app="sheet"]');
  await expect(root).toHaveAttribute('aria-busy', 'false');
  const seq = Number(await root.getAttribute('data-load-seq'));
  const chooser = page.waitForEvent('filechooser');
  await page.locator('[data-command="file.open"]').click();
  await (await chooser).setFiles(file);
  await expect
    .poll(async () => Number(await root.getAttribute('data-load-seq')))
    .toBeGreaterThan(seq);
  await expect(root).toHaveAttribute('aria-busy', 'false');
  // The timeline is on the report sheet.
  await page.getByRole('tab', { name: 'Báo cáo' }).click();
  const tl = page.getByTestId('timeline');
  await expect(tl).toBeVisible();
  await expect(tl).toContainText('Quý');
  // The file selects Q2 2023.
  await expect(tl.getByRole('button', { name: 'Quý 2 2023' })).toHaveAttribute('aria-pressed', 'true');
  await expect(tl.getByRole('button', { name: 'Quý 3 2023' })).toHaveAttribute('aria-pressed', 'false');
  // Choose Q1 2024, then extend to Q2 2024 with Shift.
  await tl.getByRole('button', { name: 'Quý 1 2024' }).click();
  await expect(tl.getByRole('button', { name: 'Quý 1 2024' })).toHaveAttribute('aria-pressed', 'true');
  await expect(tl.getByRole('button', { name: 'Quý 2 2023' })).toHaveAttribute('aria-pressed', 'false');
  await tl.getByRole('button', { name: 'Quý 2 2024' }).click({ modifiers: ['Shift'] });
  await expect(tl.getByRole('button', { name: 'Quý 2 2024' })).toHaveAttribute('aria-pressed', 'true');
  await expect(tl.getByRole('button', { name: 'Quý 1 2024' })).toHaveAttribute('aria-pressed', 'true');
  // Clear: nothing is filtered.
  await tl.getByTestId('timeline-clear').click();
  await expect(tl.getByRole('button', { name: 'Quý 1 2024' })).toHaveAttribute('aria-pressed', 'false');
  await expect(tl.getByTestId('timeline-clear')).toBeDisabled();
  expect(errors).toEqual([]);
});
