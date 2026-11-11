import { expect, test, type Page } from '@playwright/test';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

// Pivot table family (SHEET-PIVOT-001/002, SHEET-SLICER-001, SHEET-TIMELINE-001): drill-down,
// pivot table slicers (insert, filter, options, move, save, delete), timelines (level, range,
// insert, delete) and the filter buttons of a pivot chart.

const corpus = join(dirname(fileURLToPath(import.meta.url)), '..', '..', 'corpus', 'sheet');

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

async function openFile(page: Page, path: string) {
  const root = page.locator('[data-app="sheet"]');
  await expect(root).toHaveAttribute('aria-busy', 'false');
  const seq = Number(await root.getAttribute('data-load-seq'));
  const chooser = page.waitForEvent('filechooser');
  await page.locator('[data-command="file.open"]').click();
  await (await chooser).setFiles(path);
  await expect
    .poll(async () => Number(await root.getAttribute('data-load-seq')))
    .toBeGreaterThan(seq);
  await expect(root).toHaveAttribute('aria-busy', 'false');
}

test('BlueSheet: drill-down and a pivot table slicer (insert, filter, options, move, save, delete)', async ({
  page,
}) => {
  const errors: string[] = [];
  page.on('pageerror', (e) => errors.push(e.message));
  await page.goto('/sheet');
  await expect(page.getByTestId('sheet-canvas')).toHaveAttribute('data-rendered', /\d+/);
  // Kind A: 10 + 2 + 1 + 3 = 16; kind B: 5 + 7 = 12; Nam: A 1, B 5.
  const rows = [
    ['Vùng', 'Loại', 'Tiền'],
    ['Bắc', 'A', '10'],
    ['Nam', 'B', '5'],
    ['Bắc', 'B', '7'],
    ['Trung', 'A', '2'],
    ['Nam', 'A', '1'],
    ['Bắc', 'A', '3'],
  ];
  for (const [i, row] of rows.entries()) {
    await goTo(page, `A${i + 1}`);
    for (const v of row) {
      await page.keyboard.type(v);
      await page.keyboard.press('Tab');
    }
  }
  await goTo(page, 'B2');
  await page.getByRole('tab', { name: 'Chèn' }).click();
  await page.locator('[data-command="insert.pivot"]').click();
  await page.getByTestId('pivot-ok').click();
  await expect(page.getByRole('tab', { name: 'Sheet2' })).toHaveAttribute('aria-selected', 'true');
  await page.getByTestId('pivot-field-1').check();
  await page.getByTestId('pivot-field-2').check();
  await expect(await valueOf(page, 'A4')).toHaveValue('A');
  await expect(await valueOf(page, 'B4')).toHaveValue('16');
  await expect(await valueOf(page, 'B6')).toHaveValue('28');

  // Show details of kind B: its two source rows on a new sheet.
  await goTo(page, 'B5');
  await pivotCommand(page, 'pivot.showDetails');
  await expect(page.getByRole('tab', { name: 'Chi tiết1' })).toHaveAttribute(
    'aria-selected',
    'true',
  );
  await expect(await valueOf(page, 'A1')).toHaveValue('Vùng');
  await expect(await valueOf(page, 'A2')).toHaveValue('Nam');
  await expect(await valueOf(page, 'C2')).toHaveValue('5');
  await expect(await valueOf(page, 'A3')).toHaveValue('Bắc');
  await expect(await valueOf(page, 'C3')).toHaveValue('7');
  await expect(await valueOf(page, 'A4')).toHaveValue('');
  await page.getByRole('tab', { name: 'Sheet2' }).click();
  // Double-clicking a value cell does the same: the grand total lists all six rows.
  await goTo(page, 'B6');
  const cell = await page.getByLabel('Ô đang nhập').boundingBox();
  expect(cell).not.toBeNull();
  if (cell) await page.mouse.dblclick(cell.x + 6, cell.y + 6);
  await expect(page.getByRole('tab', { name: 'Chi tiết2' })).toHaveAttribute(
    'aria-selected',
    'true',
  );
  await expect(await valueOf(page, 'C7')).toHaveValue('3');
  await expect(await valueOf(page, 'A8')).toHaveValue('');
  await page.getByRole('tab', { name: 'Sheet2' }).click();

  // A slicer over "Vùng", which is not in the report.
  await goTo(page, 'A4');
  await page.getByRole('tab', { name: 'Chèn' }).click();
  await page.locator('[data-command="insert.slicer"]').click();
  await expect(page.getByTestId('frame-insert-dialog')).toBeVisible();
  await page.getByTestId('frame-field-0').check();
  await page.getByTestId('frame-insert-ok').click();
  const slicer = page.getByTestId('slicer');
  await expect(slicer).toBeVisible();
  await expect(slicer.getByRole('button', { name: 'Trung' })).toHaveAttribute(
    'aria-pressed',
    'true',
  );
  await slicer.getByRole('button', { name: 'Nam' }).click();
  await expect(slicer.getByRole('button', { name: 'Bắc' })).toHaveAttribute(
    'aria-pressed',
    'false',
  );
  await expect(await valueOf(page, 'B4')).toHaveValue('1');
  await expect(await valueOf(page, 'B5')).toHaveValue('5');
  await expect(await valueOf(page, 'B6')).toHaveValue('6');

  // Options: caption, two columns, a dark style.
  await slicer.getByTestId('slicer-settings').click();
  await expect(page.getByTestId('frame-options-dialog')).toBeVisible();
  await page.getByTestId('frame-caption').fill('Khu vực');
  await page.getByTestId('frame-columns').fill('2');
  await page.getByTestId('frame-style').selectOption('SlicerStyleDark2');
  await page.getByTestId('frame-options-ok').click();
  await expect(slicer).toHaveAttribute('data-style', 'SlicerStyleDark2');
  await expect(slicer.getByTestId('slicer-header')).toContainText('Khu vực');

  // Drag the header: the frame moves right by whole columns.
  const before = await slicer.boundingBox();
  expect(before).not.toBeNull();
  const head = await slicer.getByTestId('slicer-header').boundingBox();
  expect(head).not.toBeNull();
  if (before && head) {
    await page.mouse.move(head.x + 20, head.y + head.height / 2);
    await page.mouse.down();
    await page.mouse.move(head.x + 120, head.y + head.height / 2, { steps: 6 });
    await page.mouse.move(head.x + 220, head.y + head.height / 2, { steps: 6 });
    await page.mouse.up();
    await expect
      .poll(async () => (await slicer.boundingBox())?.x ?? 0)
      .toBeGreaterThan(before.x + 100);
  }

  // Saved and opened again: the slicer comes back with its choice and options.
  await page.getByLabel('Ô đang nhập').focus();
  const download = page.waitForEvent('download');
  await page.keyboard.press('Control+s');
  const path = await (await download).path();
  await page.reload();
  await expect(page.getByRole('tab', { name: 'Sheet1' })).toBeVisible();
  await openFile(page, path);
  await page.getByRole('tab', { name: 'Sheet2' }).click();
  const again = page.getByTestId('slicer');
  await expect(again).toBeVisible();
  await expect(again).toHaveAttribute('data-style', 'SlicerStyleDark2');
  await expect(again.getByRole('button', { name: 'Nam' })).toHaveAttribute('aria-pressed', 'true');
  await expect(again.getByRole('button', { name: 'Bắc' })).toHaveAttribute('aria-pressed', 'false');
  await expect(await valueOf(page, 'B6')).toHaveValue('6');
  // Deleting the slicer read from the file removes its filter (selected first).
  await again.getByTestId('slicer-header').click();
  await again.getByTestId('slicer-delete').click();
  await expect(page.getByTestId('slicer')).toHaveCount(0);
  await expect(await valueOf(page, 'B6')).toHaveValue('28');
  expect(errors).toEqual([]);
});

