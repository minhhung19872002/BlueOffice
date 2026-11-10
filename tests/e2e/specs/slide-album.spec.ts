import { expect, test, type Page } from '@playwright/test';
import { mkdtempSync, readFileSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { deflateSync, inflateRawSync } from 'node:zlib';

/**
 * SLIDE-ALBUM-001: a photo album from picked pictures, slides from an outline
 * (a BlueDoc file and a text file), and slides reused from another presentation —
 * each one undo step, kept when the presentation is saved and opened again.
 */

const here = dirname(fileURLToPath(import.meta.url));
const corpus = join(here, '..', '..', 'corpus');
const bullets = join(corpus, 'slide', 'slide-text-bullets.pptx');
const presets = join(corpus, 'slide', 'slide-shapes-presets.pptx');
const headings = join(corpus, 'doc', 'lists-04-heading-linked.docx');

async function openDeck(page: Page, path: string) {
  await page.getByRole('tab', { name: 'Trang đầu' }).click();
  const chooser = page.waitForEvent('filechooser');
  await page.getByRole('button', { name: 'Mở', exact: true }).click();
  await (await chooser).setFiles(path);
  await expect(page.getByTestId('slide-status')).toHaveText(/^Slide 1 \/ \d+$/);
  await expect(page.getByTestId('slide-overlay')).toBeVisible();
}

async function command(page: Page, tab: string, id: string) {
  await page.getByRole('tab', { name: tab, exact: true }).click();
  await page.locator(`[data-command="${id}"]`).first().click();
}

/** Save with Ctrl+S; returns the saved bytes and opens the copy again. */
async function saveAndReopen(page: Page, name: string): Promise<Buffer> {
  const download = page.waitForEvent('download');
  await page.keyboard.press('Control+s');
  const bytes = readFileSync(await (await download).path());
  const copy = join(mkdtempSync(join(tmpdir(), 'bo-e2e-album-')), name);
  writeFileSync(copy, bytes);
  await openDeck(page, copy);
  return bytes;
}

/** Entries of a zip package (stored or deflated), from the central directory. */
function zipEntries(zip: Buffer): Map<string, () => Buffer> {
  const out = new Map<string, () => Buffer>();
  let eocd = zip.length - 22;
  while (eocd >= 0 && zip.readUInt32LE(eocd) !== 0x06054b50) eocd--;
  if (eocd < 0) return out;
  const entries = zip.readUInt16LE(eocd + 10);
  let p = zip.readUInt32LE(eocd + 16);
  for (let i = 0; i < entries; i++) {
    const method = zip.readUInt16LE(p + 10);
    const size = zip.readUInt32LE(p + 20);
    const nameLen = zip.readUInt16LE(p + 28);
    const local = zip.readUInt32LE(p + 42);
    const name = zip.toString('utf8', p + 46, p + 46 + nameLen);
    out.set(name, () => {
      const start = local + 30 + zip.readUInt16LE(local + 26) + zip.readUInt16LE(local + 28);
      const data = zip.subarray(start, start + size);
      return method === 8 ? inflateRawSync(data) : data;
    });
    p += 46 + nameLen + zip.readUInt16LE(p + 30) + zip.readUInt16LE(p + 32);
  }
  return out;
}

const slideParts = (zip: Map<string, () => Buffer>) =>
  [...zip.keys()].filter((n) => /^ppt\/slides\/slide\d+\.xml$/.test(n));
const mediaParts = (zip: Map<string, () => Buffer>) =>
  [...zip.keys()].filter((n) => n.startsWith('ppt/media/'));
/** Text runs of a slide part. */
const textOf = (xml: string) => [...xml.matchAll(/<a:t>([^<]*)<\/a:t>/g)].map((m) => m[1]);

const CRC = (() => {
  const t = new Uint32Array(256);
  for (let n = 0; n < 256; n++) {
    let c = n;
    for (let k = 0; k < 8; k++) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1;
    t[n] = c >>> 0;
  }
  return t;
})();

function crc32(data: Buffer): number {
  let c = 0xffffffff;
  for (const b of data) c = CRC[(c ^ b) & 0xff]! ^ (c >>> 8);
  return (c ^ 0xffffffff) >>> 0;
}

/** A plain-colour RGB PNG (ISO/IEC 15948: signature, IHDR, IDAT, IEND). */
function png(width: number, height: number, [r, g, b]: [number, number, number]): Buffer {
  const chunk = (type: string, data: Buffer) => {
    const body = Buffer.concat([Buffer.from(type, 'ascii'), data]);
    const len = Buffer.alloc(4);
    len.writeUInt32BE(data.length);
    const crc = Buffer.alloc(4);
    crc.writeUInt32BE(crc32(body));
    return Buffer.concat([len, body, crc]);
  };
  const ihdr = Buffer.alloc(13);
  ihdr.writeUInt32BE(width, 0);
  ihdr.writeUInt32BE(height, 4);
  ihdr.set([8, 2, 0, 0, 0], 8); // 8 bits, RGB, deflate, no filter, no interlace
  const row = Buffer.alloc(1 + width * 3);
  for (let x = 0; x < width; x++) row.set([r, g, b], 1 + x * 3);
  const raw = Buffer.concat(Array.from({ length: height }, () => row));
  return Buffer.concat([
    Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]),
    chunk('IHDR', ihdr),
    chunk('IDAT', deflateSync(raw)),
    chunk('IEND', Buffer.alloc(0)),
  ]);
}

