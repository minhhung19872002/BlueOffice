import { expect, test, type Page } from '@playwright/test';
import { copyFileSync, mkdirSync, readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { inflateRawSync } from 'node:zlib';

// CORE-CHART-007 in BlueDoc: insert a column chart, edit two values in the data grid, change it to
// a line chart, save and reopen; the chart part holds the new values.
const outDir = join(dirname(fileURLToPath(import.meta.url)), '..', '..', 'fidelity', 'output');
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

test('BlueDoc: inserts a column chart, edits its data, makes it a line chart, saved and reopened', async ({
  page,
}) => {
  const errors: string[] = [];
  page.on('pageerror', (e) => errors.push(e.message));
  await page.goto('/doc');
  await page.getByTestId('new-document').click();
  await expect(page.getByTestId('caret')).toBeVisible();
  const chartTab = page.getByRole('tab', { name: 'Biểu đồ', exact: true });
  await expect(chartTab).toHaveCount(0);

  // Insert › Chart: a column chart of the sample grid, two values changed.
  await page.getByRole('tab', { name: 'Chèn' }).click();
  await cmd(page, 'insert.chart').click();
  const dialog = page.getByTestId('chart-dialog');
  await expect(dialog.getByTestId('chart-kind')).toHaveValue('column');
  await dialog.getByTestId('chart-cell-0-0').fill('7,5');
  await dialog.getByTestId('chart-cell-1-1').fill('9');
  await dialog.getByTestId('chart-ok').click();
  await expect(dialog).toHaveCount(0);
  await expect(page.locator('.bo-page[data-page="1"]')).toHaveAttribute('data-rendered', 'true');

  // The new chart stays selected, with its Chart Design tab: make it a line chart.
  await expect(chartTab).toBeVisible();
  await chartTab.click();
  await cmd(page, 'chart.type.line').click();
  await expect(cmd(page, 'chart.type.line')).toHaveAttribute('aria-pressed', 'true');

  const download = page.waitForEvent('download');
  await page.keyboard.press('Control+s');
  const saved = await (await download).path();
  mkdirSync(outDir, { recursive: true });
  copyFileSync(saved, join(outDir, 'web-doc-chart.docx'));
  const zip = readFileSync(saved);
  const xml = zipEntry(zip, 'word/charts/chart1.xml');
  expect(xml).toContain('<c:lineChart>');
  expect(xml).toContain('<c:v>7.5</c:v>');
  expect(xml).toContain('<c:v>9</c:v>');
  expect(zipEntry(zip, 'word/document.xml')).toContain('drawingml/2006/chart');
  expect(zipEntry(zip, 'word/charts/_rels/chart1.xml.rels')).toContain('../embeddings/chart1.xlsx');

  // Reopened: the chart comes back with its type and data.
  await page.reload();
  const chooser = page.waitForEvent('filechooser');
  await page.getByRole('button', { name: 'Mở tệp từ máy' }).click();
  await (await chooser).setFiles(saved);
  await expect(page.locator('.bo-page[data-page="1"]')).toHaveAttribute('data-rendered', 'true');
  await page.keyboard.press('Control+a');
  await expect(chartTab).toBeVisible();
  await chartTab.click();
  await cmd(page, 'chart.edit').click();
  await expect(dialog.getByTestId('chart-kind')).toHaveValue('line');
  await expect(dialog.getByTestId('chart-cell-0-0')).toHaveValue('7.5');
  await expect(dialog.getByTestId('chart-cell-1-1')).toHaveValue('9');
  await page.keyboard.press('Escape');
  await expect(dialog).toHaveCount(0);
  expect(errors).toEqual([]);
});
