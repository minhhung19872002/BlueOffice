import { expect, test, type Page } from '@playwright/test';
import { readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { inflateRawSync } from 'node:zlib';

/**
 * CORE-ICON-001: the icon library — search without accents, insert into a BlueDoc
 * document (inline PNG picture, one inch square) and onto a BlueSlide slide (SVG picture
 * with a PNG fallback); the icon name is the alternative text and the picture is still in
 * the file after saving and reopening.
 */

const here = dirname(fileURLToPath(import.meta.url));
const deck = join(here, '..', '..', 'corpus', 'slide', 'slide-text-bullets.pptx');
const library = join(here, '..', '..', '..', 'assets', 'icons', 'library');

/** Every entry of a zip package (stored or deflated), from the central directory. */
function unzip(zip: Buffer): Map<string, Buffer> {
  const out = new Map<string, Buffer>();
  let eocd = zip.length - 22;
  while (eocd >= 0 && zip.readUInt32LE(eocd) !== 0x06054b50) eocd--;
  if (eocd < 0) return out;
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
    const start = local + 30 + zip.readUInt16LE(local + 26) + zip.readUInt16LE(local + 28);
    const data = zip.subarray(start, start + size);
    out.set(name, method === 8 ? inflateRawSync(data) : Buffer.from(data));
    p += 46 + nameLen + extraLen + commentLen;
  }
  return out;
}

/** Width and height of a PNG (ISO/IEC 15948 IHDR). */
function pngSize(png: Buffer): [number, number] {
  expect(png.subarray(1, 4).toString('latin1')).toBe('PNG');
  return [png.readUInt32BE(16), png.readUInt32BE(20)];
}

async function openIcons(page: Page) {
  await page.getByRole('tab', { name: 'Chèn' }).click();
  await page.locator('[data-command="insert.icon"]').first().click();
  const panel = page.getByRole('dialog', { name: 'Chèn biểu tượng' });
  await expect(panel).toBeVisible();
  // The library chunk is fetched on first use.
  await expect(panel.getByTestId('icon-grid')).toHaveAttribute('aria-busy', 'false');
  return panel;
}

async function save(page: Page): Promise<{ path: string; parts: Map<string, Buffer> }> {
  const download = page.waitForEvent('download');
  await page.keyboard.press('Control+s');
  const path = await (await download).path();
  await expect(page.getByTestId('status-modified')).toHaveCount(0);
  return { path, parts: unzip(readFileSync(path)) };
}

function media(parts: Map<string, Buffer>, dir: string, ext: string): Buffer[] {
  return [...parts].filter(([n]) => n.startsWith(dir) && n.endsWith(ext)).map(([, d]) => d);
}

test('BlueDoc: inserts an icon found without accents; it survives save and reopen', async ({
  page,
}) => {
  const errors: string[] = [];
  page.on('pageerror', (e) => errors.push(e.message));
  await page.goto('/doc');
  await page.getByTestId('new-document').click();
  await expect(page.getByTestId('caret')).toBeVisible();
  await page.keyboard.type('Dau ');

  const panel = await openIcons(page);
  // Category filter: the administrative icons only.
  await panel.getByTestId('icon-category').selectOption({ label: 'Hành chính' });
  await expect(panel.getByRole('gridcell', { name: 'Con dấu tròn' })).toBeVisible();
  await expect(panel.getByRole('gridcell', { name: 'Mũi tên phải' })).toHaveCount(0);
  await panel.getByTestId('icon-category').selectOption({ label: 'Tất cả các nhóm' });
  // "con dau" (no accents) finds "Con dấu tròn".
  await panel.getByTestId('icon-search').fill('con dau');
  const seal = panel.getByRole('gridcell', { name: 'Con dấu tròn' });
  await expect(seal).toBeVisible();
  await seal.dblclick();
  await expect(page.getByTestId('status-modified')).toBeVisible();
  await page.keyboard.press('Escape');
  await expect(panel).toHaveCount(0);

  const check = (parts: Map<string, Buffer>) => {
    const xml = parts.get('word/document.xml')?.toString('utf8') ?? '';
    // One inline picture, 1 inch = 914400 EMU square, named by the icon (alt text).
    expect(xml.match(/<pic:pic[ >]/g)?.length).toBe(1);
    expect(xml).toContain('descr="Con dấu tròn"');
    expect(xml).toMatch(/<wp:extent cx="914400" cy="914400"\/>/);
    // Only the sharp PNG is stored (the box placeholder was replaced).
    const pngs = media(parts, 'word/media/', '.png');
    expect(pngs.map(pngSize)).toEqual([[384, 384]]);
  };

  // Insertion is one undo step: undo removes the picture, redo brings it back whole.
  const first = await save(page);
  check(first.parts);
  await page.keyboard.press('Control+z');
  const undone = await save(page);
  expect(undone.parts.get('word/document.xml')?.toString('utf8')).not.toContain('<pic:pic');
  await page.keyboard.press('Control+y');
  const redone = await save(page);
  check(redone.parts);

  // Reopen the saved file, edit it and save again: the picture is still there.
  await page.reload();
  const chooser = page.waitForEvent('filechooser');
  await page.getByRole('button', { name: 'Mở tệp từ máy' }).click();
  await (await chooser).setFiles(redone.path);
  await expect(page.locator('.bo-page[data-page="1"]')).toHaveAttribute('data-rendered', 'true');
  // Click right of the first line (it is one inch tall now): the caret goes to its end.
  const sheet = (await page.locator('.bo-page[data-page="1"]').boundingBox())!;
  await page.mouse.click(sheet.x + sheet.width * 0.7, sheet.y + sheet.height * 0.13);
  await expect(page.getByTestId('caret')).toBeVisible();
  await page.keyboard.type(' xong');
  await expect(page.getByTestId('status-modified')).toBeVisible();
  check((await save(page)).parts);
  expect(errors).toEqual([]);
});

