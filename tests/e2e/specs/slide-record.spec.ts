import { expect, test, type Page } from '@playwright/test';
import { readFileSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { inflateRawSync } from 'node:zlib';

/**
 * SLIDE-REC-001: recording a slide show — narration and camera from Chromium's fake devices
 * (MediaRecorder through `Platform.media`), ink drawn while recording, the time of each slide —
 * saved into the deck, played back, then cleared.
 */

test.use({
  launchOptions: {
    args: ['--use-fake-device-for-media-stream', '--use-fake-ui-for-media-stream'],
  },
  permissions: ['microphone', 'camera'],
});

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

/** Names and text of the entries of a zip package. */
function zipEntries(zip: Buffer): Map<string, Buffer> {
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
    const entry = zip.toString('utf8', p + 46, p + 46 + nameLen);
    const start = local + 30 + zip.readUInt16LE(local + 26) + zip.readUInt16LE(local + 28);
    const data = zip.subarray(start, start + size);
    out.set(entry, method === 8 ? inflateRawSync(data) : data);
    p += 46 + nameLen + extraLen + commentLen;
  }
  return out;
}

async function slideShowTab(page: Page) {
  await page.getByRole('tab', { name: 'Trình chiếu' }).click();
}

async function command(page: Page, id: string) {
  await page.locator(`[data-command="${id}"]`).first().click();
}

test('a recorded show keeps narration, camera, ink and timings, plays them and is cleared', async ({
  page,
}) => {
  const errors: string[] = [];
  page.on('pageerror', (e) => errors.push(e.message));
  await page.goto('/slide');
  await openDeck(page, bullets);

  // Record from the start with the camera in the top left corner.
  await slideShowTab(page);
  await command(page, 'show.recordFromStart');
  const setup = page.getByTestId('record-setup-dialog');
  await expect(setup).toBeVisible();
  await expect(page.getByTestId('record-narration')).toBeChecked();
  await page.getByTestId('record-camera').check();
  await page.getByTestId('record-corner').selectOption('topLeft');
  await page.getByTestId('record-start').click();

  const bar = page.getByTestId('recording-bar');
  await expect(bar).toBeVisible();
  await expect(bar).toHaveAttribute('data-state', 'recording');
  await expect(page.getByTestId('recording-notice')).toHaveCount(0);
  await expect(page.getByTestId('record-camera-preview')).toBeVisible();
  const stage = page.getByTestId('show-stage');
  await expect(stage).toHaveAttribute('data-slide', '0');

  // Ink on slide 1 while recording.
  await page.waitForTimeout(600);
  await page.getByTestId('show-tool-pen').click();
  const surface = page.getByTestId('show-surface');
  const box = (await surface.boundingBox())!;
  await page.mouse.move(box.x + box.width * 0.3, box.y + box.height * 0.6);
  await page.mouse.down();
  await page.mouse.move(box.x + box.width * 0.5, box.y + box.height * 0.7, { steps: 8 });
  await page.mouse.up();
  await expect(page.getByTestId('show-ink')).toHaveAttribute('data-strokes', '1');
  await page.waitForTimeout(700);
  await page.keyboard.press('ArrowRight');
  await expect(stage).toHaveAttribute('data-slide', '1');

  // Pause and resume on slide 2.
  await page.waitForTimeout(600);
  await page.getByTestId('recording-pause').click();
  await expect(bar).toHaveAttribute('data-state', 'paused');
  await page.getByTestId('recording-pause').click();
  await expect(bar).toHaveAttribute('data-state', 'recording');
  await page.waitForTimeout(600);
  await page.getByTestId('recording-stop').click();

  // Keep the recording.
  const end = page.getByTestId('end-show-dialog');
  await expect(end).toBeVisible({ timeout: 15_000 });
  await expect(page.getByTestId('end-keep-recording')).toBeChecked();
  await page.getByTestId('end-show-ok').click();
  await expect(end).toHaveCount(0);
  await expect(page.getByTestId('status-modified')).toBeVisible();

  const bytes = await saveAndReopen(page, 'recorded.pptx');
  const zip = zipEntries(bytes);
  const s1 = zip.get('ppt/slides/slide1.xml')!.toString('utf8');
  const s2 = zip.get('ppt/slides/slide2.xml')!.toString('utf8');
  for (const xml of [s1, s2]) {
    expect(xml).toContain('isNarration="1"');
    expect(xml).toContain('showWhenStopped="0"');
    expect(xml).toContain('<p:video');
    expect(Number(/advTm="(\d+)"/.exec(xml)?.[1] ?? 0)).toBeGreaterThanOrEqual(1000);
  }
  // The recorded ink is an InkML part with times.
  const inks = [...zip.keys()].filter((n) => n.startsWith('ppt/ink/'));
  expect(inks).toHaveLength(1);
  expect(zip.get(inks[0]!)!.toString('utf8')).toContain('name="T"');
  expect(s1).toContain('presetClass="entr"');
  // Narration and camera are media parts recorded by the browser.
  const media = [...zip.keys()].filter((n) => /^ppt\/media\/media\d+\.(webm|oga|m4a|mp4)$/.test(n));
  expect(media.length).toBeGreaterThanOrEqual(4);
  expect((zip.get('ppt/presProps.xml') ?? Buffer.from('')).toString('utf8')).toContain(
    'showNarration="1"',
  );
  await slideShowTab(page);
  await expect(page.locator('[data-command="show.playNarration"]').first()).toHaveAttribute(
    'aria-pressed',
    'true',
  );

  // Played back: the narration and the camera clip of the slide load with it.
  await page.keyboard.press('F5');
  await expect(stage).toHaveAttribute('data-slide', '0');
  await expect(page.getByTestId('media-audio')).toHaveCount(1);
  await expect(page.getByTestId('media-video')).toHaveCount(1);
  await expect(page.getByTestId('media-audio')).toHaveAttribute('data-ready', 'true', {
    timeout: 15_000,
  });
  await page.keyboard.press('Escape');
  await expect(page.getByTestId('slide-overlay')).toBeVisible();

  // Clear the recordings and the timings of every slide.
  await slideShowTab(page);
  await command(page, 'show.clearRecordingAll');
  await command(page, 'show.clearTimingAll');
  const cleared = zipEntries(await saveAndReopen(page, 'cleared.pptx'));
  const c1 = cleared.get('ppt/slides/slide1.xml')!.toString('utf8');
  expect(c1).not.toContain('isNarration');
  expect(c1).not.toContain('advTm');
  expect([...cleared.keys()].filter((n) => n.startsWith('ppt/ink/'))).toHaveLength(0);
  expect(errors).toEqual([]);
});

