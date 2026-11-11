import { expect, test, type Page } from '@playwright/test';

// How BlueDoc feels under the hand (docs/parity/office-behaviour-doc.md, "Cảm giác"): the
// time from a key press to the glyph painted, from a mouse press to the caret moved, and the
// frame times of scrolling, measured with requestAnimationFrame probes in the page. The
// numbers are attached to the report; the bounds are loose (shared CI machines) and only
// catch a regression to visibly laggy typing.

// Playwright's trace snapshots the DOM at every action: it would be measured as well.
test.use({ trace: 'off' });

async function newDocument(page: Page) {
  await page.goto('/doc');
  await page.getByTestId('new-document').click();
  await expect(page.getByTestId('caret')).toBeVisible();
}

/** Installs the probes: the time of every page canvas redraw (a `getContext` per paint). */
async function installProbes(page: Page) {
  await page.evaluate(() => {
    const w = window as unknown as { __paints: number[]; __input: number };
    w.__paints = [];
    w.__input = 0;
    // The time the browser took the key / button press (the event's own time stamp).
    for (const type of ['keydown', 'mousedown'])
      window.addEventListener(type, (e) => (w.__input = e.timeStamp), true);
    const orig = HTMLCanvasElement.prototype.getContext;
    HTMLCanvasElement.prototype.getContext = function (
      this: HTMLCanvasElement,
      ...args: Parameters<typeof orig>
    ) {
      if (this.parentElement?.classList.contains('bo-page')) w.__paints.push(performance.now());
      return orig.apply(this, args);
    } as typeof orig;
  });
}

/**
 * Key press → glyph on screen: the key goes in with a real keydown at `t0`; the time is that of
 * the first animation frame after the page canvas was redrawn.
 */
async function keyToPaint(page: Page, key: string): Promise<number> {
  const t0 = await page.evaluate(() => {
    const w = window as unknown as { __t0: number; __paints: number[] };
    w.__paints = [];
    w.__t0 = performance.now();
    return w.__t0;
  });
  await page.keyboard.press(key);
  return page.evaluate(
    (start) =>
      new Promise<number>((resolve) => {
        const w = window as unknown as { __paints: number[]; __input: number };
        const tick = () => {
          if (w.__paints.some((t) => t >= start))
            requestAnimationFrame(() => resolve(performance.now() - (w.__input || start)));
          else if (performance.now() - start > 5000) resolve(5000);
          else requestAnimationFrame(tick);
        };
        requestAnimationFrame(tick);
      }),
    t0,
  );
}

/** Mouse press → caret drawn at the new place (the frame after it moved). */
async function pressToCaret(page: Page, x: number, y: number): Promise<number> {
  const t0 = await page.evaluate(() => {
    const c = document.querySelector<HTMLElement>('[data-testid="caret"]');
    const w = window as unknown as { __from: string; __t0: number };
    w.__from = c ? `${c.style.left},${c.style.top}` : '';
    w.__t0 = performance.now();
    return w.__t0;
  });
  await page.mouse.move(x, y);
  await page.mouse.down();
  const ms = await page.evaluate(
    (start) =>
      new Promise<number>((resolve) => {
        const w = window as unknown as { __from: string; __input: number };
        const tick = () => {
          const c = document.querySelector<HTMLElement>('[data-testid="caret"]');
          const at = c ? `${c.style.left},${c.style.top}` : '';
          if (c && at !== w.__from)
            requestAnimationFrame(() => resolve(performance.now() - (w.__input || start)));
          else if (performance.now() - start > 5000) resolve(5000);
          else requestAnimationFrame(tick);
        };
        requestAnimationFrame(tick);
      }),
    t0,
  );
  await page.mouse.up();
  return ms;
}

const median = (v: number[]) => [...v].sort((a, b) => a - b)[Math.floor(v.length / 2)] ?? 0;
const p95 = (v: number[]) => [...v].sort((a, b) => a - b)[Math.floor(v.length * 0.95)] ?? 0;
const round = (n: number) => Math.round(n * 10) / 10;

