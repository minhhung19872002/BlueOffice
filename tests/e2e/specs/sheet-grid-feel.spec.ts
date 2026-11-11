import { expect, test, type Page } from '@playwright/test';

// Look and feel of the BlueSheet grid against the common spreadsheet conventions (black-box
// knowledge of the desktop spreadsheet the project owner compares with):
//  * a click selects on mouse-DOWN: the selection is drawn in the very next frame, with no
//    engine round trip in between and no intermediate state;
//  * dragging extends the range live; Shift+click extends from the active cell; Ctrl+click
//    adds another range; a header click selects the whole column, dragging over the headers
//    several;
//  * the headers of the selected rows / columns are highlighted (darker band, accent line on
//    the edge towards the cells); light-grey 1 px grid lines on whole device pixels; the
//    active cell unshaded inside a shaded range with a 2 px border;
//  * the fill handle shows a thin plus cursor and a grey dashed outline while dragged;
//  * a double-click edits in place with the caret at the click point and the text where the
//    cell showed it;
//  * dragging a header boundary shows a line and a size tip (64 px = 8.43 characters);
//  * one wheel notch scrolls three rows.

interface Probe {
  t0: number;
  /** Selection box right after the mousedown was dispatched (before any frame). */
  sync: string;
  syncSel: string;
  frames: { t: number; box: string; sel: string }[];
}

const boxOfJs = () => {
  const el = document.querySelector('[data-testid="fill-handle"]')?.parentElement;
  if (!el) return '';
  const r = el.getBoundingClientRect();
  return `${Math.round(r.left)},${Math.round(r.top)},${Math.round(r.width)},${Math.round(r.height)}`;
};

/** Installs a probe: on the next mousedown, records the selection right away and every frame. */
async function armProbe(page: Page) {
  await page.evaluate(`(() => {
    const boxOf = ${boxOfJs.toString()};
    const sel = () => document.querySelector('[data-testid="sheet-overlay"]')?.dataset.sel ?? '';
    const p = { t0: 0, sync: '', syncSel: '', frames: [] };
    window.__gridProbe = p;
    window.addEventListener('mousedown', () => {
      p.t0 = performance.now();
      const tick = (t) => {
        p.frames.push({ t: t - p.t0, box: boxOf(), sel: sel() });
        if (p.frames.length < 20) requestAnimationFrame(tick);
      };
      requestAnimationFrame(tick);
    }, { capture: true, once: true });
    // Bubble phase on the window: runs after the grid's own handler (React's root listener).
    window.addEventListener('mousedown', () => {
      p.sync = boxOf();
      p.syncSel = sel();
    }, { once: true });
  })()`);
}

async function probeResult(page: Page) {
  await page.waitForFunction(
    () => (window as unknown as { __gridProbe: Probe }).__gridProbe.frames.length >= 20,
  );
  return page.evaluate(() => (window as unknown as { __gridProbe: Probe }).__gridProbe);
}

async function openSheet(page: Page) {
  const errors: string[] = [];
  page.on('pageerror', (e) => errors.push(e.message));
  await page.goto('/sheet');
  await expect(page.getByTestId('sheet-canvas')).toHaveAttribute('data-rendered', /\d+/);
  await expect(page.getByTestId('sheet-overlay')).toHaveAttribute('data-sel', 'A1');
  return errors;
}

const canvasBox = async (page: Page) => (await page.getByTestId('sheet-canvas').boundingBox())!;

/** Client point inside cell (row, col) at 100 % (64 × 20 px cells after 40 × 20 px headers). */
async function cellPoint(page: Page, row: number, col: number, dx = 32, dy = 10) {
  const c = await canvasBox(page);
  return { x: c.x + 40 + col * 64 + dx, y: c.y + 20 + row * 20 + dy };
}

