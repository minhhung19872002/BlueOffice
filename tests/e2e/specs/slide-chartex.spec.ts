import { expect, test, type Page } from '@playwright/test';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

// CORE-CHART-004: chartex charts ([MS-ODRAWXML] 2.24) made, edited and kept on a slide.
const here = dirname(fileURLToPath(import.meta.url));
const deck = join(here, '..', '..', 'corpus', 'slide', 'slide-text-bullets.pptx');

async function openDeck(page: Page, path: string) {
  const chooser = page.waitForEvent('filechooser');
  await page.getByRole('button', { name: 'Mở', exact: true }).click();
  await (await chooser).setFiles(path);
  await expect(page.getByTestId('slide-status')).toHaveText(/^Slide 1 \/ \d+$/);
  await expect(page.getByTestId('slide-overlay')).toBeVisible();
}

test('inserts a waterfall chart on a slide, edits it, saves and reopens it as a chart', async ({
  page,
}) => {
  const errors: string[] = [];
  page.on('pageerror', (e) => errors.push(e.message));
  await page.goto('/slide');
  await openDeck(page, deck);
  await page.getByRole('button', { name: '+ Thêm slide' }).click();
  await expect(page.getByTestId('slide-status')).toHaveText(/^Slide 2 \/ \d+$/);

  await page.getByRole('tab', { name: 'Chèn' }).click();
  await page.getByRole('button', { name: 'Biểu đồ', exact: true }).click();
  await expect(page.getByTestId('chart-dialog')).toBeVisible();
  await page.getByTestId('chart-kind').selectOption('waterfall');
  await page.getByTestId('chart-title').fill('Dòng tiền');
  await page.getByTestId('chart-ok').click();
  await expect(page.getByTestId('chart-dialog')).toBeHidden();
  await expect(page.getByTestId('selection-box').first()).toBeVisible();

  // The chart tab edits the selected chartex chart: same type, new title.
  await page.getByRole('tab', { name: 'Biểu đồ' }).click();
  await page.getByText('Sửa biểu đồ').click();
  await expect(page.getByTestId('chart-dialog')).toBeVisible();
  await expect(page.getByTestId('chart-kind')).toHaveValue('waterfall');
  await expect(page.getByTestId('chart-title')).toHaveValue('Dòng tiền');
  await page.getByTestId('chart-kind').selectOption('funnel');
  await page.getByTestId('chart-title').fill('Phễu bán hàng');
  await page.getByTestId('chart-ok').click();
  await expect(page.getByTestId('chart-dialog')).toBeHidden();

  // Saved and reopened: a chart again (selectable, the chart tab edits it).
  const download = page.waitForEvent('download');
  await page.keyboard.press('Control+s');
  const saved = await (await download).path();
  await page.reload();
  await openDeck(page, saved);
  await page.getByTestId('thumb-2').click();
  await expect(page.getByTestId('slide-status')).toHaveText(/^Slide 2 \//);
  const o = await page.getByTestId('slide-overlay').boundingBox();
  expect(o).not.toBeNull();
  await page.mouse.click(o!.x + o!.width / 2, o!.y + o!.height / 2);
  await page.getByRole('tab', { name: 'Biểu đồ' }).click();
  await page.getByText('Sửa biểu đồ').click();
  await expect(page.getByTestId('chart-kind')).toHaveValue('funnel');
  await expect(page.getByTestId('chart-title')).toHaveValue('Phễu bán hàng');
  expect(errors).toEqual([]);
});