test('typing paints the glyph without visible lag; a click moves the caret at once', async ({
  page,
}, info) => {
  await newDocument(page);
  // A normal document: a few pages of text.
  const para = 'Hôm nay trời đẹp, chúng tôi cùng nhau soạn thảo một văn bản dài để đo độ trễ. ';
  for (let i = 0; i < 24; i++) {
    await page.keyboard.insertText(para.repeat(4));
    await page.keyboard.press('Enter');
  }
  await expect(page.getByTestId('status-page')).toHaveText(/\/[2-9]$/);
  await page.keyboard.press('Control+Home');
  await page.keyboard.press('End');
  await installProbes(page);
  // Warm up (glyph cache of the letters).
  for (const k of 'abc') await keyToPaint(page, k);
  const typing: number[] = [];
  for (const k of 'chungtoivietbaocao') typing.push(await keyToPaint(page, k));
  const enter: number[] = [];
  for (let i = 0; i < 5; i++) enter.push(await keyToPaint(page, 'Enter'));
  const back: number[] = [];
  for (let i = 0; i < 5; i++) back.push(await keyToPaint(page, 'Backspace'));

  const box = (await page.locator('.bo-page[data-page="1"]').boundingBox())!;
  const clicks: number[] = [];
  for (let i = 0; i < 8; i++) {
    clicks.push(
      await pressToCaret(page, box.x + box.width * (0.2 + 0.07 * i), box.y + 110 + 20 * (i % 4)),
    );
  }

  // Scrolling: frame times while the wheel scrolls through the pages.
  await page.mouse.move(box.x + box.width / 2, box.y + 300);
  const scrollFrames = page.evaluate(
    () =>
      new Promise<number[]>((resolve) => {
        const out: number[] = [];
        let last = performance.now();
        const end = last + 1200;
        const tick = (t: number) => {
          out.push(t - last);
          last = t;
          if (t < end) requestAnimationFrame(tick);
          else resolve(out);
        };
        requestAnimationFrame(tick);
      }),
  );
  for (let i = 0; i < 20; i++) await page.mouse.wheel(0, 120);
  const scroll = await scrollFrames;

  const report = {
    keyToPaintMs: { median: round(median(typing)), p95: round(p95(typing)) },
    enterToPaintMs: { median: round(median(enter)) },
    backspaceToPaintMs: { median: round(median(back)) },
    mouseDownToCaretMs: { median: round(median(clicks)), p95: round(p95(clicks)) },
    scrollFrameMs: { median: round(median(scroll)), p95: round(p95(scroll)) },
  };
  await info.attach('latency.json', { body: JSON.stringify(report, null, 2) });
  console.log(`BlueDoc latency ${JSON.stringify(report)}`);
  // Loose bounds: a regression to typing that visibly lags (several frames) fails.
  expect(report.keyToPaintMs.median).toBeLessThan(150);
  expect(report.mouseDownToCaretMs.median).toBeLessThan(150);
});

test('caret and selection look: thin caret of the text height, grey selection', async ({
  page,
}) => {
  await newDocument(page);
  await page.keyboard.insertText('Chữ thường');
  const caret = page.getByTestId('caret');
  const look = await caret.evaluate((c) => {
    const s = getComputedStyle(c);
    return {
      width: c.getBoundingClientRect().width,
      blink: s.animationDuration,
      height: c.getBoundingClientRect().height,
    };
  });
  // One pixel wide, 530 ms on + 530 ms off.
  expect(look.width).toBeLessThanOrEqual(1.01);
  expect(look.blink).toBe('1.06s');
  // The height of an 11 pt line of text (≈ 1.15 × 11 pt ≈ 17 px at 100 %), not of the page.
  expect(look.height).toBeGreaterThan(12);
  expect(look.height).toBeLessThan(26);
  await page.keyboard.press('Shift+Home');
  const sel = page.locator('.bo-sel').first();
  await expect(sel).toBeVisible();
  const tint = await sel.evaluate((e) => {
    const s = getComputedStyle(e);
    return { bg: s.backgroundColor, opacity: Number(s.opacity) };
  });
  // Neutral grey (no hue), translucent so the text stays readable.
  const [r, g, b, a = 1] = (tint.bg.match(/[\d.]+/g) ?? []).map(Number);
  expect(r).toBe(g);
  expect(g).toBe(b);
  expect(a * tint.opacity).toBeGreaterThan(0.1);
  expect(a * tint.opacity).toBeLessThan(0.35);
});