async function clickCell(
  page: Page,
  row: number,
  col: number,
  modifiers: ('Shift' | 'Control')[] = [],
) {
  const p = await cellPoint(page, row, col);
  for (const m of modifiers) await page.keyboard.down(m);
  await page.mouse.click(p.x, p.y);
  for (const m of modifiers) await page.keyboard.up(m);
}

async function goTo(page: Page, ref: string) {
  const box = page.getByLabel('Hộp tên');
  await box.click();
  await box.fill(ref);
  await box.press('Enter');
  await expect(page.getByLabel('Ô đang nhập')).toBeFocused();
}

/** RGBA of the grid as shown (grid canvas + selection layer) at a CSS point of the canvas. */
async function pixel(page: Page, x: number, y: number): Promise<number[]> {
  return page.evaluate(
    ([x, y]) => {
      const base = document.querySelector<HTMLCanvasElement>('[data-testid="sheet-canvas"]')!;
      const over = document.querySelector<HTMLCanvasElement>('[data-testid="sheet-overlay"]')!;
      const dpr = base.width / base.getBoundingClientRect().width;
      const px = Math.floor(x * dpr);
      const py = Math.floor(y * dpr);
      const c = document.createElement('canvas');
      c.width = 1;
      c.height = 1;
      const ctx = c.getContext('2d')!;
      ctx.drawImage(base, px, py, 1, 1, 0, 0, 1, 1);
      ctx.drawImage(over, px, py, 1, 1, 0, 0, 1, 1);
      return Array.from(ctx.getImageData(0, 0, 1, 1).data);
    },
    [x, y] as const,
  );
}

const rgb = (p: number[]) => p.slice(0, 3);

/** The sheet accent as the page resolves it. */
async function accent(page: Page): Promise<number[]> {
  return page.evaluate(() => {
    const el = document.querySelector('[data-testid="sheet-canvas"]')!.closest('div[class]')!;
    const v = getComputedStyle(el).getPropertyValue('--bo-sheet-accent').trim() || '#0b7a6b';
    const n = parseInt(v.replace('#', ''), 16);
    return [(n >> 16) & 255, (n >> 8) & 255, n & 255];
  });
}

test('BlueSheet grid: a click selects on mouse-down, drawn in the first frame, no intermediate state', async ({
  page,
}) => {
  const errors = await openSheet(page);
  const latencies: number[] = [];
  for (const [row, col, ref] of [
    [4, 3, 'D5'],
    [9, 1, 'B10'],
    [2, 6, 'G3'],
    [12, 4, 'E13'],
  ] as const) {
    const p = await cellPoint(page, row, col);
    await page.mouse.move(p.x, p.y);
    await armProbe(page);
    await page.mouse.down();
    const r = await probeResult(page);
    // Still holding the button: the grid already selected the cell, synchronously in the
    // mousedown dispatch (no engine round trip before drawing).
    expect(r.syncSel).toBe(ref);
    const [l, t, w, h] = r.sync.split(',').map(Number) as [number, number, number, number];
    expect(p.x >= l && p.x < l + w && p.y >= t && p.y < t + h).toBe(true);
    // ...so the first frame after the mousedown shows it, and nothing changes afterwards.
    expect(r.frames[0]!.sel).toBe(ref);
    expect(new Set(r.frames.map((f) => `${f.box}|${f.sel}`)).size).toBe(1);
    await page.mouse.up();
    latencies.push(r.frames[0]!.t);
  }
  console.log(
    `grid mousedown → first frame with the selection (ms): ${latencies.map((l) => l.toFixed(1)).join(', ')}`,
  );
  // Budget for a loaded headless machine (the frame clock itself is ~16 ms on a desktop).
  for (const l of latencies) expect(l).toBeLessThan(50);
  expect(errors).toEqual([]);
});

