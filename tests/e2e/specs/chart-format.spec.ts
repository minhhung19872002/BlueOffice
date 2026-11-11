import { expect, test, type Page } from '@playwright/test';
import { readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { inflateRawSync } from 'node:zlib';

/**
 * CORE-CHART-005 / CORE-CHART-007: the shared "Định dạng biểu đồ" pane and the "Thêm thành phần
 * biểu đồ" menu in BlueSheet, BlueSlide and BlueDoc. Options set in the pane are written as the
 * `c:` elements of the chart part and are still there after the file is reopened and saved again.
 */
const here = dirname(fileURLToPath(import.meta.url));
const deck = join(here, '..', '..', 'corpus', 'slide', 'slide-text-bullets.pptx');
const cmd = (page: Page, id: string) => page.locator(`[data-command="${id}"]`);

/** One entry of a zip package (stored or deflated), from the central directory. */
function zipEntry(zip: Buffer, want: string): string {
  let eocd = zip.length - 22;
  while (eocd >= 0 && zip.readUInt32LE(eocd) !== 0x06054b50) eocd--;
  if (eocd < 0) throw new Error('not a zip');
  const entries = zip.readUInt16LE(eocd + 10);
  let p = zip.readUInt32LE(eocd + 16);
  for (let i = 0; i < entries; i++) {
    const method = zip.readUInt16LE(p + 10);
    const size = zip.readUInt32LE(p + 20);
    const nameLen = zip.readUInt16LE(p + 28);
    const extraLen = zip.readUInt16LE(p + 30);
    const commentLen = zip.readUInt16LE(p + 32);
    const local = zip.readUInt32LE(p + 42);
    const name = zip.toString('utf8', p + 46, p + 46 + nameLen);
    if (name === want) {
      const start = local + 30 + zip.readUInt16LE(local + 26) + zip.readUInt16LE(local + 28);
      const data = zip.subarray(start, start + size);
      return (method === 8 ? inflateRawSync(data) : Buffer.from(data)).toString('utf8');
    }
    p += 46 + nameLen + extraLen + commentLen;
  }
  throw new Error(`${want} not in the package`);
}

async function goTo(page: Page, ref: string) {
  const box = page.getByLabel('Hộp tên');
  await box.click();
  await box.fill(ref);
  await box.press('Enter');
  await expect(page.getByLabel('Ô đang nhập')).toBeFocused();
}

/** Types into a field of the pane and commits it. */
async function setField(page: Page, testId: string, text: string) {
  const field = page.getByTestId(testId);
  await field.fill(text);
  await field.press('Enter');
  await expect(field).toHaveValue(text);
}

/** Clicks a (controlled) checkbox of the pane and waits for the chart to take it. */
async function tick(page: Page, testId: string) {
  await page.getByTestId(testId).click();
  await expect(page.getByTestId(testId)).toBeChecked();
}

const pick = (page: Page, element: string) =>
  page.getByTestId('chart-format-element').selectOption(element);

async function insertSheetChart(page: Page, range: string, kind: string) {
  await goTo(page, range);
  await page.getByRole('tab', { name: 'Chèn' }).click();
  await cmd(page, 'insert.chart').click();
  await page.getByTestId('menu-chart').locator(`[data-menu="${kind}"]`).click();
  await expect(page.getByTestId('chart-selection')).toBeVisible();
  await page.getByRole('tab', { name: 'Thiết kế biểu đồ' }).click();
}

test('BlueSheet: log axis with a fixed maximum, percentage labels, polynomial trend line, error bars', async ({
  page,
}) => {
  const errors: string[] = [];
  page.on('pageerror', (e) => errors.push(e.message));
  await page.goto('/sheet');
  await expect(page.getByTestId('sheet-canvas')).toHaveAttribute('data-rendered', /\d+/);
  const rows = [
    ['Tỉnh', 'Kế hoạch', 'Thực hiện'],
    ['Hà Nội', '12', '150'],
    ['Huế', '40', '90'],
    ['Đà Nẵng', '75', '600'],
    ['Cần Thơ', '20', '35'],
  ];
  for (const [i, row] of rows.entries()) {
    await goTo(page, `A${i + 1}`);
    for (const v of row) {
      await page.keyboard.type(v);
      await page.keyboard.press('Tab');
    }
  }

  // A column chart: the Chart Design tab opens the Format chart pane.
  await insertSheetChart(page, 'A1:C5', 'column');
  await cmd(page, 'chart.format').click();
  const pane = page.getByTestId('chart-format-pane');
  await expect(pane).toBeVisible();
  await expect(page.getByTestId('chart-format-element')).toHaveValue('chartArea');

  // Value axis: logarithmic scale and a fixed maximum.
  await pick(page, 'axis.val');
  await tick(page, 'cf-axis-log');
  await expect(page.getByTestId('cf-axis-log-base')).toHaveValue('10');
  await setField(page, 'cf-axis-max', '1000');

  // A polynomial trend line of order 3 with its equation on the first series.
  await pick(page, 'trendline');
  await page.getByTestId('cf-trend-add').click();
  await expect(page.getByTestId('cf-trend-item-0')).toBeVisible();
  await page.getByTestId('cf-trend-kind').selectOption('poly');
  await expect(page.getByTestId('cf-trend-order')).toHaveValue('2');
  await setField(page, 'cf-trend-order', '3');
  await tick(page, 'cf-trend-eq');

  // Error bars of 10 % on the first series.
  await pick(page, 'errorBars');
  await tick(page, 'cf-err-on');
  await page.getByTestId('cf-err-type').selectOption('percentage');
  await expect(page.getByTestId('cf-err-value')).toHaveValue('5');
  await setField(page, 'cf-err-value', '10');

  // One undo step per change: the amount goes back to 5 %, then forward again.
  await page.getByRole('tab', { name: 'Trang đầu' }).click();
  await cmd(page, 'edit.undo').first().click();
  await expect(page.getByTestId('cf-err-value')).toHaveValue('5');
  await cmd(page, 'edit.redo').first().click();
  await expect(page.getByTestId('cf-err-value')).toHaveValue('10');
  await page.getByRole('tab', { name: 'Thiết kế biểu đồ' }).click();

  // The quick menu adds gridlines; the pane shows the same state.
  await page.getByTestId('chart-add-element').click();
  await cmd(page, 'chart.el.gridlines.valMinor').click();
  await pick(page, 'gridlines');
  await expect(page.getByTestId('cf-grid-val-minor')).toBeChecked();

  // A pie chart of the first series: data labels with percentages, outside the slices.
  await insertSheetChart(page, 'A1:B5', 'pie');
  await pick(page, 'labels');
  await tick(page, 'cf-labels-on');
  await expect(page.getByTestId('cf-labels-percent')).toBeChecked();
  await tick(page, 'cf-labels-category');
  await page.getByTestId('cf-labels-pos').selectOption('outEnd');
  await expect(page.getByTestId('cf-labels-pos')).toHaveValue('outEnd');

  const check = (zip: Buffer) => {
    const column = zipEntry(zip, 'xl/charts/chart1.xml');
    for (const want of [
      '<c:logBase val="10"/>',
      '<c:max val="1000"/>',
      '<c:minorGridlines/>',
      '<c:trendlineType val="poly"/><c:order val="3"/>',
      '<c:dispEq val="1"/>',
      '<c:errValType val="percentage"/><c:noEndCap val="0"/><c:val val="10"/>',
    ]) {
      expect(column, want).toContain(want);
    }
    const pie = zipEntry(zip, 'xl/charts/chart2.xml');
    for (const want of [
      '<c:dLblPos val="outEnd"/>',
      '<c:showCatName val="1"/>',
      '<c:showPercent val="1"/>',
    ]) {
      expect(pie, want).toContain(want);
    }
  };

  // Saved: the chart parts hold the elements.
  await page.getByLabel('Ô đang nhập').focus();
  let download = page.waitForEvent('download');
  await page.keyboard.press('Control+s');
  const saved = await (await download).path();
  check(readFileSync(saved));

  // Reopened and saved again: the elements were read back and are written again.
  await page.reload();
  await expect(page.getByTestId('sheet-canvas')).toHaveAttribute('data-rendered', /\d+/);
  const chooser = page.waitForEvent('filechooser');
  await cmd(page, 'file.open').click();
  await (await chooser).setFiles(saved);
  await expect(page.getByLabel('Thanh công thức')).toBeVisible();
  await goTo(page, 'B2');
  await page.keyboard.type('13');
  await page.keyboard.press('Enter');
  download = page.waitForEvent('download');
  await page.keyboard.press('Control+s');
  check(readFileSync(await (await download).path()));
  expect(errors).toEqual([]);
});

test('BlueSlide: a trend line from the Add chart element menu, its equation from the pane', async ({
  page,
}) => {
  const errors: string[] = [];
  page.on('pageerror', (e) => errors.push(e.message));
  await page.goto('/slide');
  const chooser = page.waitForEvent('filechooser');
  await page.getByRole('button', { name: 'Mở', exact: true }).click();
  await (await chooser).setFiles(deck);
  await expect(page.getByTestId('slide-status')).toHaveText(/^Slide 1 \/ \d+$/);
  await page.getByRole('button', { name: '+ Thêm slide' }).click();
  await expect(page.getByTestId('slide-status')).toHaveText(/^Slide 2 \/ \d+$/);

  await page.getByRole('tab', { name: 'Chèn' }).click();
  await page.getByRole('button', { name: 'Biểu đồ', exact: true }).click();
  await expect(page.getByTestId('chart-dialog')).toBeVisible();
  await page.getByTestId('chart-ok').click();
  await expect(page.getByTestId('chart-dialog')).toBeHidden();
  await expect(page.getByTestId('selection-box').first()).toBeVisible();

  await page.getByRole('tab', { name: 'Biểu đồ' }).click();
  await page.getByTestId('chart-add-element').click();
  await cmd(page, 'chart.el.trendline.linear').click();
  await cmd(page, 'chart.format').click();
  await expect(page.getByTestId('chart-format-pane')).toBeVisible();
  await pick(page, 'trendline');
  await expect(page.getByTestId('cf-trend-item-0')).toBeVisible();
  await expect(page.getByTestId('cf-trend-kind')).toHaveValue('linear');
  await tick(page, 'cf-trend-eq');
  await pick(page, 'legend');
  await page.getByTestId('cf-legend-pos').selectOption('t');
  await expect(page.getByTestId('cf-legend-pos')).toHaveValue('t');

  const download = page.waitForEvent('download');
  await page.keyboard.press('Control+s');
  const xml = zipEntry(readFileSync(await (await download).path()), 'ppt/charts/chart1.xml');
  expect(xml).toContain('<c:trendlineType val="linear"/><c:dispRSqr val="0"/><c:dispEq val="1"/>');
  expect(xml).toContain('<c:legendPos val="t"/>');
  expect(errors).toEqual([]);
});

test('BlueDoc: data labels from the Add chart element menu, a fixed axis maximum from the pane', async ({
  page,
}) => {
  const errors: string[] = [];
  page.on('pageerror', (e) => errors.push(e.message));
  await page.goto('/doc');
  await page.getByTestId('new-document').click();
  await expect(page.getByTestId('caret')).toBeVisible();
  await page.getByRole('tab', { name: 'Chèn' }).click();
  await cmd(page, 'insert.chart').click();
  await page.getByTestId('chart-dialog').getByTestId('chart-ok').click();
  await expect(page.getByTestId('chart-dialog')).toHaveCount(0);
  await expect(page.locator('.bo-page[data-page="1"]')).toHaveAttribute('data-rendered', 'true');

  const chartTab = page.getByRole('tab', { name: 'Biểu đồ', exact: true });
  await expect(chartTab).toBeVisible();
  await chartTab.click();
  await page.getByTestId('chart-add-element').click();
  await cmd(page, 'chart.el.labels.outEnd').click();
  await page.getByTestId('chart-add-element').click();
  await expect(cmd(page, 'chart.el.labels.outEnd')).toHaveAttribute('aria-checked', 'true');
  await cmd(page, 'chart.format').first().click();
  await expect(page.getByTestId('chart-format-pane')).toBeVisible();
  await pick(page, 'axis.val');
  await setField(page, 'cf-axis-max', '8');
  await setField(page, 'cf-axis-major', '2');

  const download = page.waitForEvent('download');
  await page.keyboard.press('Control+s');
  const xml = zipEntry(readFileSync(await (await download).path()), 'word/charts/chart1.xml');
  expect(xml).toContain('<c:dLblPos val="outEnd"/>');
  expect(xml).toContain('<c:showVal val="1"/>');
  expect(xml).toContain('<c:max val="8"/>');
  expect(xml).toContain('<c:majorUnit val="2"/>');
  expect(errors).toEqual([]);
});
