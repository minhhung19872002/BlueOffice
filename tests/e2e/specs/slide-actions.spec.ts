import { expect, test, type Page } from '@playwright/test';
import { readFileSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { inflateRawSync } from 'node:zlib';

/**
 * SLIDE-ACTION-001: hyperlinks and actions — a link to a slide, an action
 * button, a web link that asks first, and actions that are never run
 * (docs/deploy/security.md §10.1).
 */

const here = dirname(fileURLToPath(import.meta.url));
const corpus = join(here, '..', '..', 'corpus', 'slide');
const bullets = join(corpus, 'slide-text-bullets.pptx');
const unsafe = join(corpus, 'slide-action-unsafe.pptx');

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

async function insertTab(page: Page) {
  await page.getByRole('tab', { name: 'Chèn' }).click();
}

async function command(page: Page, id: string) {
  await page.locator(`[data-command="${id}"]`).first().click();
}

async function clickSlide(page: Page, fx: number, fy: number) {
  const o = (await page.getByTestId('slide-overlay').boundingBox())!;
  await page.mouse.click(o.x + o.width * fx, o.y + o.height * fy);
}

test('a slide link, an action button and a web link that asks first', async ({ page, context }) => {
  const errors: string[] = [];
  page.on('pageerror', (e) => errors.push(e.message));
  // The web link's target is answered here: nothing leaves the machine.
  await context.route('https://example.vn/**', (r) =>
    r.fulfill({ contentType: 'text/html', body: '<title>Đích</title>ok' }),
  );
  await page.goto('/slide');
  await openDeck(page, bullets);
  await expect(page.getByTestId('slide-status')).toHaveText('Slide 1 / 3');

  // Slide 1: the title links to slide 3 (Insert › Link › place in this deck).
  await clickSlide(page, 0.48, 0.3);
  await expect(page.getByTestId('selection-box')).toHaveCount(1);
  await insertTab(page);
  await command(page, 'insert.link');
  const linkDialog = page.getByTestId('link-dialog');
  await expect(linkDialog).toBeVisible();
  await page.getByTestId('link-kind-place').click();
  // The slide picker: a thumbnail and the title of every slide.
  await expect(page.getByTestId('link-slides').locator('button')).toHaveCount(3);
  await expect(page.getByTestId('link-slide-1').locator('canvas')).toHaveCount(1);
  await page.getByTestId('link-slide-3').click();
  await page.getByTestId('link-tooltip').fill('Tới trang 3');
  await page.getByTestId('link-ok').click();
  await expect(linkDialog).toHaveCount(0);
  await expect(page.getByTestId('status-modified')).toBeVisible();

  // Slide 2: an action button that goes to the last slide.
  await page.getByTestId('thumb-2').click();
  await expect(page.getByTestId('slide-status')).toHaveText('Slide 2 / 3');
  await insertTab(page);
  await command(page, 'insert.actionButtons');
  await expect(page.getByTestId('action-buttons-dialog')).toBeVisible();
  await page.getByTestId('action-button-actionButtonEnd').click();
  await expect(page.getByTestId('action-buttons-dialog')).toHaveCount(0);
  await expect(page.getByTestId('selection-box')).toHaveCount(1);
  // Its action shows in Insert › Action: click → last slide, highlighted.
  await command(page, 'insert.action');
  const actionDialog = page.getByTestId('action-dialog');
  await expect(actionDialog).toBeVisible();
  await expect(page.getByTestId('action-link')).toBeChecked();
  await expect(page.getByTestId('action-to')).toHaveValue('last');
  await expect(page.getByTestId('action-highlight')).toBeChecked();
  // A sound on hover (built-in, synthesised) next to it.
  await page.getByTestId('action-tab-hover').click();
  await page.getByTestId('action-sound').selectOption('click');
  await page.getByTestId('action-ok').click();
  await expect(actionDialog).toHaveCount(0);

  // Slide 3: a shape with a web link.
  await page.getByTestId('thumb-3').click();
  await expect(page.getByTestId('slide-status')).toHaveText('Slide 3 / 3');
  await insertTab(page);
  await command(page, 'insert.rect');
  await expect(page.getByTestId('selection-box')).toHaveCount(1);
  await command(page, 'insert.link');
  await expect(linkDialog).toBeVisible();
  // Other schemes cannot be entered as a web link.
  await page.getByTestId('link-url').fill('javascript:alert(1)');
  await expect(page.getByTestId('link-ok')).toBeDisabled();
  await page.getByTestId('link-url').fill('example.vn/blueoffice');
  await page.getByTestId('link-ok').click();
  await expect(linkDialog).toHaveCount(0);

  // Saved as hlinkClick / hlinkHover with relationships; reopened.
  const bytes = await saveAndReopen(page, 'actions.pptx');
  const s1 = zipEntry(bytes, 'ppt/slides/slide1.xml')!;
  expect(s1).toMatch(
    /<a:hlinkClick r:id="rId\d+" action="ppaction:\/\/hlinksldjump" tooltip="Tới trang 3"\/>/,
  );
  expect(zipEntry(bytes, 'ppt/slides/_rels/slide1.xml.rels')).toContain(
    'relationships/slide" Target="slide3.xml"',
  );
  const s2 = zipEntry(bytes, 'ppt/slides/slide2.xml')!;
  expect(s2).toContain('prst="actionButtonEnd"');
  expect(s2).toContain(
    '<a:hlinkClick r:id="" action="ppaction://hlinkshowjump?jump=lastslide" highlightClick="1"/>',
  );
  expect(s2).toMatch(
    /<a:hlinkHover r:id="" action="ppaction:\/\/noaction"><a:snd r:embed="rId\d+" name="click.wav"\/><\/a:hlinkHover>/,
  );
  expect(zipEntry(bytes, 'ppt/slides/_rels/slide3.xml.rels')).toContain(
    'Target="https://example.vn/blueoffice" TargetMode="External"',
  );

  // Present: the title of slide 1 jumps to slide 3.
  await page.keyboard.press('F5');
  const stage = page.getByTestId('show-stage');
  await expect(stage).toHaveAttribute('data-slide', '0');
  const link = page.locator('[data-testid^="show-link-"]');
  await expect(link).toHaveCount(1);
  await expect(link).toHaveAttribute('data-action', 'slide');
  await expect(link).toHaveAttribute('title', 'Tới trang 3');
  await link.click();
  await expect(stage).toHaveAttribute('data-slide', '2');

  // Slide 3: the web link asks first; "Cancel" opens nothing and the show stays.
  await expect(link).toHaveCount(1);
  await expect(link).toHaveAttribute('data-action', 'url');
  await link.click();
  const ask = page.getByTestId('show-link-ask');
  await expect(ask).toBeVisible();
  await expect(page.getByTestId('show-link-url')).toHaveText('https://example.vn/blueoffice');
  expect(context.pages()).toHaveLength(1);
  // Keys do not move the show while it asks; Esc answers "no" (and does not end the show).
  await page.keyboard.press('ArrowLeft');
  await expect(stage).toHaveAttribute('data-slide', '2');
  await page.keyboard.press('Escape');
  await expect(ask).toHaveCount(0);
  await expect(stage).toHaveAttribute('data-slide', '2');
  expect(context.pages()).toHaveLength(1);

  // Back to slide 2: its action button goes to the last slide.
  await page.keyboard.press('ArrowLeft');
  await expect(stage).toHaveAttribute('data-slide', '1');
  await expect(link).toHaveCount(1);
  await expect(link).toHaveAttribute('data-action', 'last');
  await expect(link).toHaveAttribute('data-hover', 'none');
  await link.click();
  await expect(stage).toHaveAttribute('data-slide', '2');

  // "Open": a new tab that has no way back to the app.
  await link.click();
  await expect(ask).toBeVisible();
  const popup = context.waitForEvent('page');
  await page.getByTestId('show-link-open').click();
  const opened = await popup;
  await opened.waitForLoadState();
  expect(opened.url()).toBe('https://example.vn/blueoffice');
  expect(await opened.evaluate(() => window.opener)).toBeNull();
  await opened.close();
  await expect(ask).toHaveCount(0);
  expect(errors).toEqual([]);
});

test('programs, macros and files of a deck are never run in the show', async ({
  page,
  context,
}) => {
  const errors: string[] = [];
  page.on('pageerror', (e) => errors.push(e.message));
  const requests: string[] = [];
  page.on('request', (r) => {
    if (!r.url().startsWith('http://localhost')) requests.push(r.url());
  });
  await page.goto('/slide');
  await openDeck(page, unsafe);

  await page.keyboard.press('F5');
  const stage = page.getByTestId('show-stage');
  await expect(stage).toHaveAttribute('data-slide', '0');
  const blocked = page.getByTestId('show-link-blocked');
  for (const [action, reason, text] of [
    ['program', 'program', 'không chạy chương trình'],
    ['macro', 'macro', 'không chạy macro'],
    ['file', 'file', 'không mở tệp'],
    ['presentation', 'presentation', 'không mở bài trình chiếu khác'],
    ['url', 'scheme', 'không được phép mở'],
  ] as const) {
    const area = page.locator(`[data-testid^="show-link-"][data-action="${action}"]`);
    await expect(area).toHaveCount(1);
    await area.click();
    await expect(blocked).toBeVisible();
    await expect(blocked).toHaveAttribute('data-reason', reason);
    await expect(blocked).toContainText(text);
    // Nothing to open from the notice; the show did not move.
    await expect(page.getByTestId('show-link-open')).toHaveCount(0);
    await expect(page.getByTestId('show-link-ask')).toHaveCount(0);
    await page.getByTestId('show-link-dismiss').click();
    await expect(blocked).toHaveCount(0);
    await expect(stage).toHaveAttribute('data-slide', '0');
  }
  // The unknown hover action: moving over it does nothing at all.
  const hover = page.locator('[data-testid^="show-link-"][data-hover="unknown"]');
  await expect(hover).toHaveCount(1);
  await hover.hover();
  await expect(blocked).toHaveCount(0);
  await expect(stage).toHaveAttribute('data-slide', '0');
  expect(context.pages()).toHaveLength(1);
  expect(requests).toEqual([]);
  await page.keyboard.press('Escape');
  await expect(page.getByTestId('slide-overlay')).toBeVisible();

  // Saving keeps every one of them as it was.
  const download = page.waitForEvent('download');
  await page.keyboard.press('Control+s');
  const saved = readFileSync(await (await download).path());
  expect(zipEntry(saved, 'ppt/slides/slide1.xml')).toBe(
    zipEntry(readFileSync(unsafe), 'ppt/slides/slide1.xml'),
  );
  expect(zipEntry(saved, 'ppt/slides/_rels/slide1.xml.rels')).toBe(
    zipEntry(readFileSync(unsafe), 'ppt/slides/_rels/slide1.xml.rels'),
  );
  expect(errors).toEqual([]);
});
