import { expect, test, type Page } from '@playwright/test';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const here = dirname(fileURLToPath(import.meta.url));

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

async function pivotCommand(page: Page, id: string) {
  await page.getByRole('tab', { name: 'Bảng tổng hợp', exact: true }).click();
  await page.locator(`[data-command="${id}"]`).click();
}

async function saveAndReopen(page: Page) {
  await page.getByLabel('Ô đang nhập').focus();
  const download = page.waitForEvent('download');
  await page.keyboard.press('Control+s');
  const path = await (await download).path();
  await page.reload();
  await expect(page.getByRole('tab', { name: 'Sheet1' })).toBeVisible();
  const chooser = page.waitForEvent('filechooser');
  await page.locator('[data-command="file.open"]').click();
  await (await chooser).setFiles(path);
}

test('BlueSheet: typed dates, manual groups, calculated item, date filter and summary chart survive save', async ({
  page,
}) => {
  const errors: string[] = [];
  page.on('pageerror', (e) => errors.push(e.message));
  await page.goto('/sheet');
  await expect(page.getByTestId('sheet-canvas')).toHaveAttribute('data-rendered', /\d+/);

  // Dates typed as dd/mm/yyyy become dates (formula bar shows the date).
  const rows = [
    ['Ngày', 'Vùng', 'Doanh thu'],
    ['15/01/2024', 'Bắc', '10'],
    ['20/01/2024', 'Nam', '30'],
    ['10/02/2024', 'Bắc', '20'],
    ['05/03/2024', 'Nam', '40'],
  ];
  for (const [i, row] of rows.entries()) {
    await goTo(page, `A${i + 1}`);
    for (const v of row) {
      await page.keyboard.type(v);
      await page.keyboard.press('Tab');
    }
  }
  await expect(await valueOf(page, 'A2')).toHaveValue('15/01/2024');

  await goTo(page, 'B2');
  await page.getByRole('tab', { name: 'Chèn' }).click();
  await page.locator('[data-command="insert.pivot"]').click();
  await page.getByTestId('pivot-ok').click();
  await expect(page.getByRole('tab', { name: 'Sheet2' })).toHaveAttribute('aria-selected', 'true');
  await page.getByTestId('pivot-field-1').check();
  await page.getByTestId('pivot-field-2').check();
  await expect(await valueOf(page, 'A4')).toHaveValue('Bắc');

  // Group Bắc + Nam, then ungroup.
  await goTo(page, 'A4:A5');
  await pivotCommand(page, 'pivot.groupSelection');
  await expect(await valueOf(page, 'A4')).toHaveValue('Nhóm 1');
  await expect(await valueOf(page, 'B4')).toHaveValue('Bắc');
  await goTo(page, 'A4');
  await pivotCommand(page, 'pivot.ungroupSelection');
  await expect(await valueOf(page, 'A4')).toHaveValue('Bắc');

  // Calculated item "Cả hai" = Bắc + Nam.
  await pivotCommand(page, 'pivot.calculatedItem');
  await expect(page.getByTestId('pivot-calc-item-dialog')).toBeVisible();
  await page.getByTestId('pivot-calc-item-name').fill('Cả hai');
  await page.getByTestId('pivot-calc-item-formula').fill('=Bắc+Nam');
  await page.getByTestId('pivot-calc-item-ok').click();
  await expect(await valueOf(page, 'A6')).toHaveValue('Cả hai');
  await expect(await valueOf(page, 'B6')).toHaveValue('100');

  // Summary chart of the pivot table.
  await pivotCommand(page, 'pivot.chart');
  const sel = page.getByTestId('chart-selection');
  await expect(sel).toBeVisible();
  const box = await sel.boundingBox();
  expect(box).not.toBeNull();

  // Dates on rows with a date filter (January 2024): 10 + 30.
  await goTo(page, 'A3');
  await page.getByTestId('pivot-field-1').uncheck();
  await page.getByTestId('pivot-field-0').check();
  await pivotCommand(page, 'pivot.fieldSettings');
  await expect(page.getByTestId('pivot-field-dialog')).toBeVisible();
  await page.getByTestId('pivot-filter-mode').selectOption('date');
  await page.getByTestId('pivot-date-filter').selectOption('dateBetween');
  await page.getByTestId('pivot-date-value1').fill('2024-01-01');
  await page.getByTestId('pivot-date-value2').fill('2024-01-31');
  await page.getByTestId('pivot-field-ok').click();
  await expect(await valueOf(page, 'A6')).toHaveValue('Tổng cộng');
  await expect(await valueOf(page, 'B6')).toHaveValue('40');

  await saveAndReopen(page);
  await expect(await valueOf(page, 'A2')).toHaveValue('15/01/2024');
  await page.getByRole('tab', { name: 'Sheet2' }).click();
  await expect(await valueOf(page, 'B6')).toHaveValue('40');
  // The chart is still there (click inside its box).
  if (box) await page.mouse.click(box.x + box.width / 2, box.y + box.height / 2);
  await expect(page.getByTestId('chart-selection')).toBeVisible();
  expect(errors).toEqual([]);
});

test('BlueSheet: a chart in an absolute anchor is resized and keeps its size after reopening', async ({
  page,
}) => {
  const errors: string[] = [];
  page.on('pageerror', (e) => errors.push(e.message));
  await page.goto('/sheet');
  await expect(page.getByTestId('sheet-canvas')).toHaveAttribute('data-rendered', /\d+/);
  const chooser = page.waitForEvent('filechooser');
  await page.locator('[data-command="file.open"]').click();
  await (await chooser).setFiles(join(here, '..', '..', 'corpus', 'sheet', 'chart-anchors-2.xlsx'));
  await expect(page.getByTestId('sheet-canvas')).toHaveAttribute('data-rendered', /\d+/);
  const canvas = (await page.getByTestId('sheet-canvas').boundingBox())!;
  const sel = page.getByTestId('chart-selection');
  // Find the chart: click across the sheet until it is selected.
  let hit: { x: number; y: number } | null = null;
  for (let y = canvas.y + 120; y < canvas.y + 320 && !hit; y += 40) {
    for (let x = canvas.x + 260; x < canvas.x + 700 && !hit; x += 40) {
      await page.mouse.click(x, y);
      if (await sel.isVisible()) hit = { x, y };
    }
  }
  expect(hit).not.toBeNull();
  const before = (await sel.boundingBox())!;
  const handle = (await page.getByTestId('chart-resize').boundingBox())!;
  await page.mouse.move(handle.x + handle.width / 2, handle.y + handle.height / 2);
  await page.mouse.down();
  await page.mouse.move(handle.x + 120, handle.y + 90, { steps: 5 });
  await page.mouse.up();
  await expect
    .poll(async () => (await sel.boundingBox())?.width ?? 0)
    .toBeGreaterThan(before.width + 40);
  const after = (await sel.boundingBox())!;
  await saveAndReopen(page);
  await expect(page.getByTestId('sheet-canvas')).toHaveAttribute('data-rendered', /\d+/);
  await page.mouse.click(hit!.x, hit!.y);
  await expect(sel).toBeVisible();
  const reopened = (await sel.boundingBox())!;
  expect(Math.abs(reopened.width - after.width)).toBeLessThan(2);
  expect(Math.abs(reopened.height - after.height)).toBeLessThan(2);
  expect(errors).toEqual([]);
});