test('BlueSheet: places icons as pictures in cells with the name as alt text', async ({ page }) => {
  const errors: string[] = [];
  page.on('pageerror', (e) => errors.push(e.message));
  await page.goto('/sheet');
  await expect(page.getByTestId('sheet-canvas')).toHaveAttribute('data-rendered', /\d+/);

  const panel = await openIcons(page);
  await panel.getByTestId('icon-search').fill('Bar chart');
  await panel.getByRole('gridcell', { name: 'Biểu đồ cột' }).click();
  await panel.getByTestId('icon-search').fill('lich');
  await panel.getByRole('gridcell', { name: 'Lịch', exact: true }).click();
  await panel.getByTestId('icon-insert').click();
  await expect(panel.getByTestId('icon-status')).not.toHaveText('Đã chọn 2 biểu tượng');
  await page.keyboard.press('Escape');
  await expect(panel).toHaveCount(0);
  // The active cell (A1) now holds a picture: its fallback value shows in the formula bar.
  await expect(page.getByLabel('Thanh công thức')).toHaveValue('#VALUE!');

  const download = page.waitForEvent('download');
  await page.keyboard.press('Control+s');
  const parts = unzip(readFileSync(await (await download).path()));
  // A1 and B1 carry the pictures (value metadata → rich values), in selection order.
  const sheet = parts.get('xl/worksheets/sheet1.xml')?.toString('utf8') ?? '';
  expect(sheet).toMatch(/<c r="A1"[^>]* vm="1"/);
  expect(sheet).toMatch(/<c r="B1"[^>]* vm="2"/);
  const values = parts.get('xl/richData/rdrichvalue.xml')?.toString('utf8') ?? '';
  expect(values.indexOf('<v>Biểu đồ cột</v>')).toBeGreaterThan(0);
  expect(values.indexOf('<v>Lịch</v>')).toBeGreaterThan(values.indexOf('<v>Biểu đồ cột</v>'));
  expect(media(parts, 'xl/media/', '.png').map(pngSize)).toEqual([
    [384, 384],
    [384, 384],
  ]);
  expect(errors).toEqual([]);
});

test('BlueSlide: inserts icons as SVG pictures with a PNG fallback; they survive save and reopen', async ({
  page,
}) => {
  const errors: string[] = [];
  page.on('pageerror', (e) => errors.push(e.message));
  await page.goto('/slide');
  const openDeck = async (path: string) => {
    const chooser = page.waitForEvent('filechooser');
    await page.getByRole('button', { name: 'Mở', exact: true }).click();
    await (await chooser).setFiles(path);
    await expect(page.getByTestId('slide-status')).toHaveText(/^Slide 1 \/ \d+$/);
    await expect(page.getByTestId('slide-overlay')).toBeVisible();
  };
  await openDeck(deck);

  const panel = await openIcons(page);
  // Multi-select: two icons, inserted together with the Insert button.
  await panel.getByTestId('icon-search').fill('lich');
  await panel.getByRole('gridcell', { name: 'Lịch', exact: true }).click();
  await panel.getByTestId('icon-search').fill('bieu do cot');
  await panel.getByRole('gridcell', { name: 'Biểu đồ cột' }).click();
  await expect(panel.getByTestId('icon-status')).toHaveText('Đã chọn 2 biểu tượng');
  await panel.getByTestId('icon-insert').click();
  await expect(page.getByTestId('status-modified')).toBeVisible();
  await expect(panel.getByTestId('icon-status')).not.toHaveText('Đã chọn 2 biểu tượng');
  await page.keyboard.press('Escape');
  await expect(panel).toHaveCount(0);
  // The last inserted picture is selected.
  await expect(page.getByTestId('selection-box').first()).toBeVisible();

  const check = (parts: Map<string, Buffer>) => {
    const xml = parts.get('ppt/slides/slide1.xml')?.toString('utf8') ?? '';
    // Alternative text = the icon name (ECMA-376 Part 1, cNvPr `descr`).
    expect(xml).toContain('descr="Lịch"');
    expect(xml).toContain('descr="Biểu đồ cột"');
    // Each picture's blip carries the SVG in the svgBlip extension next to the PNG.
    expect(xml.match(/<asvg:svgBlip /g)?.length).toBe(2);
    // The stored SVG parts are the library files, byte for byte.
    const svgs = media(parts, 'ppt/media/', '.svg').map((b) => b.toString('utf8'));
    expect(svgs.sort()).toEqual(
      ['calendar.svg', 'bar-chart.svg'].map((f) => readFileSync(join(library, f), 'utf8')).sort(),
    );
    // 96 user units × 4 (the engine's fallback scale) = 384 px.
    const fallbacks = media(parts, 'ppt/media/', '.png').map(pngSize);
    expect(fallbacks.filter(([w, h]) => w === 384 && h === 384)).toHaveLength(2);
    const types = parts.get('[Content_Types].xml')?.toString('utf8') ?? '';
    expect(types).toContain('image/svg+xml');
  };

  const first = await save(page);
  check(first.parts);

  // Reopen the saved deck, change it and save again: both pictures round-trip.
  await page.reload();
  await openDeck(first.path);
  await page.getByRole('tab', { name: 'Chèn' }).click();
  await page.getByRole('button', { name: 'Hình elip' }).click();
  await expect(page.getByTestId('status-modified')).toBeVisible();
  check((await save(page)).parts);
  expect(errors).toEqual([]);
});
