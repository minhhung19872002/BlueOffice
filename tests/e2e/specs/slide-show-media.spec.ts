import { expect, test, type Download, type Page } from '@playwright/test';
import { mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const here = dirname(fileURLToPath(import.meta.url));
const corpus = join(here, '..', '..', 'corpus', 'slide');
const bullets = join(corpus, 'slide-text-bullets.pptx');
const video = join(corpus, 'slide-media-video.pptx');
const outDir = join(here, '..', '..', 'fidelity', 'output');

async function openDeck(page: Page, path: string) {
  // "Open" is on the Home tab.
  await page.getByRole('tab', { name: 'Trang đầu' }).click();
  const chooser = page.waitForEvent('filechooser');
  await page.getByRole('button', { name: 'Mở', exact: true }).click();
  await (await chooser).setFiles(path);
  await expect(page.getByTestId('slide-status')).toHaveText(/^Slide 1 \/ \d+$/);
  await expect(page.getByTestId('slide-overlay')).toBeVisible();
}

/** Save with Ctrl+S and open the downloaded deck again. */
async function saveAndReopen(page: Page, name: string): Promise<string> {
  const download = page.waitForEvent('download');
  await page.keyboard.press('Control+s');
  const saved = await (await download).path();
  const copy = join(tmpdir(), `bo-e2e-${Date.now()}-${name}`);
  writeFileSync(copy, readFileSync(saved));
  await openDeck(page, copy);
  await expect(page.getByTestId('status-modified')).toHaveCount(0);
  return copy;
}

async function clickSlide(page: Page, fx: number, fy: number) {
  const o = (await page.getByTestId('slide-overlay').boundingBox())!;
  await page.mouse.click(o.x + o.width * fx, o.y + o.height * fy);
}

async function bytesOf(d: Download): Promise<Buffer> {
  return readFileSync(await d.path());
}

/** A short mono 8 kHz PCM WAV tone (RIFF / WAVE, every browser decodes it). */
function wav(): Buffer {
  const rate = 8000;
  const n = rate / 2;
  const b = Buffer.alloc(44 + n * 2);
  b.write('RIFF', 0);
  b.writeUInt32LE(36 + n * 2, 4);
  b.write('WAVE', 8);
  b.write('fmt ', 12);
  b.writeUInt32LE(16, 16);
  b.writeUInt16LE(1, 20);
  b.writeUInt16LE(1, 22);
  b.writeUInt32LE(rate, 24);
  b.writeUInt32LE(rate * 2, 28);
  b.writeUInt16LE(2, 32);
  b.writeUInt16LE(16, 34);
  b.write('data', 36);
  b.writeUInt32LE(n * 2, 40);
  for (let i = 0; i < n; i++) {
    b.writeInt16LE(Math.round(Math.sin((i / rate) * 2 * Math.PI * 440) * 8000), 44 + i * 2);
  }
  return b;
}

test('sets a transition and an effect, plays them in the show, saves and reopens', async ({
  page,
}) => {
  const errors: string[] = [];
  page.on('pageerror', (e) => errors.push(e.message));
  await page.goto('/slide');
  await openDeck(page, bullets);

  // Transition of slide 2: push up.
  await page.getByTestId('thumb-2').click();
  await expect(page.getByTestId('slide-status')).toHaveText(/^Slide 2 \//);
  await page.getByRole('tab', { name: 'Chuyển trang' }).click();
  await page.getByRole('button', { name: 'Đẩy', exact: true }).click();
  await expect(page.getByTestId('anim-pane')).toBeVisible();
  await expect(page.getByTestId('transition-kind')).toHaveValue('push');
  await page.getByTestId('transition-dir').selectOption('u');
  await expect(page.getByTestId('transition-dir')).toHaveValue('u');

  // Slide 1: the title flies in on click.
  await page.getByTestId('thumb-1').click();
  await clickSlide(page, 0.48, 0.3);
  await expect(page.getByTestId('selection-box')).toHaveCount(1);
  await page.getByRole('tab', { name: 'Hiệu ứng' }).click();
  await page.getByRole('button', { name: 'Bay vào', exact: true }).click();
  await expect(page.getByTestId('anim-item-0')).toBeVisible();
  await expect(page.getByTestId('anim-trigger-0')).toHaveValue('onClick');
  const dur = page.getByTestId('anim-duration-0');
  await dur.fill('0.8');
  await dur.blur();
  await expect(page.getByTestId('anim-duration-0')).toHaveValue('0.8');
  // Preview in the pane.
  await page.getByTestId('anim-preview').click();
  await expect(page.getByTestId('anim-pane').getByTestId('show-stage')).toBeVisible();

  await saveAndReopen(page, 'anim.pptx');
  await page.getByRole('tab', { name: 'Hiệu ứng' }).click();
  await expect(page.getByTestId('anim-item-0')).toBeVisible();
  await expect(page.getByTestId('anim-duration-0')).toHaveValue('0.8');
  await page.getByTestId('thumb-2').click();
  await expect(page.getByTestId('transition-kind')).toHaveValue('push');
  await expect(page.getByTestId('transition-dir')).toHaveValue('u');
  await page.getByTestId('thumb-1').click();

  // Slide show: the title is hidden until the click, then the push transition.
  await page.keyboard.press('F5');
  const stage = page.getByTestId('show-stage');
  await expect(stage).toHaveAttribute('data-slide', '0');
  await expect(stage).toHaveAttribute('data-clicks', '1');
  await expect(page.locator('[data-testid="show-layer"][data-visible="false"]')).toHaveCount(1);
  await page.keyboard.press('ArrowRight');
  await expect(stage).toHaveAttribute('data-click', '1');
  await expect(page.locator('[data-testid="show-layer"][data-visible="false"]')).toHaveCount(0);
  await page.keyboard.press('ArrowRight');
  await expect(stage).toHaveAttribute('data-slide', '1');
  await expect(stage).toHaveAttribute('data-transition', 'push');
  mkdirSync(outDir, { recursive: true });
  await page.screenshot({ path: join(outDir, 'web-slide-show-transition.png') });
  await page.keyboard.press('Escape');
  await expect(page.getByTestId('slide-overlay')).toBeVisible();
  expect(errors).toEqual([]);
});

test('inserts a chart, edits its data, saves and reopens it editable', async ({ page }) => {
  const errors: string[] = [];
  page.on('pageerror', (e) => errors.push(e.message));
  await page.goto('/slide');
  await openDeck(page, bullets);
  await page.getByRole('button', { name: '+ Thêm slide' }).click();
  await expect(page.getByTestId('slide-status')).toHaveText(/^Slide 2 \//);
  await page.getByRole('tab', { name: 'Chèn' }).click();
  await page.getByRole('button', { name: 'Biểu đồ', exact: true }).click();
  const dialog = page.getByTestId('chart-dialog');
  await expect(dialog).toBeVisible();
  await page.getByTestId('chart-title').fill('Doanh thu 2026');
  await page.getByTestId('chart-cell-0-0').fill('12,5');
  await page.getByTestId('chart-add-row').click();
  await page.getByTestId('chart-cat-4').fill('Quý 5');
  await page.getByTestId('chart-cell-4-1').fill('9');
  await page.getByTestId('chart-ok').click();
  await expect(dialog).toHaveCount(0);
  await expect(page.getByTestId('selection-box')).toHaveCount(1);
  // The chart tab changes the type of the selected chart.
  await page.getByRole('tab', { name: 'Biểu đồ' }).click();
  await page.getByRole('button', { name: 'Đường', exact: true }).click();
  await expect(page.getByTestId('status-modified')).toBeVisible();

  await saveAndReopen(page, 'chart.pptx');
  await page.getByTestId('thumb-2').click();
  await clickSlide(page, 0.5, 0.5);
  await expect(page.getByTestId('selection-box')).toHaveCount(1);
  await page.getByRole('tab', { name: 'Biểu đồ' }).click();
  await page.getByRole('button', { name: 'Sửa biểu đồ' }).click();
  await expect(page.getByTestId('chart-dialog')).toBeVisible();
  await expect(page.getByTestId('chart-kind')).toHaveValue('line');
  await expect(page.getByTestId('chart-title')).toHaveValue('Doanh thu 2026');
  await expect(page.getByTestId('chart-cell-0-0')).toHaveValue('12.5');
  await expect(page.getByTestId('chart-cat-4')).toHaveValue('Quý 5');
  await expect(page.getByTestId('chart-cell-4-1')).toHaveValue('9');
  await page.getByTestId('chart-legend').selectOption('none');
  await page.getByTestId('chart-ok').click();
  await expect(page.getByTestId('status-modified')).toBeVisible();
  expect(errors).toEqual([]);
});

test('plays embedded media, shows the poster otherwise, inserts audio and keeps it', async ({
  page,
}) => {
  const errors: string[] = [];
  page.on('pageerror', (e) => errors.push(e.message));
  await page.goto('/slide');
  // The corpus clip is not a real stream: the poster frame stays, marked as not playable.
  await openDeck(page, video);
  await page.getByTestId('media-play-4').click();
  await expect(page.getByTestId('media-video')).toHaveAttribute('data-playable', 'false');

  // Insert a real (PCM) audio clip: it plays in normal view and in the show.
  const file = join(tmpdir(), `bo-e2e-${Date.now()}-tieng.wav`);
  writeFileSync(file, wav());
  const chooser = page.waitForEvent('filechooser');
  await page.getByRole('tab', { name: 'Chèn' }).click();
  await page.getByRole('button', { name: 'Video / âm thanh' }).click();
  await (await chooser).setFiles(file);
  await expect(page.locator('[data-testid^="media-play-"]')).toHaveCount(2);
  await page.locator('[data-testid^="media-play-"]').last().click();
  await expect(page.getByTestId('media-audio')).toHaveAttribute('data-ready', 'true');
  await page.locator('[data-testid^="media-play-"]').last().click();
  await expect(page.getByTestId('media-audio')).toHaveCount(0);

  await page.keyboard.press('F5');
  const stage = page.getByTestId('show-stage');
  await expect(stage.getByTestId('media-audio')).toHaveAttribute('data-ready', 'true');
  await expect(stage.getByTestId('media-video')).toHaveAttribute('data-playable', 'false');
  await page.keyboard.press('Escape');

  await saveAndReopen(page, 'media.pptx');
  await expect(page.locator('[data-testid^="media-play-"]')).toHaveCount(2);
  expect(errors).toEqual([]);
});

test('exports PNG pictures, reorders slides in the sorter and reads in reading view', async ({
  page,
}) => {
  const errors: string[] = [];
  page.on('pageerror', (e) => errors.push(e.message));
  await page.goto('/slide');
  await openDeck(page, bullets);
  const total = Number(
    /\/ (\d+)$/.exec((await page.getByTestId('slide-status').textContent()) ?? '')?.[1],
  );
  expect(total).toBeGreaterThanOrEqual(3);

  // PNG of the current slide, then of every slide (one .zip).
  await page.getByRole('button', { name: 'Xuất ảnh PNG' }).click();
  await expect(page.getByTestId('export-png-dialog')).toBeVisible();
  let download = page.waitForEvent('download');
  await page.getByTestId('export-png-ok').click();
  let d = await download;
  expect(d.suggestedFilename()).toBe('slide-text-bullets-1.png');
  const png = await bytesOf(d);
  expect(png.subarray(0, 8)).toEqual(Buffer.from([137, 80, 78, 71, 13, 10, 26, 10]));
  // 2 px per point by default: 1920 px wide for a 960 pt slide.
  expect(png.readUInt32BE(16)).toBe(1920);
  await page.getByRole('button', { name: 'Xuất ảnh PNG' }).click();
  await page.getByTestId('export-png-all').check();
  await page.getByTestId('export-png-scale').selectOption('1');
  download = page.waitForEvent('download');
  await page.getByTestId('export-png-ok').click();
  d = await download;
  expect(d.suggestedFilename()).toBe('slide-text-bullets-png.zip');
  const zip = await bytesOf(d);
  expect(zip.readUInt32LE(0)).toBe(0x04034b50);
  expect(zip.readUInt16LE(zip.length - 22 + 10)).toBe(total);

  // Mark slide 1 by its notes, then drag it to the third place in the sorter.
  const notes = page.getByTestId('notes-editor');
  await notes.fill('Slide đầu tiên');
  await page.getByTestId('thumb-2').click();
  await page.getByRole('tab', { name: 'Xem' }).click();
  await page.getByRole('button', { name: 'Sắp xếp slide' }).click();
  await page.getByTestId('sorter-1').dragTo(page.getByTestId('sorter-3'));
  await expect(page.getByTestId('status-modified')).toBeVisible();
  await page.getByRole('button', { name: 'Thường' }).click();
  await page.getByTestId('thumb-3').click();
  await expect(page.getByTestId('notes-editor')).toHaveValue('Slide đầu tiên');
  await page.getByTestId('thumb-1').click();
  await expect(page.getByTestId('notes-editor')).not.toHaveValue('Slide đầu tiên');

  // Reading view: slides in the window, buttons and keys move through them.
  await page.getByRole('tab', { name: 'Xem' }).click();
  await page.getByRole('button', { name: 'Chế độ đọc' }).click();
  await expect(page.getByTestId('reading-view')).toBeVisible();
  await expect(page.getByTestId('reading-status')).toHaveText(`Slide 1 / ${total}`);
  await expect(page.getByTestId('show-stage')).toBeVisible();
  await page.getByTestId('reading-next').click();
  await expect(page.getByTestId('reading-status')).toHaveText(`Slide 2 / ${total}`);
  await page.keyboard.press('ArrowRight');
  await expect(page.getByTestId('reading-status')).toHaveText(`Slide 3 / ${total}`);
  await page.keyboard.press('ArrowLeft');
  await expect(page.getByTestId('reading-status')).toHaveText(`Slide 2 / ${total}`);
  await page.keyboard.press('Escape');
  await expect(page.getByTestId('slide-overlay')).toBeVisible();
  await expect(page.getByTestId('thumb-2')).toHaveAttribute('aria-current', 'true');
  expect(errors).toEqual([]);
});
