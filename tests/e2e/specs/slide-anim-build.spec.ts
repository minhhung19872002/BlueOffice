import { expect, test, type Page } from '@playwright/test';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

// SLIDE-ANIM-003 / 004: text built paragraph by paragraph, a chart by series, SmartArt node by
// node, dim / hide after the effect, and effects reordered by dragging them in the pane.

const here = dirname(fileURLToPath(import.meta.url));
const deck = join(here, '..', '..', 'corpus', 'slide', 'slide-edit-anim-build-elements.pptx');

async function showPane(page: Page) {
  await page.getByRole('tab', { name: 'Hiệu ứng' }).click();
  if (!(await page.getByTestId('anim-pane').isVisible())) {
    await page.getByRole('button', { name: 'Ngăn hiệu ứng' }).first().click();
  }
  await expect(page.getByTestId('anim-pane')).toBeVisible();
}

async function openDeck(page: Page) {
  await page.getByRole('tab', { name: 'Trang đầu' }).click();
  const chooser = page.waitForEvent('filechooser');
  await page.getByRole('button', { name: 'Mở', exact: true }).click();
  await (await chooser).setFiles(deck);
  await expect(page.getByTestId('slide-status')).toHaveText(/^Slide 1 \/ \d+$/);
  await expect(page.getByTestId('slide-overlay')).toBeVisible();
}

/** Pieces of the show's layers shown (`data-visible`), in order. */
async function piecesShown(page: Page): Promise<boolean[]> {
  return page
    .getByTestId('show-part')
    .evaluateAll((els) => els.map((e) => e.getAttribute('data-visible') === 'true'));
}

