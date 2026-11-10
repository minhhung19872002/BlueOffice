import { expect, test, type Page } from '@playwright/test';
import { readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { inflateRawSync } from 'node:zlib';

/**
 * CORE-INK-002: draw a rough rectangle and a rough circle with the mouse on the Draw tab, convert
 * them to shapes ("Chuyển nét vẽ thành hình"), press undo / redo (the single undo step itself is
 * asserted in crates/bo-engine/tests/slide_ink_convert.rs), draw
 * another rectangle in the automatic mode, and find the preset shapes in the saved file. Without
 * a handwriting recogniser the ink → text / math commands are disabled.
 */

const here = dirname(fileURLToPath(import.meta.url));
const deck = join(here, '..', '..', 'corpus', 'slide', 'slide-text-bullets.pptx');

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

async function openDeck(page: Page) {
  await page.getByRole('tab', { name: 'Trang đầu' }).click();
  const chooser = page.waitForEvent('filechooser');
  await page.getByRole('button', { name: 'Mở', exact: true }).click();
  await (await chooser).setFiles(deck);
  await expect(page.getByTestId('slide-status')).toHaveText(/^Slide 1 \/ \d+$/);
  await expect(page.getByTestId('slide-overlay')).toBeVisible();
}

/** Drag the mouse through `pts` (fractions of the slide) like a hand: small steps, a little wobble. */
async function scribble(page: Page, pts: [number, number][]) {
  const box = (await page.getByTestId('draw-layer').boundingBox())!;
  const at = (p: [number, number]) =>
    [box.x + p[0] * box.width, box.y + p[1] * box.height] as const;
  const [x0, y0] = at(pts[0]!);
  await page.mouse.move(x0, y0);
  await page.mouse.down();
  for (let i = 1; i < pts.length; i++) {
    const [ax, ay] = at(pts[i - 1]!);
    const [bx, by] = at(pts[i]!);
    const n = Math.max(2, Math.ceil(Math.hypot(bx - ax, by - ay) / 6));
    for (let s = 1; s <= n; s++) {
      const t = s / n;
      const wob = s % 2 === 0 ? 0.7 : -0.7;
      await page.mouse.move(ax + t * (bx - ax) + wob, ay + t * (by - ay) - wob);
    }
  }
  await page.mouse.up();
}

/** A rough rectangle: not quite square corners, ends a little past the start. */
const rect = (x: number, y: number, w: number, h: number): [number, number][] => [
  [x, y],
  [x + w, y + 0.006],
  [x + w - 0.004, y + h],
  [x + 0.003, y + h - 0.005],
  [x + 0.004, y + 0.01],
  [x + 0.04, y + 0.004],
];

/** A rough circle (an ellipse in slide fractions, 16:9), drawn round once. */
const circle = (cx: number, cy: number, r: number): [number, number][] =>
  Array.from({ length: 37 }, (_, i) => {
    const a = (i / 36) * 2 * Math.PI;
    const rr = r * (1 + (i % 3 === 0 ? 0.03 : -0.015));
    return [cx + rr * Math.cos(a), cy + ((rr * 16) / 9) * Math.sin(a)] as [number, number];
  });

test('rough ink becomes a rectangle and a circle', async ({ page }) => {
  const errors: string[] = [];
  page.on('pageerror', (e) => errors.push(e.message));
  await page.goto('/slide');
  await openDeck(page);

  await page.getByRole('tab', { name: 'Vẽ', exact: true }).click();
  // No handwriting recogniser here: ink → text / math are disabled.
  await expect(page.locator('[data-command="draw.inkToText"]').first()).toBeDisabled();
  await expect(page.locator('[data-command="draw.inkToMath"]').first()).toBeDisabled();

  await page.locator('[data-command="draw.pen"]').first().click();
  await expect(page.getByTestId('draw-layer')).toBeVisible();
  await scribble(page, rect(0.1, 0.55, 0.3, 0.25));
  await scribble(page, circle(0.7, 0.68, 0.1));

  await page.locator('[data-command="draw.inkToShape"]').first().click();
  await expect(page.getByTestId('slide-status')).toHaveText(
    'Đã chuyển 2 hình; 0 nét vẽ giữ nguyên.',
  );

  // One undo step brings the ink back; redo converts again.
  await page.keyboard.press('Escape');
  await page.getByTestId('slide-overlay').focus();
  await page.keyboard.press('Control+z');
  await page.keyboard.press('Control+y');

  // The automatic mode: a rectangle becomes a shape as soon as the mouse is released.
  await page.locator('[data-command="draw.autoShape"]').first().click();
  await expect(page.locator('[data-command="draw.autoShape"]').first()).toHaveAttribute(
    'aria-pressed',
    'true',
  );
  await scribble(page, rect(0.45, 0.1, 0.2, 0.15));
  await page.keyboard.press('Escape');

  const download = page.waitForEvent('download');
  await page.keyboard.press('Control+s');
  const parts = unzip(readFileSync(await (await download).path()));
  const slide = parts.get('ppt/slides/slide1.xml')!.toString('utf8');
  expect(slide.match(/<a:prstGeom prst="rect">/g)?.length ?? 0).toBeGreaterThanOrEqual(2);
  expect(slide).toContain('<a:prstGeom prst="ellipse">');
  // The pen's colour (black preset) and width (3 pt) are kept, and no ink is left.
  expect(slide).toContain('<a:ln w="38100" cap="rnd"><a:solidFill><a:srgbClr val="1B1B1B"/>');
  expect([...parts.keys()].some((k) => k.startsWith('ppt/ink/'))).toBe(false);
  expect(errors).toEqual([]);
});
