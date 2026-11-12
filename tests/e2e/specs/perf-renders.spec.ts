import { expect, test, type Page } from '@playwright/test';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

// Typing must not re-render the window chrome (docs/perf/RESULTS.md, step 4): the React
// commits after each keystroke are counted by UI region with the desktop perf probe
// (apps/desktop/src-tauri/src/perf-probe.js: a stand-in React DevTools hook walking the
// fibers that rendered). Ribbon and title bar: 0 per keystroke in BlueDoc, BlueSheet and
// BlueSlide; the status bar only when one of its values changed (not inside a word).

const here = dirname(fileURLToPath(import.meta.url));
const probe = join(here, '..', '..', '..', 'apps', 'desktop', 'src-tauri', 'src', 'perf-probe.js');
const deck = join(here, '..', '..', 'corpus', 'slide', 'slide-text-bullets.pptx');

test.use({ trace: 'off' });

type Regions = Record<string, number>;

/** Renders by region during `keys` typed one by one (each settled before the next). */
async function rendersWhileTyping(page: Page, keys: string): Promise<Regions[]> {
  const out: Regions[] = [];
  await page.evaluate(() => {
    (window as unknown as { __boPerf: { countRenders: boolean } }).__boPerf.countRenders = true;
  });
  for (const k of keys) {
    const from = await page.evaluate(
      () => (window as unknown as { __boPerf: { commits: unknown[] } }).__boPerf.commits.length,
    );
    await page.keyboard.press(k);
    // Settle: the engine answer, the deferred editor render, late status updates.
    await page.waitForTimeout(250);
    out.push(
      await page.evaluate((n) => {
        const P = (window as unknown as { __boPerf: { commits: { byRegion: Regions }[] } })
          .__boPerf;
        const r: Regions = {};
        for (const c of P.commits.slice(n))
          for (const [k, v] of Object.entries(c.byRegion)) r[k] = (r[k] ?? 0) + v;
        return r;
      }, from),
    );
  }
  return out;
}

const total = (list: Regions[], region: string) => list.reduce((s, r) => s + (r[region] ?? 0), 0);

test.beforeEach(async ({ page }) => {
  await page.addInitScript({ path: probe });
});

test('BlueDoc: typing does not re-render the ribbon, title bar or status bar', async ({
  page,
}, info) => {
  await page.goto('/doc');
  await page.getByTestId('new-document').click();
  await expect(page.getByTestId('caret')).toBeVisible();
  await page.keyboard.type('Xin chao ');
  await page.waitForTimeout(500);
  const r = await rendersWhileTyping(page, 'vietnam');
  await info.attach('renders.json', { body: JSON.stringify(r, null, 2) });
  // The probe saw the editor render (it counts at all).
  expect(total(r, 'other')).toBeGreaterThan(0);
  expect(total(r, 'ribbon')).toBe(0);
  expect(total(r, 'title')).toBe(0);
  // Inside a word the word count stays: the status bar does not change either.
  expect(total(r.slice(1), 'status')).toBe(0);
});

test('BlueSheet: typing in a cell does not re-render the ribbon or title bar', async ({
  page,
}, info) => {
  await page.goto('/sheet');
  await expect(page.getByTestId('sheet-canvas')).toHaveAttribute('data-rendered', /\d+/);
  await page.keyboard.type('a');
  await page.waitForTimeout(500);
  const r = await rendersWhileTyping(page, 'bcdefg');
  await info.attach('renders.json', { body: JSON.stringify(r, null, 2) });
  expect(total(r, 'ribbon')).toBe(0);
  expect(total(r, 'title')).toBe(0);
});

test('BlueSlide: typing in a text box does not re-render the ribbon or title bar', async ({
  page,
}, info) => {
  await page.goto('/slide');
  const chooser = page.waitForEvent('filechooser');
  await page.getByRole('button', { name: 'Mở', exact: true }).click();
  await (await chooser).setFiles(deck);
  await expect(page.getByTestId('slide-overlay')).toBeVisible();
  await page.getByRole('button', { name: '+ Thêm slide' }).click();
  await page.getByRole('tab', { name: 'Chèn' }).click();
  await page.getByRole('button', { name: 'Hộp văn bản' }).click();
  await page.keyboard.press('Enter');
  const tb = (await page.getByTestId('selection-box').first().boundingBox())!;
  await page.mouse.dblclick(tb.x + tb.width / 2, tb.y + tb.height / 2);
  await expect(page.getByTestId('slide-text-editor')).toBeFocused();
  await page.keyboard.type('Xin');
  await page.waitForTimeout(500);
  const r = await rendersWhileTyping(page, 'chao');
  await info.attach('renders.json', { body: JSON.stringify(r, null, 2) });
  expect(total(r, 'ribbon')).toBe(0);
  expect(total(r, 'title')).toBe(0);
});