test('photo album from three pictures: order, captions, one undo step, saved', async ({ page }) => {
  const errors: string[] = [];
  page.on('pageerror', (e) => errors.push(e.message));
  const dir = mkdtempSync(join(tmpdir(), 'bo-e2e-album-'));
  const pictures = [
    { name: 'bai-bien.png', bytes: png(80, 60, [46, 117, 182]) },
    { name: 'nui-rung.png', bytes: png(60, 90, [84, 130, 53]) },
    { name: 'pho-co.png', bytes: png(120, 40, [192, 80, 77]) },
  ];
  const paths = pictures.map((p) => {
    const path = join(dir, p.name);
    writeFileSync(path, p.bytes);
    return path;
  });
  const baseMedia = mediaParts(zipEntries(readFileSync(bullets))).length;

  await page.goto('/slide');
  await openDeck(page, bullets);
  await expect(page.getByTestId('slide-status')).toHaveText('Slide 1 / 3');

  // Insert › Photo album: pick the three pictures at once.
  await command(page, 'Chèn', 'insert.photoAlbum');
  const dialog = page.getByTestId('album-dialog');
  await expect(dialog).toBeVisible();
  await expect(page.getByTestId('album-create')).toBeDisabled();
  const chooser = page.waitForEvent('filechooser');
  await page.getByTestId('album-add').click();
  const picked = await chooser;
  expect(picked.isMultiple()).toBe(true);
  await picked.setFiles(paths);
  await expect(page.getByTestId('album-item-2')).toContainText('pho-co.png');
  await expect(page.getByTestId('album-summary')).toHaveText('3 ảnh trên 3 slide');

  // Order from the keyboard: the last picture goes first (Enter on "move up", twice).
  const up = page.getByRole('button', { name: 'Đưa pho-co.png lên trên' });
  await up.focus();
  await page.keyboard.press('Enter');
  await expect(page.getByTestId('album-item-1')).toContainText('pho-co.png');
  await expect(up).toBeFocused();
  await page.keyboard.press('Enter');
  await expect(page.getByTestId('album-item-0')).toContainText('pho-co.png');
  await expect(page.getByTestId('album-item-2')).toContainText('nui-rung.png');

  // Two pictures per slide with captions, rounded corners, no slide titles.
  await page.getByTestId('album-layout').selectOption('two');
  await expect(page.getByTestId('album-summary')).toHaveText('3 ảnh trên 2 slide');
  await page.getByTestId('album-frame').selectOption('rounded');
  await page.getByTestId('album-titles').uncheck();
  await page.getByTestId('album-captions').check();
  await page.getByTestId('album-create').click();
  await expect(dialog).toHaveCount(0);

  // Two slides after the first one; the editor shows the album's first slide.
  await expect(page.getByTestId('slide-status')).toHaveText('Slide 2 / 5');
  await expect(page.getByTestId('thumb-4')).toBeVisible();

  // The whole album is one undo step.
  await command(page, 'Trang đầu', 'edit.undo');
  await expect(page.getByTestId('slide-status')).toHaveText(/ \/ 3$/);
  await expect(page.locator('[data-command="edit.undo"]').first()).toBeDisabled();
  await command(page, 'Trang đầu', 'edit.redo');
  await expect(page.getByTestId('slide-status')).toHaveText(/ \/ 5$/);

  // Saved: three embedded pictures as p:pic on the two album slides, captions = file names.
  const saved = await saveAndReopen(page, 'album.pptx');
  await expect(page.getByTestId('slide-status')).toHaveText('Slide 1 / 5');
  const zip = zipEntries(saved);
  expect(slideParts(zip)).toHaveLength(5);
  expect(mediaParts(zip)).toHaveLength(baseMedia + 3);
  const embedded = mediaParts(zip).map((n) => zip.get(n)!());
  for (const p of pictures) expect(embedded.some((e) => e.equals(p.bytes))).toBe(true);
  const albumSlides = slideParts(zip)
    .map((n) => zip.get(n)!().toString('utf8'))
    .filter((xml) => xml.includes('<p:pic>'));
  expect(albumSlides.map((xml) => xml.split('<p:pic>').length - 1).sort()).toEqual([1, 2]);
  const two = albumSlides.find((xml) => xml.split('<p:pic>').length === 3)!;
  const one = albumSlides.find((xml) => xml.split('<p:pic>').length === 2)!;
  expect(textOf(two)).toEqual(['pho-co', 'bai-bien']);
  expect(textOf(one)).toEqual(['nui-rung']);
  // Rounded frame: the pictures are cropped to a rounded rectangle (prstGeom roundRect).
  expect(two.split('prst="roundRect"').length - 1).toBe(2);
  // The pictures keep their proportions (pho-co.png is 120 × 40).
  const ext = /<p:pic>.*?<a:ext cx="(\d+)" cy="(\d+)"\/>/s.exec(two)!;
  expect(Number(ext[1]) / Number(ext[2])).toBeCloseTo(3, 2);
  await page.getByTestId('thumb-1').click();
  await expect(page.getByTestId('slide-status')).toHaveText('Slide 2 / 5');
  expect(errors).toEqual([]);
});