test('BlueSheet: timeline levels, a new timeline and pivot chart filter buttons', async ({
  page,
}) => {
  const errors: string[] = [];
  page.on('pageerror', (e) => errors.push(e.message));
  await page.goto('/sheet');
  // Revenue by region over 30 rows dated every 23 days from 2023-03-15: Bắc 5450, Nam 5850,
  // Trung 5150 (16450); the rows of 2024 alone: Bắc 4100, Nam 2250, Trung 2810 (9160).
  await openFile(page, join(corpus, 'writer-timelines-1.xlsx'));
  await page.getByRole('tab', { name: 'Báo cáo' }).click();
  await expect(await valueOf(page, 'A2')).toHaveValue('Bắc');
  await expect(await valueOf(page, 'B5')).toHaveValue('16450');
  const tl = page.getByTestId('timeline');
  await expect(tl).toBeVisible();
  await expect(tl).toHaveAttribute('data-level', '2');
  await expect(tl.getByTestId('timeline-range')).toHaveText('Mọi kỳ');
  // Years, then 2024.
  await tl.getByTestId('timeline-level').selectOption('0');
  await expect(tl).toHaveAttribute('data-level', '0');
  await tl.getByRole('button', { name: '2024' }).click();
  await expect(tl.getByRole('button', { name: '2024' })).toHaveAttribute('aria-pressed', 'true');
  await expect(tl.getByTestId('timeline-range')).toHaveText('2024');
  await expect(await valueOf(page, 'B2')).toHaveValue('4100');
  await expect(await valueOf(page, 'B5')).toHaveValue('9160');
  // Zooming in shows quarters; the selection stays.
  await tl.getByTestId('timeline-zoom-in').click();
  await expect(tl).toHaveAttribute('data-level', '1');
  await expect(tl.getByRole('button', { name: 'Quý 3 2024' })).toHaveAttribute(
    'aria-pressed',
    'true',
  );
  await expect(tl.getByRole('button', { name: 'Quý 1 2025' })).toHaveAttribute(
    'aria-pressed',
    'false',
  );

  // Pivot chart: its "Vùng" button hides Bắc in the pivot table (2024 without Bắc: 5060).
  await goTo(page, 'A2');
  await pivotCommand(page, 'pivot.chart');
  await expect(page.getByTestId('chart-selection')).toBeVisible();
  const filters = page.getByTestId('pivot-chart-filters');
  await expect(filters).toBeVisible();
  await filters.getByTestId('pivot-chart-filter-0').click();
  const menu = page.getByTestId('pivot-chart-filter-menu');
  await expect(menu).toBeVisible();
  await menu.getByLabel('Bắc').uncheck();
  await page.getByTestId('pivot-chart-filter-ok').click();
  await expect(menu).toHaveCount(0);
  await expect(await valueOf(page, 'A2')).toHaveValue('Nam');
  await expect(await valueOf(page, 'B4')).toHaveValue('5060');

  // Delete the timeline of the file: the date filter goes (all years, without Bắc: 11000).
  await page.getByTestId('timeline').getByTestId('timeline-delete').click();
  await expect(page.getByTestId('timeline')).toHaveCount(0);
  await expect(await valueOf(page, 'B4')).toHaveValue('11000');
  // A new timeline over the date field.
  await goTo(page, 'A2');
  await pivotCommand(page, 'insert.timeline');
  await expect(page.getByTestId('frame-insert-dialog')).toBeVisible();
  await page.getByTestId('frame-field-2').check();
  await page.getByTestId('frame-insert-ok').click();
  const fresh = page.getByTestId('timeline');
  await expect(fresh).toBeVisible();
  await expect(fresh).toHaveAttribute('data-level', '2');
  // April 2023 holds two rows (2023-04-07 Trung 420, 2023-04-30 Nam 790); Bắc stays hidden.
  await fresh.getByRole('button', { name: '04/2023' }).click();
  await expect(fresh.getByTestId('timeline-range')).toHaveText('04/2023');
  await expect(await valueOf(page, 'A2')).toHaveValue('Nam');
  await expect(await valueOf(page, 'B2')).toHaveValue('790');
  await expect(await valueOf(page, 'B3')).toHaveValue('420');
  await expect(await valueOf(page, 'B4')).toHaveValue('1210');
  expect(errors).toEqual([]);
});
