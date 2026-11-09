import { expect, test, type Page } from '@playwright/test';
import { readFileSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { inflateRawSync } from 'node:zlib';

/**
 * SLIDE-SHOW-001: custom shows, show setup, show hyperlinks, presenter tools
 * (pen, highlighter, laser, zoom, black / white screen, slide grid), rehearsed
 * timings and kept ink.
 */

const here = dirname(fileURLToPath(import.meta.url));
const bullets = join(here, '..', '..', 'corpus', 'slide', 'slide-text-bullets.pptx');

async function openDeck(page: Page, path: string) {
  await page.getByRole('tab', { name: 'Trang đầu' }).click();
  const chooser = page.waitForEvent('filechooser');
  await page.getByRole('button', { name: 'Mở', exact: true }).click();
  await (await chooser).setFiles(path);
  await expect(page.getByTestId('slide-status')).toHaveText(/^Slide 1 \/ \d+$/);
  await expect(page.getByTestId('slide-overlay')).toBeVisible();
}

/** Save with Ctrl+S; returns the saved bytes and opens the copy again. */
async function saveAndReopen(page: Page, name: string): Promise<Buffer> {
  const download = page.waitForEvent('download');
  await page.keyboard.press('Control+s');
  const bytes = readFileSync(await (await download).path());
  const copy = join(tmpdir(), `bo-e2e-${Date.now()}-${name}`);
  writeFileSync(copy, bytes);
  await openDeck(page, copy);
  await expect(page.getByTestId('status-modified')).toHaveCount(0);
  return bytes;
}

/** One entry of a zip package (stored or deflated), from the central directory. */
function zipEntry(zip: Buffer, name: string): string | null {
  let eocd = zip.length - 22;
  while (eocd >= 0 && zip.readUInt32LE(eocd) !== 0x06054b50) eocd--;
  if (eocd < 0) return null;
  const entries = zip.readUInt16LE(eocd + 10);
  let p = zip.readUInt32LE(eocd + 16);
  for (let i = 0; i < entries; i++) {
    const method = zip.readUInt16LE(p + 10);
    const size = zip.readUInt32LE(p + 20);
    const nameLen = zip.readUInt16LE(p + 28);
    const extraLen = zip.readUInt16LE(p + 30);
    const commentLen = zip.readUInt16LE(p + 32);
    const local = zip.readUInt32LE(p + 42);
    const entry = zip.toString('utf8', p + 46, p + 46 + nameLen);
    if (entry === name) {
      const start = local + 30 + zip.readUInt16LE(local + 26) + zip.readUInt16LE(local + 28);
      const data = zip.subarray(start, start + size);
      return (method === 8 ? inflateRawSync(data) : data).toString('utf8');
    }
    p += 46 + nameLen + extraLen + commentLen;
  }
  return null;
}

async function slideShowTab(page: Page) {
  await page.getByRole('tab', { name: 'Trình chiếu' }).click();
}

async function command(page: Page, id: string) {
  await page.locator(`[data-command="${id}"]`).first().click();
}

async function clickSlide(page: Page, fx: number, fy: number) {
  const o = (await page.getByTestId('slide-overlay').boundingBox())!;
  await page.mouse.click(o.x + o.width * fx, o.y + o.height * fy);
}

test('custom shows play in their order, links start them and come back, setup round-trips', async ({
  page,
}) => {
  const errors: string[] = [];
  page.on('pageerror', (e) => errors.push(e.message));
  await page.goto('/slide');
  await openDeck(page, bullets);

  // A custom show of slides 3 and 2.
  await slideShowTab(page);
  await command(page, 'show.customShows');
  const dialog = page.getByTestId('custom-shows-dialog');
  await expect(dialog).toBeVisible();
  await page.getByTestId('cs-new').click();
  await page.getByTestId('cs-name').fill('Rút gọn');
  await page.getByTestId('cs-add-2').click();
  await page.getByTestId('cs-add-0').click();
  await page.getByTestId('cs-add-1').click();
  // Drop slide 1 again, keep 3 → 2.
  await page.getByTestId('cs-remove-1').click();
  await expect(page.getByTestId('cs-item-0')).toHaveAttribute('data-slide', '2');
  await expect(page.getByTestId('cs-item-1')).toHaveAttribute('data-slide', '1');
  await page.getByTestId('cs-save').click();
  await expect(page.getByTestId('cs-show-0')).toContainText('Rút gọn');
  await expect(page.getByTestId('status-modified')).toBeVisible();

  // Play it: slide 3, slide 2, the end.
  await page.getByTestId('cs-play-0').click();
  const stage = page.getByTestId('show-stage');
  await expect(stage).toHaveAttribute('data-slide', '2');
  await page.keyboard.press('ArrowRight');
  await expect(stage).toHaveAttribute('data-slide', '1');
  await page.keyboard.press('ArrowRight');
  await expect(stage).toHaveCount(0);
  await page.keyboard.press('Escape');
  await expect(page.getByTestId('slide-overlay')).toBeVisible();

  // The title of slide 1 links to the custom show (coming back afterwards).
  await page.getByTestId('thumb-1').click();
  await clickSlide(page, 0.48, 0.3);
  await expect(page.getByTestId('selection-box')).toHaveCount(1);
  await slideShowTab(page);
  await command(page, 'show.linkCustom');
  await expect(page.getByTestId('cs-link-return')).toBeChecked();
  await page.getByTestId('cs-link-0').click();
  await page.getByTestId('cs-close').click();
  await expect(dialog).toHaveCount(0);

  await saveAndReopen(page, 'custom-show.pptx');

  // In the show: the link plays 3 → 2, then back to slide 1, then slide 2.
  await page.keyboard.press('F5');
  await expect(stage).toHaveAttribute('data-slide', '0');
  const link = page.locator('[data-testid^="show-link-"]');
  await expect(link).toHaveCount(1);
  await expect(link).toHaveAttribute('data-action', 'customShow');
  await link.click();
  await expect(stage).toHaveAttribute('data-slide', '2');
  await page.keyboard.press('ArrowRight');
  await expect(stage).toHaveAttribute('data-slide', '1');
  await page.keyboard.press('ArrowRight');
  await expect(stage).toHaveAttribute('data-slide', '0');
  await page.keyboard.press('ArrowRight');
  await expect(stage).toHaveAttribute('data-slide', '1');
  await page.keyboard.press('Escape');
  await expect(page.getByTestId('slide-overlay')).toBeVisible();

  // Setup: kiosk restarting after 90 s, no animation, slides 2–3.
  await slideShowTab(page);
  await command(page, 'show.setup');
  await expect(page.getByTestId('setup-show-dialog')).toBeVisible();
  await page.getByTestId('setup-type-kiosk').check();
  await page.getByTestId('setup-restart').fill('90');
  await expect(page.getByTestId('setup-loop')).toBeChecked();
  await expect(page.getByTestId('setup-loop')).toBeDisabled();
  await page.getByTestId('setup-no-animation').check();
  await page.getByTestId('setup-range-range').check();
  await page.getByTestId('setup-from').fill('2');
  await page.getByTestId('setup-to').fill('3');
  await page.getByTestId('setup-ok').click();
  await expect(page.getByTestId('setup-show-dialog')).toHaveCount(0);

  const bytes = await saveAndReopen(page, 'show-setup.pptx');
  const props = zipEntry(bytes, 'ppt/presProps.xml') ?? '';
  expect(props).toContain('<p:kiosk restart="90000"/>');
  expect(props).toContain('<p:sldRg st="2" end="3"/>');
  expect(props).toContain('showAnimation="0"');
  const pres = zipEntry(bytes, 'ppt/presentation.xml') ?? '';
  expect(pres).toContain('<p:custShow name="Rút gọn" id="0">');

  // The setup is back after reopening.
  await slideShowTab(page);
  await command(page, 'show.setup');
  await expect(page.getByTestId('setup-type-kiosk')).toBeChecked();
  await expect(page.getByTestId('setup-restart')).toHaveValue('90');
  await expect(page.getByTestId('setup-from')).toHaveValue('2');
  await expect(page.getByTestId('setup-to')).toHaveValue('3');
  await expect(page.getByTestId('setup-no-animation')).toBeChecked();
  await page.getByRole('button', { name: 'Huỷ' }).click();

  // Kiosk: starts at slide 2 of the range; keys do not move it, Esc ends it.
  await page.keyboard.press('F5');
  await expect(stage).toHaveAttribute('data-slide', '1');
  await page.keyboard.press('ArrowRight');
  await page.keyboard.press('Space');
  await expect(stage).toHaveAttribute('data-slide', '1');
  await page.keyboard.press('Escape');
  await expect(page.getByTestId('slide-overlay')).toBeVisible();
  expect(errors).toEqual([]);
});

test('presenter tools: pen, highlighter, laser, zoom, black / white screen, slide grid, kept ink', async ({
  page,
}) => {
  const errors: string[] = [];
  page.on('pageerror', (e) => errors.push(e.message));
  await page.goto('/slide');
  await openDeck(page, bullets);

  await page.keyboard.press('F5');
  const stage = page.getByTestId('show-stage');
  const surface = page.getByTestId('show-surface');
  await expect(stage).toHaveAttribute('data-slide', '0');
  await expect(page.getByTestId('show-toolbar')).toBeAttached();

  // Pen: one stroke dragged over the slide.
  await page.getByTestId('show-tool-pen').click();
  await expect(surface).toHaveAttribute('data-tool', 'pen');
  const box = (await surface.boundingBox())!;
  await page.mouse.move(box.x + box.width * 0.2, box.y + box.height * 0.6);
  await page.mouse.down();
  await page.mouse.move(box.x + box.width * 0.4, box.y + box.height * 0.7, { steps: 6 });
  await page.mouse.move(box.x + box.width * 0.6, box.y + box.height * 0.6, { steps: 6 });
  await page.mouse.up();
  await expect(page.getByTestId('show-ink')).toHaveAttribute('data-strokes', '1');
  // Highlighter (Ctrl+I): a second stroke.
  await page.keyboard.press('Control+i');
  await expect(surface).toHaveAttribute('data-tool', 'highlighter');
  await page.mouse.move(box.x + box.width * 0.2, box.y + box.height * 0.8);
  await page.mouse.down();
  await page.mouse.move(box.x + box.width * 0.7, box.y + box.height * 0.8, { steps: 8 });
  await page.mouse.up();
  await expect(page.getByTestId('show-ink')).toHaveAttribute('data-strokes', '2');
  // Eraser over the highlighter stroke removes it.
  await page.getByTestId('show-tool-eraser').click();
  await page.mouse.move(box.x + box.width * 0.45, box.y + box.height * 0.8);
  await page.mouse.down();
  await page.mouse.up();
  await expect(page.getByTestId('show-ink')).toHaveAttribute('data-strokes', '1');

  // Laser dot follows the pointer.
  await page.getByTestId('show-tool-laser').click();
  await page.mouse.move(box.x + box.width * 0.5, box.y + box.height * 0.5);
  await page.mouse.move(box.x + box.width * 0.52, box.y + box.height * 0.5);
  await expect(page.getByTestId('show-laser')).toBeVisible();
  // Esc puts the pointer back (the show goes on).
  await page.keyboard.press('Escape');
  await expect(surface).toHaveAttribute('data-tool', 'pointer');
  await expect(stage).toHaveAttribute('data-slide', '0');

  // White and black screen.
  await page.keyboard.press('w');
  await expect(surface).toHaveAttribute('data-cover', 'white');
  await expect(page.getByTestId('show-cover')).toBeVisible();
  await page.keyboard.press('w');
  await expect(surface).toHaveAttribute('data-cover', 'none');
  await page.keyboard.press('b');
  await expect(surface).toHaveAttribute('data-cover', 'black');
  await page.keyboard.press('w');
  await expect(surface).toHaveAttribute('data-cover', 'white');
  await page.keyboard.press('w');
  await expect(surface).toHaveAttribute('data-cover', 'none');

  // Zoom: drag a quarter of the slide → ×2 or more; Esc zooms back out.
  await page.getByTestId('show-tool-zoom').click();
  await page.mouse.move(box.x + box.width * 0.1, box.y + box.height * 0.1);
  await page.mouse.down();
  await page.mouse.move(box.x + box.width * 0.35, box.y + box.height * 0.35, { steps: 5 });
  await expect(page.getByTestId('show-zoom-band')).toBeVisible();
  await page.mouse.up();
  await expect(surface).not.toHaveAttribute('data-zoom', '1.00');
  const z = Number(await surface.getAttribute('data-zoom'));
  expect(z).toBeGreaterThanOrEqual(2);
  await page.keyboard.press('Escape');
  await expect(surface).toHaveAttribute('data-zoom', '1.00');
  await expect(stage).toHaveAttribute('data-slide', '0');

  // Slide grid: jump to slide 3.
  await page.keyboard.press('g');
  await expect(page.getByTestId('show-grid')).toBeVisible();
  await expect(page.getByTestId('show-grid-item-0')).toHaveAttribute('aria-current', 'true');
  await page.getByTestId('show-grid-item-2').click();
  await expect(page.getByTestId('show-grid')).toHaveCount(0);
  await expect(stage).toHaveAttribute('data-slide', '2');
  // Ink belongs to its slide: none here.
  await expect(page.getByTestId('show-ink')).toHaveAttribute('data-strokes', '0');

  // End: keep the ink as drawings on slide 1.
  await page.keyboard.press('Escape');
  const end = page.getByTestId('end-show-dialog');
  await expect(end).toBeVisible();
  await expect(page.getByTestId('end-keep-ink')).toBeChecked();
  await page.getByTestId('end-show-ok').click();
  await expect(end).toHaveCount(0);
  await expect(page.getByTestId('status-modified')).toBeVisible();

  const bytes = await saveAndReopen(page, 'kept-ink.pptx');
  const slide1 = zipEntry(bytes, 'ppt/slides/slide1.xml') ?? '';
  expect(slide1).toContain('<a:custGeom>');
  expect(slide1).toContain('name="Nét mực');
  expect(errors).toEqual([]);
});

test('rehearsed timings are saved per slide and played', async ({ page }) => {
  const errors: string[] = [];
  page.on('pageerror', (e) => errors.push(e.message));
  await page.goto('/slide');
  await openDeck(page, bullets);

  await slideShowTab(page);
  await command(page, 'show.rehearse');
  const bar = page.getByTestId('rehearsal-bar');
  await expect(bar).toBeVisible();
  const stage = page.getByTestId('show-stage');
  await expect(stage).toHaveAttribute('data-slide', '0');
  // At least a second on each slide.
  await expect(page.getByTestId('rehearsal-slide-time')).toHaveText(/^00:0[1-9]$/, {
    timeout: 5000,
  });
  await page.getByTestId('rehearsal-next').click();
  await expect(stage).toHaveAttribute('data-slide', '1');
  await expect(page.getByTestId('rehearsal-slide-time')).toHaveText(/^00:0[1-9]$/, {
    timeout: 5000,
  });
  await page.getByTestId('rehearsal-pause').click();
  await expect(page.getByTestId('rehearsal-pause')).toHaveAttribute('aria-pressed', 'true');
  await page.getByTestId('rehearsal-pause').click();
  await page.getByTestId('rehearsal-next').click();
  await expect(stage).toHaveAttribute('data-slide', '2');
  await expect(page.getByTestId('rehearsal-slide-time')).toHaveText(/^00:0[1-9]$/, {
    timeout: 5000,
  });
  await page.keyboard.press('Escape');

  const end = page.getByTestId('end-show-dialog');
  await expect(end).toBeVisible();
  await expect(page.getByTestId('end-keep-timings')).toBeChecked();
  await page.getByTestId('end-show-ok').click();
  await expect(end).toHaveCount(0);
  await expect(page.getByTestId('status-modified')).toBeVisible();

  const bytes = await saveAndReopen(page, 'timings.pptx');
  for (const n of [1, 2, 3]) {
    const xml = zipEntry(bytes, `ppt/slides/slide${n}.xml`) ?? '';
    const m = /advTm="(\d+)"/.exec(xml);
    expect(m, `slide ${n}`).not.toBeNull();
    expect(Number(m![1])).toBeGreaterThanOrEqual(1000);
  }
  // "Use saved timings" is on (the schema default, no attribute written).
  await slideShowTab(page);
  await expect(page.locator('[data-command="show.useTimings"]').first()).toHaveAttribute(
    'aria-pressed',
    'true',
  );
  expect(errors).toEqual([]);
});