test('plays paragraphs one by one, dims them, then a chart by series', async ({ page }) => {
  const errors: string[] = [];
  page.on('pageerror', (e) => errors.push(e.message));
  await page.goto('/slide');
  await openDeck(page);
  await page.getByTestId('thumb-2').click();
  await expect(page.getByTestId('slide-status')).toHaveText(/^Slide 2 \//);
  await showPane(page);
  // By 2nd-level paragraphs, in reverse, dimmed after: five effects, one build.
  await expect(page.getByTestId('anim-build-0')).toHaveValue('p2');
  await expect(page.getByTestId('anim-build-reverse-0')).toBeChecked();
  await expect(page.getByTestId('anim-build-1')).toHaveCount(0);
  await expect(page.getByTestId('anim-part-0')).toContainText('đoạn 6–8');
  await expect(page.getByTestId('anim-after-0')).toHaveValue('dim');
  await expect(page.getByTestId('anim-dim-color-0')).toHaveValue('#a6a6a6');

  await page.keyboard.press('Shift+F5');
  const stage = page.getByTestId('show-stage');
  await expect(stage).toHaveAttribute('data-slide', '1');
  await expect(stage).toHaveAttribute('data-clicks', '5');
  // The placeholder without its text, and five groups of paragraphs, none shown yet.
  await expect(page.getByTestId('show-part-body')).toHaveCount(1);
  await expect(page.getByTestId('show-part')).toHaveCount(5);
  await expect(page.getByTestId('show-part-body')).toHaveAttribute('data-visible', 'true');
  expect(await piecesShown(page)).toEqual([false, false, false, false, false]);
  // One more group on each click; the groups come last to first.
  const shown: number[] = [];
  for (const click of ['1', '2', '3']) {
    await page.keyboard.press('ArrowRight');
    await expect(stage).toHaveAttribute('data-click', click);
    await expect
      .poll(async () => (await piecesShown(page)).filter(Boolean).length)
      .toBe(Number(click));
    // The piece this click brought in.
    const now = await piecesShown(page);
    shown.push(now.findIndex((v, k) => v && !shown.includes(k)));
  }
  expect(shown).toEqual([4, 3, 2]);
  // Done paragraphs are dimmed: their grey drawing shows.
  await expect
    .poll(() =>
      page
        .getByTestId('show-tint')
        .evaluateAll((els) => els.filter((e) => e.getAttribute('data-shown') === 'true').length),
    )
    .toBeGreaterThanOrEqual(2);
  for (const click of ['4', '5']) {
    await page.keyboard.press('ArrowRight');
    await expect(stage).toHaveAttribute('data-click', click);
  }
  await expect.poll(async () => (await piecesShown(page)).every(Boolean)).toBe(true);

  // The chart: its background on the first click, then each series.
  await page.keyboard.press('ArrowRight');
  await expect(stage).toHaveAttribute('data-slide', '2');
  await expect(stage).toHaveAttribute('data-clicks', '3');
  await expect(page.getByTestId('show-part')).toHaveCount(2);
  await expect(page.getByTestId('show-part-body')).toHaveAttribute('data-visible', 'false');
  await page.keyboard.press('ArrowRight');
  await expect(page.getByTestId('show-part-body')).toHaveAttribute('data-visible', 'true');
  expect(await piecesShown(page)).toEqual([false, false]);
  await page.keyboard.press('ArrowRight');
  await expect.poll(() => piecesShown(page)).toEqual([true, false]);
  await page.keyboard.press('ArrowRight');
  await expect.poll(() => piecesShown(page)).toEqual([true, true]);

  // SmartArt one node by one, in reverse: three pieces.
  await page.keyboard.press('ArrowRight');
  await expect(stage).toHaveAttribute('data-slide', '3');
  await expect(page.getByTestId('show-part')).toHaveCount(3);
  await page.keyboard.press('ArrowRight');
  await expect.poll(() => piecesShown(page)).toEqual([false, false, true]);
  await page.keyboard.press('Escape');
  await expect(page.getByTestId('slide-overlay')).toBeVisible();
  expect(errors).toEqual([]);
});

test('hides after the effect and on the next click; reorders effects by dragging', async ({
  page,
}) => {
  const errors: string[] = [];
  page.on('pageerror', (e) => errors.push(e.message));
  await page.goto('/slide');
  await openDeck(page);
  await page.getByTestId('thumb-5').click();
  await expect(page.getByTestId('slide-status')).toHaveText(/^Slide 5 \//);
  await showPane(page);
  await expect(page.getByTestId('anim-after-0')).toHaveValue('hide');
  await expect(page.getByTestId('anim-sound-0')).toHaveValue('chime');
  await expect(page.getByTestId('anim-after-1')).toHaveValue('hideOnClick');
  await expect(page.getByTestId('anim-rewind-1')).toBeChecked();

  // In the show: the rounded rectangle flies in and is hidden once done; the ellipse spins and
  // is hidden at the next click.
  await page.keyboard.press('Shift+F5');
  const stage = page.getByTestId('show-stage');
  await expect(stage).toHaveAttribute('data-slide', '4');
  await expect(stage).toHaveAttribute('data-clicks', '3');
  const layer = (n: number) => page.getByTestId('show-layer').nth(n);
  await expect(page.getByTestId('show-layer')).toHaveCount(3);
  await expect(layer(1)).toHaveAttribute('data-visible', 'false');
  await page.keyboard.press('ArrowRight');
  await expect(stage).toHaveAttribute('data-click', '1');
  await expect(layer(1)).toHaveAttribute('data-visible', 'false', { timeout: 10_000 });
  await expect(layer(2)).toHaveAttribute('data-visible', 'true');
  await page.keyboard.press('ArrowRight');
  await expect(stage).toHaveAttribute('data-click', '2');
  await expect(layer(2)).toHaveAttribute('data-visible', 'true');
  await page.keyboard.press('ArrowRight');
  await expect(stage).toHaveAttribute('data-click', '3');
  await expect(layer(2)).toHaveAttribute('data-visible', 'false');
  await page.keyboard.press('Escape');
  await expect(page.getByTestId('slide-overlay')).toBeVisible();

  // Drag the first effect onto the second: their order swaps.
  await page.getByTestId('anim-item-0').dragTo(page.getByTestId('anim-item-1'));
  await expect(page.getByTestId('anim-after-0')).toHaveValue('hideOnClick');
  await expect(page.getByTestId('anim-after-1')).toHaveValue('hide');
  await expect(page.getByTestId('anim-sound-1')).toHaveValue('chime');
  expect(errors).toEqual([]);
});

test('morphs a rectangle into an ellipse point by point', async ({ page }) => {
  const errors: string[] = [];
  page.on('pageerror', (e) => errors.push(e.message));
  await page.goto('/slide');
  await openDeck(page);
  await page.getByTestId('thumb-6').click();
  await expect(page.getByTestId('slide-status')).toHaveText(/^Slide 6 \//);
  await page.keyboard.press('Shift+F5');
  const stage = page.getByTestId('show-stage');
  await expect(stage).toHaveAttribute('data-slide', '5');
  await page.keyboard.press('ArrowRight');
  await expect(stage).toHaveAttribute('data-slide', '6');
  await expect(stage).toHaveAttribute('data-transition', 'morph');
  // The outline is drawn as a path of its own that changes during the 2.5 s transition.
  const outline = page.getByTestId('morph-outline');
  await expect(outline).toHaveCount(1);
  const d0 = await outline.locator('path').first().getAttribute('d');
  await expect
    .poll(async () => Number(await outline.getAttribute('data-progress')))
    .toBeGreaterThan(20);
  const d1 = await outline.locator('path').first().getAttribute('d');
  expect(d1).not.toBe(d0);
  // 96 points in the ring on both sides of the morph.
  expect((d1 ?? '').split(' L ').length).toBe(96);
  await page.keyboard.press('Escape');
  expect(errors).toEqual([]);
});
