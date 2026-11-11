import { expect, test, type Page } from '@playwright/test';
import { mkdtempSync, readFileSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { deflateSync, inflateRawSync, inflateSync } from 'node:zlib';

/**
 * CORE-PIC-002: "Remove background" in BlueSlide and BlueDoc. A generated picture (a red disk
 * on a blue → green gradient) is inserted, the mode shows the proposed removal, a "keep" mark is
 * added from the keyboard, the change is kept (one undo step) and the saved file's picture is a
 * PNG with transparent corners and an opaque centre; the original stays in the package.
 */

const here = dirname(fileURLToPath(import.meta.url));
const deck = join(here, '..', '..', 'corpus', 'slide', 'slide-text-bullets.pptx');

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

/** RGB PNG (ISO/IEC 15948) of a red disk on a horizontal blue → green gradient. */
function diskPng(width: number, height: number): Buffer {
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
  ihdr.set([8, 2, 0, 0, 0], 8);
  const r = Math.min(width, height) * 0.32;
  const rows: Buffer[] = [];
  for (let y = 0; y < height; y++) {
    const row = Buffer.alloc(1 + width * 3);
    for (let x = 0; x < width; x++) {
      const inside = (x + 0.5 - width / 2) ** 2 + (y + 0.5 - height / 2) ** 2 <= r * r;
      const t = x / width;
      const c = inside
        ? [210, 45, 40]
        : [Math.floor(25 + 15 * t), Math.floor(70 + 120 * t), Math.floor(200 - 130 * t)];
      row.set(c, 1 + x * 3);
    }
    rows.push(row);
  }
  return Buffer.concat([
    Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]),
    chunk('IHDR', ihdr),
    chunk('IDAT', deflateSync(Buffer.concat(rows))),
    chunk('IEND', Buffer.alloc(0)),
  ]);
}

/** Decode an 8-bit RGBA / RGB non-interlaced PNG (enough for the engine's output). */
function decodePng(png: Buffer): {
  width: number;
  height: number;
  alpha: (x: number, y: number) => number;
} {
  let p = 8;
  let width = 0;
  let height = 0;
  let type = 0;
  const idat: Buffer[] = [];
  while (p < png.length) {
    const len = png.readUInt32BE(p);
    const kind = png.toString('ascii', p + 4, p + 8);
    const data = png.subarray(p + 8, p + 8 + len);
    if (kind === 'IHDR') {
      width = data.readUInt32BE(0);
      height = data.readUInt32BE(4);
      expect(data[8]).toBe(8);
      type = data[9]!;
      expect(data[12]).toBe(0);
    }
    if (kind === 'IDAT') idat.push(data);
    p += 12 + len;
  }
  const bpp = type === 6 ? 4 : type === 2 ? 3 : 0;
  expect(bpp).toBeGreaterThan(0);
  const raw = inflateSync(Buffer.concat(idat));
  const stride = width * bpp;
  const out = Buffer.alloc(stride * height);
  for (let y = 0; y < height; y++) {
    const f = raw[y * (stride + 1)]!;
    for (let i = 0; i < stride; i++) {
      const v = raw[y * (stride + 1) + 1 + i]!;
      const a = i >= bpp ? out[y * stride + i - bpp]! : 0;
      const b = y > 0 ? out[(y - 1) * stride + i]! : 0;
      const c = y > 0 && i >= bpp ? out[(y - 1) * stride + i - bpp]! : 0;
      let pred = 0;
      if (f === 1) pred = a;
      else if (f === 2) pred = b;
      else if (f === 3) pred = (a + b) >> 1;
      else if (f === 4) {
        const pa = Math.abs(b - c);
        const pb = Math.abs(a - c);
        const pc = Math.abs(a + b - 2 * c);
        pred = pa <= pb && pa <= pc ? a : pb <= pc ? b : c;
      }
      out[y * stride + i] = (v + pred) & 0xff;
    }
  }
  return {
    width,
    height,
    alpha: (x, y) => (bpp === 4 ? out[(y * width + x) * 4 + 3]! : 255),
  };
}