test('photo album in a new presentation when none is open', async ({ page }) => {
  const errors: string[] = [];
  page.on('pageerror', (e) => errors.push(e.message));
  const dir = mkdtempSync(join(tmpdir(), 'bo-e2e-album-'));
  const path = join(dir, 'anh.png');
  writeFileSync(path, png(64, 48, [255, 192, 0]));
  await page.goto('/slide');
  await command(page, 'Chèn', 'insert.photoAlbum');
  await expect(page.getByTestId('album-target-append')).toBeDisabled();
  await expect(page.getByTestId('album-target-new')).toBeChecked();
  const chooser = page.waitForEvent('filechooser');
  await page.getByTestId('album-add').click();
  await (await chooser).setFiles([path]);
  await expect(page.getByTestId('album-item-0')).toContainText('anh.png');
  await page.getByTestId('album-title').fill('Hè 2024');
  await page.getByTestId('album-layout').selectOption('fit');
  await page.getByTestId('album-create').click();
  // A title slide with the album's name, then the picture slide (shown).
  await expect(page.getByTestId('slide-status')).toHaveText('Slide 2 / 2');
  const download = page.waitForEvent('download');
  await page.keyboard.press('Control+s');
  const file = await download;
  expect(file.suggestedFilename()).toBe('Hè 2024.pptx');
  const zip = zipEntries(readFileSync(await file.path()));
  expect(slideParts(zip)).toHaveLength(2);
  expect(mediaParts(zip)).toHaveLength(1);
  const xml = slideParts(zip).map((n) => zip.get(n)!().toString('utf8'));
  expect(xml.some((x) => textOf(x).includes('Hè 2024'))).toBe(true);
  expect(xml.filter((x) => x.includes('<p:pic>'))).toHaveLength(1);
  expect(errors).toEqual([]);
});

