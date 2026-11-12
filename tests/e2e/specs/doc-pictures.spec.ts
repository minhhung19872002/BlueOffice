import { expect, test, type Page } from '@playwright/test';
import { mkdtempSync, readFileSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { inflateRawSync } from 'node:zlib';

/**
 * CORE-PIC-001 in BlueDoc: an SVG file inserted as a picture keeps its SVG next to a PNG
 * fallback ([MS-ODRAWXML] asvg:svgBlip), a click on the picture selects it (the Picture tab
 * comes), and "Convert to shapes" turns it into a group of freeforms (wpg:wgp).
 */

const SVG =
  '<svg xmlns="http://www.w3.org/2000/svg" width="200" height="100">' +
  '<rect x="0" y="0" width="100" height="100" fill="#ff0000"/>' +
  '<circle cx="150" cy="50" r="50" fill="#0000ff"/></svg>';

/** Entries of a zip (stored or deflated), by name. */
function zipEntries(buf: Buffer): Map<string, () => Buffer> {
  const out = new Map<string, () => Buffer>();
  let eocd = buf.length - 22;
  while (eocd >= 0 && buf.readUInt32LE(eocd) !== 0x06054b50) eocd--;
  const count = buf.readUInt16LE(eocd + 10);
  let at = buf.readUInt32LE(eocd + 16);
  for (let i = 0; i < count; i++) {
    const method = buf.readUInt16LE(at + 10);
    const size = buf.readUInt32LE(at + 20);
    const nameLen = buf.readUInt16LE(at + 28);
    const extra = buf.readUInt16LE(at + 30);
    const comment = buf.readUInt16LE(at + 32);
    const local = buf.readUInt32LE(at + 42);
    const name = buf.toString('utf8', at + 46, at + 46 + nameLen);
    const lname = buf.readUInt16LE(local + 26);
    const lextra = buf.readUInt16LE(local + 28);
    const start = local + 30 + lname + lextra;
    const data = buf.subarray(start, start + size);
    out.set(name, () => (method === 8 ? inflateRawSync(data) : Buffer.from(data)));
    at += 46 + nameLen + extra + comment;
  }
  return out;
}

async function save(page: Page): Promise<Map<string, () => Buffer>> {
  const download = page.waitForEvent('download');
  await page.keyboard.press('Control+s');
  return zipEntries(readFileSync(await (await download).path()));
}

test('BlueDoc: SVG picture, click to select, convert to shapes', async ({ page }) => {
  const errors: string[] = [];
  page.on('pageerror', (e) => errors.push(e.message));
  const dir = mkdtempSync(join(tmpdir(), 'bo-e2e-svg-'));
  const file = join(dir, 'logo.svg');
  writeFileSync(file, SVG);
  await page.goto('/doc');
  await page.getByTestId('new-document').click();
  await expect(page.getByTestId('caret')).toBeVisible();
  const chooser = page.waitForEvent('filechooser');
  await page.getByRole('tab', { name: 'Chèn', exact: true }).click();
  await page.locator('[data-command="insert.image"]').first().click();
  await (await chooser).setFiles(file);
  const pictureTab = page.getByRole('tab', { name: 'Định dạng ảnh', exact: true });
  await expect(pictureTab).toBeVisible();
  // The caret follows the picture: its right edge.
  const caret = (await page.getByTestId('caret').boundingBox())!;

  // Saved with the SVG and its PNG fallback.
  let zip = await save(page);
  let xml = zip.get('word/document.xml')!().toString('utf8');
  expect(xml).toContain('asvg:svgBlip');
  expect([...zip.keys()].some((n) => /^word\/media\/.*\.svg$/.test(n))).toBe(true);
  expect([...zip.keys()].some((n) => /^word\/media\/.*\.png$/.test(n))).toBe(true);

  // Move away (a new paragraph of text): no picture at the caret.
  await page.keyboard.press('End');
  await page.keyboard.press('Enter');
  await page.keyboard.insertText('Chữ sau ảnh');
  await expect(pictureTab).toHaveCount(0);
  // A click on the picture selects it.
  await page.mouse.click(caret.x - 30, caret.y + caret.height / 2);
  await expect(pictureTab).toBeVisible();
  await pictureTab.click();
  const toShapes = page.locator('[data-command="picture.toShapes"]').first();
  await expect(toShapes).toBeVisible();
  await toShapes.click();
  await expect(pictureTab).toHaveCount(0);

  zip = await save(page);
  xml = zip.get('word/document.xml')!().toString('utf8');
  expect(xml).toContain('wpg:wgp');
  expect(xml).not.toContain('svgBlip');
  expect(errors).toEqual([]);
});