const narrated = join(here, '..', '..', 'corpus', 'slide', 'slide-rec-narration.pptx');

test('the video export carries the recorded narration through the browser encoders', async ({
  page,
}) => {
  test.setTimeout(180_000);
  const errors: string[] = [];
  page.on('pageerror', (e) => errors.push(e.message));
  await page.goto('/slide');
  await openDeck(page, narrated);
  await page.getByRole('tab', { name: 'Trang đầu' }).click();
  await command(page, 'file.exportVideo');
  await expect(page.getByTestId('export-video-dialog')).toBeVisible();
  await expect(page.getByTestId('export-video-checking')).toHaveCount(0, { timeout: 15_000 });
  const video = page.getByTestId('export-video-format-video');
  test.skip(!(await video.isEnabled()), 'no WebCodecs video encoder in this browser');
  await video.check();
  await page.getByTestId('export-video-width').selectOption('480');
  await page.getByTestId('export-video-fps').selectOption('24');
  await expect(page.getByTestId('export-video-narration')).toBeChecked();
  const download = page.waitForEvent('download', { timeout: 150_000 });
  await page.getByTestId('export-video-ok').click();
  const bytes = readFileSync(await (await download).path());
  const text = bytes.toString('latin1');
  const status = page.getByTestId('slide-status');
  test.info().annotations.push({
    type: 'container',
    description: text.includes('ftyp') ? (text.includes('soun') ? 'mp4+aac' : 'mp4') : 'webm+opus',
  });
  if (text.includes('ftyp')) {
    // MP4: an AAC sound track, unless the browser has no AAC encoder (documented fallback).
    if (text.includes('soun')) {
      expect(text).toContain('mp4a');
      await expect(status).toHaveText('Đã xuất video');
    } else {
      await expect(status).toContainText('không mã hoá được âm thanh');
    }
  } else {
    // WebM: VP8 / VP9 video and an Opus track.
    expect(text).toContain('A_OPUS');
    expect(text).toContain('OpusHead');
    await expect(status).toHaveText('Đã xuất video');
  }
  expect(errors).toEqual([]);
});