test('slides from an outline: headings of a document, indented text; one undo each', async ({
  page,
}) => {
  const errors: string[] = [];
  page.on('pageerror', (e) => errors.push(e.message));
  await page.goto('/slide');
  await openDeck(page, bullets);

  // Home › Slides from outline: the headings of a BlueDoc file.
  let chooser = page.waitForEvent('filechooser');
  await command(page, 'Trang đầu', 'slide.fromOutline');
  await (await chooser).setFiles(headings);
  const dialog = page.getByTestId('outline-import-dialog');
  await expect(dialog).toBeVisible();
  const preview = page.getByTestId('outline-import-preview');
  await expect(preview.locator('li')).toHaveCount(2);
  await expect(preview).toContainText('1. Tổng quan');
  await expect(preview).toContainText('• Thuật ngữ');
  await expect(preview).not.toContainText('Nội dung');
  // The dialog is reachable by keyboard: Enter on the focused Add button.
  const add = page.getByTestId('outline-import-insert');
  await expect(add).toHaveText('Thêm 2 slide');
  await add.focus();
  await page.keyboard.press('Enter');
  await expect(dialog).toHaveCount(0);
  await expect(page.getByTestId('slide-status')).toHaveText('Slide 2 / 5');

  // A text file: lines without indentation are titles, tabs give the bullet levels.
  const dir = mkdtempSync(join(tmpdir(), 'bo-e2e-album-'));
  const txt = join(dir, 'dan-y.txt');
  writeFileSync(txt, 'Kế hoạch\n\tMục tiêu\n\t\tDoanh thu\nKết luận\n', 'utf8');
  chooser = page.waitForEvent('filechooser');
  await command(page, 'Chèn', 'slide.fromOutline');
  await (await chooser).setFiles(txt);
  await expect(preview.locator('li')).toHaveCount(2);
  await add.click();
  await expect(page.getByTestId('slide-status')).toHaveText('Slide 3 / 7');

  // Each import is one undo step.
  await command(page, 'Trang đầu', 'edit.undo');
  await expect(page.getByTestId('slide-status')).toHaveText(/ \/ 5$/);
  await command(page, 'Trang đầu', 'edit.redo');
  await expect(page.getByTestId('slide-status')).toHaveText(/ \/ 7$/);

  // Saved and reopened: titles and bullets with their levels, in order.
  const saved = await saveAndReopen(page, 'outline.pptx');
  await expect(page.getByTestId('slide-status')).toHaveText('Slide 1 / 7');
  const zip = zipEntries(saved);
  const texts = slideParts(zip).map((n) => zip.get(n)!().toString('utf8'));
  const slide = (title: string) => texts.find((x) => textOf(x)[0] === title);
  expect(textOf(slide('Tổng quan')!)).toEqual(['Tổng quan', 'Phạm vi', 'Thuật ngữ']);
  expect(textOf(slide('Thiết kế')!)).toEqual(['Thiết kế', 'Mô hình']);
  const plan = slide('Kế hoạch')!;
  expect(textOf(plan)).toEqual(['Kế hoạch', 'Mục tiêu', 'Doanh thu']);
  // "Doanh thu" is one level below "Mục tiêu" (a:pPr lvl="1").
  expect(/<a:pPr[^>]*lvl="1"[^>]*\/?>(?:(?!<\/a:p>).)*Doanh thu/s.test(plan)).toBe(true);
  expect(/<a:pPr[^>]*lvl="1"[^>]*\/?>(?:(?!<\/a:p>).)*Mục tiêu/s.test(plan)).toBe(false);
  expect(textOf(slide('Kết luận')!)).toEqual(['Kết luận']);
  expect(errors).toEqual([]);
});