/** Entries of a zip package (stored or deflated), from the central directory. */
function zipEntries(zip: Buffer): Map<string, () => Buffer> {
  const out = new Map<string, () => Buffer>();
  let eocd = zip.length - 22;
  while (eocd >= 0 && zip.readUInt32LE(eocd) !== 0x06054b50) eocd--;
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

/** Part a relationship of `part` points at (`../media/x.png` → `ppt/media/x.png`). */
function relTarget(zip: Map<string, () => Buffer>, part: string, rid: string): string {
  const dir = part.slice(0, part.lastIndexOf('/'));
  const rels = zip.get(`${dir}/_rels/${part.slice(dir.length + 1)}.rels`)!().toString('utf8');
  const m =
    new RegExp(`<Relationship [^>]*Id="${rid}"[^>]*Target="([^"]+)"`).exec(rels) ??
    new RegExp(`<Relationship [^>]*Target="([^"]+)"[^>]*Id="${rid}"`).exec(rels);
  expect(m, `${part} ${rid}`).toBeTruthy();
  // ECMA-376 Part 2 targets: relative to the part's folder, or absolute from the package root.
  const target = m![1]!;
  const parts = (target.startsWith('/') ? target.slice(1) : `${dir}/${target}`).split('/');
  const stack: string[] = [];
  for (const s of parts) {
    if (s === '..') stack.pop();
    else if (s !== '.') stack.push(s);
  }
  return stack.join('/');
}

/** The blip's picture and the layer's original of the picture with a removed background. */
function removedPicture(zip: Map<string, () => Buffer>, part: string) {
  const xml = zip.get(part)!().toString('utf8');
  const pic =
    /<a:blip [^>]*r:embed="([^"]+)"[^>]*>(?:(?!<\/a:blip>).)*<a14:imgLayer r:embed="([^"]+)">(?:(?!<\/a:blip>).)*<a14:backgroundRemoval /s.exec(
      xml,
    );
  expect(pic, `${part} has no background removal`).toBeTruthy();
  return {
    shown: zip.get(relTarget(zip, part, pic![1]!))!(),
    original: zip.get(relTarget(zip, part, pic![2]!))!(),
  };
}

function expectCutOut(png: Buffer, w: number, h: number) {
  const img = decodePng(png);
  expect([img.width, img.height]).toEqual([w, h]);
  for (const [x, y] of [
    [0, 0],
    [w - 1, 0],
    [0, h - 1],
    [w - 1, h - 1],
  ] as const) {
    expect(img.alpha(x, y), `corner ${x},${y}`).toBe(0);
  }
  expect(img.alpha(w >> 1, h >> 1)).toBe(255);
}

/** Open the mode, mark the centre to keep from the keyboard and keep the change. */
async function removeBackground(page: Page) {
  await page.getByRole('tab', { name: 'Định dạng ảnh', exact: true }).click();
  const button = page.locator('[data-command="picture.removeBackground"]').first();
  await expect(button).toHaveAttribute('aria-disabled', 'false');
  await button.click();
  const dialog = page.getByTestId('picture-bg-dialog');
  await expect(dialog).toBeVisible();
  const status = page.getByTestId('picture-bg-status');
  await expect(status).toContainText('Giữ lại');
  await expect(page.getByTestId('picture-bg-preview')).toBeVisible();
  // Keyboard: the keep tool is selected; Space marks at the cursor (the centre).
  await expect(page.getByTestId('picture-bg-mark-keep')).toHaveAttribute('aria-pressed', 'true');
  await page.getByTestId('picture-bg-area').focus();
  await page.keyboard.press('Space');
  await expect(page.getByTestId('picture-bg-area')).toHaveAttribute('data-marks', '1');
  await expect(status).toContainText('Giữ lại');
  const kept = Number(/(\d+)%/.exec((await status.textContent()) ?? '')?.[1]);
  // The disk is π·0.32² ≈ 32 % of a 4:3 picture's shorter side squared → ≈ 24 % of the area.
  expect(kept).toBeGreaterThanOrEqual(20);
  expect(kept).toBeLessThanOrEqual(28);
  await page.getByTestId('picture-bg-keep').click();
  await expect(dialog).toHaveCount(0);
}

async function command(page: Page, tab: string, id: string) {
  await page.getByRole('tab', { name: tab, exact: true }).click();
  await page.locator(`[data-command="${id}"]`).first().click();
}

test('BlueSlide: remove the background of a picture, undo / redo, saved', async ({ page }) => {
  const errors: string[] = [];
  page.on('pageerror', (e) => errors.push(e.message));
  const dir = mkdtempSync(join(tmpdir(), 'bo-e2e-bg-'));
  const file = join(dir, 'hinh-tron.png');
  const original = diskPng(240, 180);
  writeFileSync(file, original);
  await page.goto('/slide');
  let chooser = page.waitForEvent('filechooser');
  await page.getByRole('button', { name: 'Mở', exact: true }).click();
  await (await chooser).setFiles(deck);
  await expect(page.getByTestId('slide-status')).toHaveText(/^Slide 1 \/ \d+$/);
  chooser = page.waitForEvent('filechooser');
  await command(page, 'Chèn', 'insert.picture');
  await (await chooser).setFiles(file);
  await removeBackground(page);
  // One undo step: undo and redo the removal.
  await command(page, 'Trang đầu', 'edit.undo');
  await command(page, 'Trang đầu', 'edit.redo');
  const download = page.waitForEvent('download');
  await page.keyboard.press('Control+s');
  const zip = zipEntries(readFileSync(await (await download).path()));
  const slides = [...zip.keys()].filter((n) => /^ppt\/slides\/slide\d+\.xml$/.test(n));
  const part = slides.find((n) => zip.get(n)!().toString('utf8').includes('a14:backgroundRemoval'));
  expect(part).toBeTruthy();
  const { shown, original: kept } = removedPicture(zip, part!);
  expectCutOut(shown, 240, 180);
  expect(kept.equals(original)).toBe(true);
  expect(errors).toEqual([]);
});

test('BlueDoc: remove the background of a picture, saved', async ({ page }) => {
  const errors: string[] = [];
  page.on('pageerror', (e) => errors.push(e.message));
  const dir = mkdtempSync(join(tmpdir(), 'bo-e2e-bg-'));
  const file = join(dir, 'hinh-tron.png');
  const original = diskPng(200, 150);
  writeFileSync(file, original);
  await page.goto('/doc');
  await page.getByTestId('new-document').click();
  await expect(page.getByTestId('caret')).toBeVisible();
  const chooser = page.waitForEvent('filechooser');
  await command(page, 'Chèn', 'insert.image');
  await (await chooser).setFiles(file);
  await removeBackground(page);
  const download = page.waitForEvent('download');
  await page.keyboard.press('Control+s');
  const zip = zipEntries(readFileSync(await (await download).path()));
  const { shown, original: kept } = removedPicture(zip, 'word/document.xml');
  expectCutOut(shown, 200, 150);
  expect(kept.equals(original)).toBe(true);
  expect(errors).toEqual([]);
});
