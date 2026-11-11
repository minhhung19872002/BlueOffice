import { expect, test, type Page } from '@playwright/test';
import { readFileSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

// SLIDE-ANIM-003 / 004: effects run word by word and letter by letter, colour changes shown in
// the show, see-through layers, motion paths drawn on the slide (reverse, lock, edit points) and
// Morph by word.

const here = dirname(fileURLToPath(import.meta.url));
const corpus = join(here, '..', '..', 'corpus', 'slide');
const deck = join(corpus, 'slide-edit-anim-text-paths.pptx');
const bullets = join(corpus, 'slide-text-bullets.pptx');

/** Open the animation pane (the ribbon button toggles it). */
async function showPane(page: Page) {
  await page.getByRole('tab', { name: 'Hiệu ứng' }).click();
  if (!(await page.getByTestId('anim-pane').isVisible())) {
    await page.getByRole('button', { name: 'Ngăn hiệu ứng' }).first().click();
  }
  await expect(page.getByTestId('anim-pane')).toBeVisible();
}

async function openDeck(page: Page, path: string) {
  await page.getByRole('tab', { name: 'Trang đầu' }).click();
  const chooser = page.waitForEvent('filechooser');
  await page.getByRole('button', { name: 'Mở', exact: true }).click();
  await (await chooser).setFiles(path);
  await expect(page.getByTestId('slide-status')).toHaveText(/^Slide 1 \/ \d+$/);
  await expect(page.getByTestId('slide-overlay')).toBeVisible();
}

/** RGBA of the canvas under slide point (`x`, `y`) of a stage layer (`null`: no canvas there). */
async function pixel(page: Page, selector: string, x: number, y: number): Promise<number[] | null> {
  return page.evaluate(
    ({ selector, x, y }) => {
      const el = document.querySelector(selector);
      const stage = document.querySelector('[data-testid="show-stage"]');
      if (!el || !stage) return null;
      const k = stage.getBoundingClientRect().width / 960;
      const s = stage.getBoundingClientRect();
      for (const c of Array.from(el.querySelectorAll('canvas'))) {
        const r = c.getBoundingClientRect();
        const px = s.left + x * k - r.left;
        const py = s.top + y * k - r.top;
        if (px < 0 || py < 0 || px >= r.width || py >= r.height) continue;
        const d = c
          .getContext('2d')
          ?.getImageData(
            Math.floor((px * c.width) / r.width),
            Math.floor((py * c.height) / r.height),
            1,
            1,
          ).data;
        if (d) return Array.from(d);
      }
      return null;
    },
    { selector, x, y },
  );
}

test('plays text by word / letter, colour changes and see-through layers in the show', async ({
  page,
}) => {
  const errors: string[] = [];
  page.on('pageerror', (e) => errors.push(e.message));
  await page.goto('/slide');
  await openDeck(page, deck);
  await page.getByTestId('thumb-2').click();
  await expect(page.getByTestId('slide-status')).toHaveText(/^Slide 2 \//);
  await showPane(page);
  await expect(page.getByTestId('anim-text-by-0')).toHaveValue('word');
  await expect(page.getByTestId('anim-text-by-1')).toHaveValue('letter');
  await expect(page.getByTestId('anim-text-delay-1')).toHaveValue('20');

  await page.keyboard.press('Shift+F5');
  const stage = page.getByTestId('show-stage');
  await expect(stage).toHaveAttribute('data-slide', '1');
  await expect(stage).toHaveAttribute('data-clicks', '3');
  // "Doanh thu quý ba tăng mạnh" (21 letters) and "Từng chữ một" (10).
  await expect(page.getByTestId('show-piece')).toHaveCount(31);
  // Layers over the backdrop are see-through: the ellipse (centre 120, 340 pt) shows through the
  // layer of the rounded rectangle above it.
  const top = '[data-testid="show-layer"]:last-of-type';
  // Drawn once the glyphs are in: the rectangle itself is opaque…
  await expect.poll(async () => (await pixel(page, top, 680, 345))?.[3]).toBe(255);
  // …and the rest of its layer is see-through.
  expect((await pixel(page, top, 120, 340))?.[3]).toBe(0);
  // The colour drawing is not shown before its click.
  const tint = page.getByTestId('show-tint');
  await expect(tint).toHaveAttribute('data-shown', 'false');
  for (const click of ['1', '2', '3']) {
    await page.keyboard.press('ArrowRight');
    await expect(stage).toHaveAttribute('data-click', click);
  }
  // After the fill colour change: green (00B050) at the rounded rectangle's centre.
  await expect(tint).toHaveAttribute('data-shown', 'true', { timeout: 10_000 });
  await expect
    .poll(async () => (await pixel(page, '[data-testid="show-tint"]', 680, 345))?.slice(0, 3))
    .toEqual([0, 176, 80]);
  // Morph by word into slide 4: the words travel on canvases of their own.
  await page.keyboard.press('ArrowRight');
  await expect(stage).toHaveAttribute('data-slide', '2');
  await page.keyboard.press('ArrowRight');
  await expect(stage).toHaveAttribute('data-slide', '3');
  await expect(stage).toHaveAttribute('data-transition', 'morph');
  await page.keyboard.press('Escape');
  await expect(page.getByTestId('slide-overlay')).toBeVisible();
  expect(errors).toEqual([]);
});

test('draws a motion path, reverses, locks and edits it, saves and reopens', async ({ page }) => {
  const errors: string[] = [];
  page.on('pageerror', (e) => errors.push(e.message));
  await page.goto('/slide');
  await openDeck(page, bullets);
  const o = (await page.getByTestId('slide-overlay').boundingBox())!;
  await page.mouse.click(o.x + o.width * 0.48, o.y + o.height * 0.3);
  await expect(page.getByTestId('selection-box')).toHaveCount(1);
  await page.getByRole('tab', { name: 'Hiệu ứng' }).click();
  await page.getByRole('button', { name: 'Vẽ đường chuyển động' }).click();
  const draw = page.getByTestId('motion-paths');
  await expect(draw).toBeVisible();
  // Two corners: a quarter of the slide right and a tenth down; Enter ends the path.
  const b = (await draw.boundingBox())!;
  await page.mouse.click(b.x + b.width * 0.3, b.y + b.height * 0.5);
  await page.mouse.click(b.x + b.width * 0.55, b.y + b.height * 0.6);
  await page.keyboard.press('Enter');
  await showPane(page);
  await expect(page.getByTestId('anim-item-0')).toContainText('Đường tự vẽ');
  await expect(page.getByTestId('motion-path-0')).toBeVisible();
  // Reverse: the path now ends where it started.
  const before = await page.getByTestId('motion-path-0').getAttribute('d');
  await page.getByTestId('anim-reverse-path-0').click();
  await expect(page.getByTestId('motion-path-0')).not.toHaveAttribute('d', before ?? '');
  await page.getByTestId('anim-lock-path-0').check();
  await expect(page.getByTestId('anim-lock-path-0')).toBeChecked();
  // Edit points: drag the end a little.
  await page.getByTestId('anim-edit-path-0').click();
  const end = page.getByTestId('motion-point-0-1');
  const e = (await end.boundingBox())!;
  const reversed = await page.getByTestId('motion-path-0').getAttribute('d');
  await page.mouse.move(e.x + e.width / 2, e.y + e.height / 2);
  await page.mouse.down();
  await page.mouse.move(e.x + e.width / 2 + 30, e.y + e.height / 2 + 20, { steps: 4 });
  await page.mouse.up();
  await expect(page.getByTestId('motion-path-0')).not.toHaveAttribute('d', reversed ?? '');
  const edited = await page.getByTestId('motion-path-0').getAttribute('d');

  // Saved and opened again: the same path, still locked.
  const download = page.waitForEvent('download');
  await page.keyboard.press('Control+s');
  const copy = join(tmpdir(), `bo-e2e-${Date.now()}-path.pptx`);
  writeFileSync(copy, readFileSync(await (await download).path()));
  await openDeck(page, copy);
  await page.mouse.click(o.x + o.width * 0.48, o.y + o.height * 0.3);
  await showPane(page);
  await expect(page.getByTestId('anim-lock-path-0')).toBeChecked();
  await expect(page.getByTestId('motion-path-0')).toHaveAttribute('d', edited ?? '');
  expect(errors).toEqual([]);
});