test('reuse two slides of another presentation, keeping their formatting; saved', async ({
  page,
}) => {
  const errors: string[] = [];
  page.on('pageerror', (e) => errors.push(e.message));
  const source = zipEntries(readFileSync(bullets));
  const sourceTitles = ['ppt/slides/slide1.xml', 'ppt/slides/slide2.xml', 'ppt/slides/slide3.xml']
    .map((n) => textOf(source.get(n)!().toString('utf8'))[0]!)
    .sort();
  const baseMasters = [...zipEntries(readFileSync(presets)).keys()].filter((n) =>
    /^ppt\/slideMasters\/[^/]+\.xml$/.test(n),
  ).length;

  await page.goto('/slide');
  await openDeck(page, presets);
  await expect(page.getByTestId('slide-status')).toHaveText('Slide 1 / 2');

  // Home › Reuse slides opens the pane; open the other presentation in it.
  await command(page, 'Trang đầu', 'slide.reuse');
  const pane = page.getByTestId('reuse-pane');
  await expect(pane).toBeVisible();
  await expect(page.locator('[data-command="slide.reuse"]').first()).toHaveAttribute(
    'aria-pressed',
    'true',
  );
  const chooser = page.waitForEvent('filechooser');
  await page.getByTestId('reuse-open').click();
  await (await chooser).setFiles(bullets);
  await expect(page.getByTestId('reuse-source')).toHaveText('slide-text-bullets.pptx — 3 slide');
  const options = pane.getByRole('option');
  await expect(options).toHaveCount(3);
  // Thumbnails are real pictures of the slides.
  for (let i = 0; i < 3; i++) {
    const img = page.getByTestId(`reuse-slide-${i}`).locator('img');
    await expect(img).toBeVisible();
    expect(await img.evaluate((el: HTMLImageElement) => el.naturalWidth)).toBeGreaterThan(0);
  }
  const titles = (await options.allTextContents()).map((s) => s.replace(/^Slide \d+/, ''));
  expect([...titles].sort()).toEqual(sourceTitles);

  // Choose slides 2 and 3 with the keyboard, keep the source formatting, insert.
  await expect(page.getByTestId('reuse-insert')).toBeDisabled();
  await page.getByTestId('reuse-slide-0').focus();
  await page.keyboard.press('ArrowDown');
  await page.keyboard.press('Space');
  await page.keyboard.press('ArrowDown');
  await page.keyboard.press('Space');
  await expect(options.nth(1)).toHaveAttribute('aria-selected', 'true');
  await expect(options.nth(2)).toHaveAttribute('aria-selected', 'true');
  await expect(options.nth(0)).toHaveAttribute('aria-selected', 'false');
  await page.getByTestId('reuse-keep-source').check();
  await expect(page.getByTestId('reuse-insert')).toHaveText('Chèn slide đã chọn (2)');
  await page.getByTestId('reuse-insert').click();
  await expect(page.getByTestId('slide-status')).toHaveText('Slide 2 / 4');
  await expect(page.getByTestId('thumb-3')).toBeVisible();

  // One undo step for both slides.
  await command(page, 'Trang đầu', 'edit.undo');
  await expect(page.getByTestId('slide-status')).toHaveText(/ \/ 2$/);
  await command(page, 'Trang đầu', 'edit.redo');
  await expect(page.getByTestId('slide-status')).toHaveText(/ \/ 4$/);

  // Without keeping the formatting: one more slide (the first), by double click.
  await page.getByTestId('reuse-keep-source').uncheck();
  await page.getByTestId('reuse-slide-0').dblclick();
  await expect(page.getByTestId('slide-status')).toHaveText(/ \/ 5$/);

  // Saved and reopened: the reused slides are there, in order, after the first slide.
  const saved = await saveAndReopen(page, 'reuse.pptx');
  await expect(page.getByTestId('slide-status')).toHaveText('Slide 1 / 5');
  const zip = zipEntries(saved);
  expect(slideParts(zip)).toHaveLength(5);
  const all = slideParts(zip).map((n) => textOf(zip.get(n)!().toString('utf8'))[0]);
  for (const title of titles) expect(all).toContain(title);
  // Every relationship of the slides points at a part of the package.
  for (const n of slideParts(zip)) {
    const rels = zip.get(n.replace('slides/', 'slides/_rels/') + '.rels')!().toString('utf8');
    for (const m of rels.matchAll(/Target="\.\.\/([^"]+)"/g)) {
      expect(zip.has(`ppt/${m[1]}`), `${n} → ${m[1]}`).toBe(true);
    }
  }
  expect(
    [...zip.keys()].filter((n) => /^ppt\/slideMasters\/[^/]+\.xml$/.test(n)).length,
  ).toBeGreaterThanOrEqual(baseMasters);
  // The pane is still open; it closes from the keyboard (Escape inside it).
  await expect(pane).toBeVisible();
  await page.getByTestId('reuse-open').focus();
  await page.keyboard.press('Escape');
  await expect(pane).toHaveCount(0);
  expect(errors).toEqual([]);
});