test('BlueSheet grid: dragging extends the range live; Shift / Ctrl clicks; header clicks and drags', async ({
  page,
}) => {
  const errors = await openSheet(page);
  const overlay = page.getByTestId('sheet-overlay');
  const a = await cellPoint(page, 1, 1);
  await page.mouse.move(a.x, a.y);
  await page.mouse.down();
  for (const [row, col, ref] of [
    [2, 2, 'B2:C3'],
    [4, 3, 'B2:D5'],
  ] as const) {
    const p = await cellPoint(page, row, col);
    await page.mouse.move(p.x, p.y, { steps: 3 });
    // Before the button is released, in the frame after the move.
    expect(await overlay.getAttribute('data-sel')).toBe(ref);
  }
  await page.mouse.up();
  await expect(page.getByLabel('Hộp tên')).toHaveValue('B2:D5');

  // Shift+click extends from the active cell (B2).
  await clickCell(page, 1, 1);
  await clickCell(page, 5, 2, ['Shift']);
  await expect(overlay).toHaveAttribute('data-sel', 'B2:C6');
  await expect(page.getByLabel('Hộp tên')).toHaveValue('B2:C6');
  // Ctrl+click adds a range: the first one stays drawn.
  await clickCell(page, 8, 4, ['Control']);
  await expect(overlay).toHaveAttribute('data-sel', 'E9');
  await expect(page.getByTestId('selection-extra')).toHaveCount(1);

  // A column header selects the column; dragging over the headers selects several.
  const c = await canvasBox(page);
  await page.mouse.move(c.x + 40 + 64 * 2 + 30, c.y + 10);
  await expect(page.getByTestId('sheet-canvas')).toHaveCSS('cursor', /url\(/);
  await page.mouse.down();
  await expect(overlay).toHaveAttribute('data-sel', /^C1:C\d+$/);
  await page.mouse.move(c.x + 40 + 64 * 4 + 30, c.y + 10, { steps: 3 });
  expect(await overlay.getAttribute('data-sel')).toMatch(/^C1:E\d+$/);
  await page.mouse.up();
  await expect(page.getByTestId('selection-extra')).toHaveCount(0);
  // Row headers likewise.
  await page.mouse.click(c.x + 20, c.y + 20 + 20 * 3 + 10);
  await expect(overlay).toHaveAttribute('data-sel', /^A4:[A-Z]+4$/);
  expect(errors).toEqual([]);
});

test('BlueSheet grid: neutral grid lines, highlighted headers, unshaded active cell, 2 px border (pixels)', async ({
  page,
}) => {
  const errors = await openSheet(page);
  const acc = await accent(page);
  // Select B2:C4 by dragging (active cell B2).
  const a = await cellPoint(page, 1, 1);
  const b = await cellPoint(page, 3, 2);
  await page.mouse.move(a.x, a.y);
  await page.mouse.down();
  await page.mouse.move(b.x, b.y, { steps: 2 });
  await page.mouse.up();
  await expect(page.getByTestId('sheet-overlay')).toHaveAttribute('data-sel', 'B2:C4');
  // Light-grey 1 px grid line between F and G, white either side.
  expect(rgb(await pixel(page, 40 + 64 * 6 - 1, 300))).toEqual([212, 212, 212]);
  expect(rgb(await pixel(page, 40 + 64 * 6 - 2, 300))).toEqual([255, 255, 255]);
  expect(rgb(await pixel(page, 40 + 64 * 6, 300))).toEqual([255, 255, 255]);
  // Header band of an unselected column: light grey.
  expect(rgb(await pixel(page, 40 + 64 * 5 + 6, 4))).toEqual([243, 243, 243]);
  // Header of a selected column: darker band (12 % accent over #DCDCDC), accent line at its
  // bottom edge; the row header of a selected row has the line on its right edge.
  const band = [220, 220, 220].map((v, i) => Math.round(v + (acc[i]! - v) * 0.12));
  expect(rgb(await pixel(page, 40 + 64 + 6, 4))).toEqual(band);
  expect(rgb(await pixel(page, 40 + 64 + 6, 19))).toEqual(acc);
  expect(rgb(await pixel(page, 39, 20 + 20 * 2 + 10))).toEqual(acc);
  // Border: 2 px on the grid line before B and one pixel inside; nothing outside.
  expect(rgb(await pixel(page, 40 + 64 - 1, 70))).toEqual(acc);
  expect(rgb(await pixel(page, 40 + 64, 70))).toEqual(acc);
  expect(rgb(await pixel(page, 40 + 64 - 2, 70))).toEqual([255, 255, 255]);
  // The far side: the grid line after C and one pixel past it.
  expect(rgb(await pixel(page, 40 + 64 * 3 - 1, 70))).toEqual(acc);
  expect(rgb(await pixel(page, 40 + 64 * 3, 70))).toEqual(acc);
  expect(rgb(await pixel(page, 40 + 64 * 3 + 1, 70))).toEqual([255, 255, 255]);
  // The active cell B2 stays white; the rest of the range is shaded grey.
  expect(rgb(await pixel(page, 40 + 64 + 30, 20 + 20 + 10))).toEqual([255, 255, 255]);
  const shade = await pixel(page, 40 + 64 * 2 + 30, 20 + 20 * 2 + 10);
  expect(rgb(shade)).toEqual([225, 225, 225]);
  expect(errors).toEqual([]);
});

for (const dpr of [1.25, 1.5]) {
  test(`BlueSheet grid: 1 px grid lines stay sharp at ${dpr * 100} %`, async ({ browser }) => {
    const ctx = await browser.newContext({
      deviceScaleFactor: dpr,
      viewport: { width: 1000, height: 600 },
    });
    const page = await ctx.newPage();
    await openSheet(page);
    // A horizontal device-pixel strip across columns D..H of row 10: only white and the
    // grid-line grey, no anti-aliased in-between values.
    const values = await page.evaluate(() => {
      const base = document.querySelector<HTMLCanvasElement>('[data-testid="sheet-canvas"]')!;
      const r = base.getBoundingClientRect();
      const k = base.width / r.width;
      const y = Math.floor((20 + 20 * 9 + 10) * k);
      const x0 = Math.floor((40 + 64 * 3) * k);
      const data = base.getContext('2d')!.getImageData(x0, y, Math.floor(64 * 5 * k), 1).data;
      const out = new Set<number>();
      for (let i = 0; i < data.length; i += 4) out.add(data[i]!);
      return [...out].sort((a, b) => a - b);
    });
    expect(values).toEqual([212, 255]);
    await ctx.close();
  });
}

test('BlueSheet grid: cursors, fill handle drag outline, moving the selection by its border', async ({
  page,
}) => {
  const errors = await openSheet(page);
  const canvas = page.getByTestId('sheet-canvas');
  await goTo(page, 'B2');
  await page.keyboard.type('5');
  await page.keyboard.press('Enter');
  await clickCell(page, 1, 1);
  // Thick plus over the cells; four-way arrow on the selection border.
  const inCell = await cellPoint(page, 4, 4);
  await page.mouse.move(inCell.x, inCell.y);
  await expect(canvas).toHaveCSS('cursor', 'cell');
  const edge = await cellPoint(page, 1, 1, 32, 0);
  await page.mouse.move(edge.x, edge.y);
  await expect(canvas).toHaveCSS('cursor', 'move');
  // Thin plus on the fill handle; dragging it shows a grey dashed outline of the target.
  const handle = page.getByTestId('fill-handle');
  await expect(handle).toHaveCSS('cursor', 'crosshair');
  const hb = (await handle.boundingBox())!;
  await page.mouse.move(hb.x + hb.width / 2, hb.y + hb.height / 2);
  await page.mouse.down();
  const to = await cellPoint(page, 4, 1);
  await page.mouse.move(to.x, to.y, { steps: 4 });
  const preview = page.getByTestId('fill-preview');
  await expect(preview).toBeVisible();
  await expect(preview).toHaveCSS('border-top-style', 'dashed');
  await expect(preview).toHaveCSS('border-top-color', 'rgb(127, 127, 127)');
  const pb = (await preview.boundingBox())!;
  const c = await canvasBox(page);
  // B2:B5.
  expect(Math.round(pb.x - c.x)).toBe(40 + 64);
  expect(Math.round(pb.height)).toBe(80);
  await page.mouse.up();
  await expect(preview).toHaveCount(0);
  await expect(page.getByLabel('Hộp tên')).toHaveValue('B2:B5');

  // Drag the selection by its border: B2:B5 moves to D2:D5 (one undo step).
  const from = await cellPoint(page, 1, 1, 32, 0);
  await page.mouse.move(from.x, from.y);
  await expect(canvas).toHaveCSS('cursor', 'move');
  await page.mouse.down();
  const drop = await cellPoint(page, 1, 3, 32, 0);
  await page.mouse.move(drop.x, drop.y, { steps: 4 });
  await expect(preview).toBeVisible();
  await page.mouse.up();
  await expect(page.getByLabel('Hộp tên')).toHaveValue('D2:D5');
  await goTo(page, 'D2');
  await expect(page.getByLabel('Thanh công thức')).toHaveValue('5');
  await goTo(page, 'B2');
  await expect(page.getByLabel('Thanh công thức')).toHaveValue('');
  await page.keyboard.press('Control+z');
  await expect(page.getByLabel('Thanh công thức')).toHaveValue('5');
  expect(errors).toEqual([]);
});

test('BlueSheet grid: double-click edits in place, caret at the click point, text does not move', async ({
  page,
}) => {
  const errors = await openSheet(page);
  await goTo(page, 'A2');
  await page.keyboard.type('abcdefgh');
  await page.keyboard.press('Enter');
  await clickCell(page, 5, 5);
  const c = await canvasBox(page);
  const clip = { x: c.x + 40, y: c.y + 20 + 20, width: 64, height: 20 };
  /** Ink bounds of a screenshot clip: leftmost dark column and lowest dark row. */
  const ink = async () => {
    const png = (await page.screenshot({ clip })).toString('base64');
    return page.evaluate(async (b64) => {
      const img = new Image();
      img.src = `data:image/png;base64,${b64}`;
      await img.decode();
      const cv = document.createElement('canvas');
      cv.width = img.width;
      cv.height = img.height;
      const ctx = cv.getContext('2d')!;
      ctx.drawImage(img, 0, 0);
      const d = ctx.getImageData(0, 0, img.width, img.height).data;
      let left = img.width;
      let bottom = -1;
      // Dark = text; the 2 px frame of the selection / editor is skipped (3 px margin).
      for (let y = 3; y < img.height - 3; y++)
        for (let x = 3; x < img.width - 3; x++) {
          const i = (y * img.width + x) * 4;
          if (d[i]! < 110 && d[i + 1]! < 110 && d[i + 2]! < 110) {
            left = Math.min(left, x);
            bottom = Math.max(bottom, y);
          }
        }
      return { left, bottom };
    }, png);
  };
  const shown = await ink();
  // Double-click near the left edge: in-place editing with the caret before the first letter.
  const left = await cellPoint(page, 1, 0, 3);
  await page.mouse.dblclick(left.x, left.y);
  const input = page.getByLabel('Ô đang nhập');
  await expect(input).toHaveValue('abcdefgh');
  await expect.poll(() => input.evaluate((el: HTMLInputElement) => el.selectionStart)).toBe(0);
  const editing = await ink();
  // The text stays where the cell showed it (within a pixel).
  expect(Math.abs(editing.left - shown.left)).toBeLessThanOrEqual(1);
  expect(Math.abs(editing.bottom - shown.bottom)).toBeLessThanOrEqual(1);
  await page.keyboard.press('Escape');
  // Double-click in the middle of the text: the caret lands inside it, not at the end.
  const mid = await cellPoint(page, 1, 0, 24);
  await page.mouse.dblclick(mid.x, mid.y);
  await expect(input).toHaveValue('abcdefgh');
  await expect
    .poll(() => input.evaluate((el: HTMLInputElement) => el.selectionStart ?? -1))
    .toBeGreaterThan(1);
  expect(await input.evaluate((el: HTMLInputElement) => el.selectionStart)).toBeLessThan(7);
  // Typing grows the editor to the right.
  const w0 = (await input.boundingBox())!.width;
  await page.keyboard.press('End');
  await page.keyboard.type(' và thêm nhiều chữ nữa');
  expect((await input.boundingBox())!.width).toBeGreaterThan(w0 + 40);
  await page.keyboard.press('Escape');
  expect(errors).toEqual([]);
});

test('BlueSheet grid: header boundary drag shows the line and the size tip; wheel scrolls 3 rows', async ({
  page,
}) => {
  const errors = await openSheet(page);
  const c = await canvasBox(page);
  const canvas = page.getByTestId('sheet-canvas');
  // Boundary between B and C.
  const x = c.x + 40 + 64 * 2;
  await page.mouse.move(x, c.y + 10);
  await expect(canvas).toHaveCSS('cursor', 'col-resize');
  await page.mouse.down();
  await page.mouse.move(x + 20, c.y + 10, { steps: 4 });
  // 84 px: (84 − 5) / 7 = 11.29 characters (7 px digits + 5 px padding at 100 %).
  await expect(page.getByTestId('resize-tip')).toHaveText('Rộng: 11,29 (84 điểm ảnh)');
  await page.mouse.up();
  await expect(page.getByTestId('resize-tip')).toHaveCount(0);
  // The column is wider: C starts 20 px later.
  await expect(async () => {
    await page.mouse.click(x + 20 + 5, c.y + 30);
    await expect(page.getByLabel('Hộp tên')).toHaveValue('C1');
  }).toPass();
  // Row boundary: the tip gives the height in points.
  const y = c.y + 20 + 20 * 3;
  await page.mouse.move(c.x + 20, y);
  await expect(canvas).toHaveCSS('cursor', 'row-resize');
  await page.mouse.down();
  await page.mouse.move(c.x + 20, y + 10, { steps: 3 });
  await expect(page.getByTestId('resize-tip')).toHaveText('Cao: 22,50 (30 điểm ảnh)');
  await page.mouse.up();

  // One wheel notch: three rows.
  const scroller = page.getByTestId('sheet-canvas').locator('xpath=../..');
  await page.mouse.move(c.x + 300, c.y + 200);
  await page.mouse.wheel(0, 100);
  await expect.poll(() => scroller.evaluate((el) => el.scrollTop)).toBe(60 + 10);
  expect(errors).toEqual([]);
});

// Screenshots for a side-by-side review (not committed): GRID_SHOTS=<dir>.
const SHOTS = process.env.GRID_SHOTS;
for (const dpr of [1, 1.25, 1.5]) {
  test(`BlueSheet grid screenshot at ${dpr * 100} %`, async ({ browser }) => {
    test.skip(!SHOTS, 'GRID_SHOTS not set');
    const ctx = await browser.newContext({
      deviceScaleFactor: dpr,
      viewport: { width: 1100, height: 700 },
    });
    const page = await ctx.newPage();
    await openSheet(page);
    const box = page.getByLabel('Hộp tên');
    const put = async (ref: string, v: string) => {
      await box.click();
      await box.fill(ref);
      await box.press('Enter');
      await page.keyboard.type(v);
      await page.keyboard.press('Enter');
    };
    await put('A1', 'Sản phẩm');
    await put('B1', 'Số lượng');
    await put('A2', 'Bút bi');
    await put('B2', '120');
    await put('A3', 'Vở');
    await put('B3', '45,5');
    await put('C2', 'TRUE');
    await box.click();
    await box.fill('B2:C4');
    await box.press('Enter');
    await page.waitForTimeout(500);
    await page.screenshot({ path: `${SHOTS}/grid-${dpr * 100}.png` });
    await ctx.close();
  });
}
