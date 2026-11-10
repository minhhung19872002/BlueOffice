import { expect, test, type Page } from '@playwright/test';
import { readFileSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

/**
 * SLIDE-ZOOM-001: insert a summary zoom ("Thu phóng tổng quan"), present it, click a zoom,
 * come back to the summary slide after the section; options and saving.
 */

const here = dirname(fileURLToPath(import.meta.url));
const deck = join(here, '..', '..', 'corpus', 'slide', 'slide-text-bullets.pptx');

async function openDeck(page: Page, path: string) {
  await page.getByRole('tab', { name: 'Trang đầu' }).click();
  const chooser = page.waitForEvent('filechooser');
  await page.getByRole('button', { name: 'Mở', exact: true }).click();
  await (await chooser).setFiles(path);
  await expect(page.getByTestId('slide-status')).toHaveText(/^Slide 1 \/ \d+$/);
  await expect(page.getByTestId('slide-overlay')).toBeVisible();
}

test('summary zoom: insert, present, click a zoom, come back', async ({ page }) => {
  const errors: string[] = [];
  page.on('pageerror', (e) => errors.push(e.message));
  await page.goto('/slide');
  await openDeck(page, deck);
  await expect(page.getByTestId('slide-status')).toHaveText('Slide 1 / 3');

  // Insert › Summary zoom of slides 2 and 3.
  await page.getByRole('tab', { name: 'Chèn' }).click();
  await page.getByRole('button', { name: 'Thu phóng tổng quan', exact: true }).click();
  const dialog = page.getByTestId('zoom-dialog');
  await expect(dialog).toBeVisible();
  await expect(page.getByTestId('zoom-insert')).toBeDisabled();
  await page.getByTestId('zoom-pick-1').check();
  await page.getByTestId('zoom-pick-2').check();
  await page.getByTestId('zoom-insert').click();
  await expect(dialog).toHaveCount(0);

  // A summary slide before slide 2, the zoom selected with its tab.
  await expect(page.getByTestId('slide-status')).toHaveText('Slide 2 / 4');
  await expect(page.getByTestId('selection-box')).toHaveCount(1);
  const zoomTab = page.getByRole('tab', { name: 'Thu phóng', exact: true });
  await expect(zoomTab).toBeVisible();
  await zoomTab.click();
  const ret = page.locator('[data-command="zoom.return"]').first();
  await expect(ret).toHaveAttribute('aria-pressed', 'true');

  // Present from the summary slide; click the first zoom: its section (slide 3) plays.
  await page.keyboard.press('Shift+F5');
  const stage = page.getByTestId('show-stage');
  await expect(stage).toHaveAttribute('data-slide', '1');
  const zooms = page.locator('[data-testid^="show-link-"][data-action="zoom"]');
  await expect(zooms).toHaveCount(2);
  await zooms.first().click();
  await expect(stage).toHaveAttribute('data-slide', '2');
  // After the section: back to the summary slide (zooming out), then the next zoom.
  await page.keyboard.press('ArrowRight');
  await expect(stage).toHaveAttribute('data-slide', '1');
  await zooms.nth(1).click();
  await expect(stage).toHaveAttribute('data-slide', '3');
  await page.keyboard.press('ArrowRight');
  await expect(stage).toHaveAttribute('data-slide', '1');
  await page.keyboard.press('Escape');
  await expect(page.getByTestId('slide-overlay')).toBeVisible();

  // Without "return", the show goes on from the target: 3, then 4, then the end.
  await expect(page.getByTestId('slide-status')).toHaveText(/^Slide \d \/ 4$/);
  await page.getByTestId('thumb-2').click();
  await expect(page.getByTestId('slide-status')).toHaveText('Slide 2 / 4');
  const o = (await page.getByTestId('slide-overlay').boundingBox())!;
  await page.mouse.click(o.x + o.width * 0.3, o.y + o.height * 0.6);
  await expect(zoomTab).toBeVisible();
  await zoomTab.click();
  await ret.click();
  await expect(ret).toHaveAttribute('aria-pressed', 'false');

  // Saved and reopened: the zooms and their option are kept.
  const download = page.waitForEvent('download');
  await page.keyboard.press('Control+s');
  const saved = readFileSync(await (await download).path());
  const copy = join(tmpdir(), `bo-e2e-${Date.now()}-zoom.pptx`);
  writeFileSync(copy, saved);
  await openDeck(page, copy);
  await expect(page.getByTestId('slide-status')).toHaveText('Slide 1 / 4');
  await page.getByTestId('thumb-2').click();
  await expect(page.getByTestId('slide-status')).toHaveText('Slide 2 / 4');
  await page.keyboard.press('Shift+F5');
  await expect(stage).toHaveAttribute('data-slide', '1');
  await zooms.first().click();
  await expect(stage).toHaveAttribute('data-slide', '2');
  await page.keyboard.press('ArrowRight');
  await expect(stage).toHaveAttribute('data-slide', '3');
  await page.keyboard.press('Escape');
  expect(errors).toEqual([]);
});
