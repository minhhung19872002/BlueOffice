import { expect, test, type Page } from '@playwright/test';
import { readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

// A legacy binary file (.doc / .xls) with a VBA project saves as the macro-enabled type
// (.docm / .xlsm) by default, with the project carried over; picking the macro-free type
// in File › Save As warns first that the macros will be left out. The files are
// generated (`bo-cli sample-macros tests/corpus/macro`).

const corpus = join(dirname(fileURLToPath(import.meta.url)), '..', '..', 'corpus', 'macro');
const cmd = (page: Page, id: string) => page.locator(`[data-command="${id}"]`);

/** Names of the entries of a zip package (central directory). */
function zipNames(zip: Buffer): string[] {
  const end = zip.lastIndexOf(Buffer.from([0x50, 0x4b, 0x05, 0x06]));
  let at = zip.readUInt32LE(end + 16);
  const out: string[] = [];
  for (let n = zip.readUInt16LE(end + 10); n > 0; n--) {
    const nameLen = zip.readUInt16LE(at + 28);
    out.push(zip.toString('utf8', at + 46, at + 46 + nameLen));
    at += 46 + nameLen + zip.readUInt16LE(at + 30) + zip.readUInt16LE(at + 32);
  }
  return out;
}

/** File › Save As: the note shown for `ext`, then save as `ext`. */
async function saveAs(page: Page, ext: string) {
  await page.getByTestId('file-tab').click();
  await page
    .getByRole('navigation', { name: 'Tệp' })
    .getByRole('button', { name: 'Lưu thành', exact: true })
    .click();
  await expect(page.getByTestId('backstage')).toHaveAttribute('data-page', 'saveAs');
  const entry = page.locator(`[data-format=".${ext}"]`);
  const note = (await entry.textContent()) ?? '';
  const download = page.waitForEvent('download');
  await entry.click();
  const d = await download;
  return { note, name: d.suggestedFilename(), bytes: readFileSync(await d.path()) };
}

const DROPPED = 'Kiểu tệp này không chứa được macro nên macro sẽ bị bỏ khi lưu';

test('BlueDoc: a .doc with macros saves as .docm and keeps its VBA project', async ({ page }) => {
  const errors: string[] = [];
  page.on('pageerror', (e) => errors.push(e.message));
  await page.goto('/doc');
  const chooser = page.waitForEvent('filechooser');
  await page.getByRole('button', { name: 'Mở tệp từ máy' }).click();
  await (await chooser).setFiles(join(corpus, 'macro-legacy.doc'));
  await expect(page.locator('.bo-page[data-page="1"]')).toHaveAttribute('data-rendered', 'true');
  await expect(page.getByTestId('macro-warning')).toBeVisible();
  await expect(page.getByTestId('macro-signature-dropped')).toHaveCount(0);

  // Save: the macro-enabled type, with the project.
  const download = page.waitForEvent('download');
  await page.keyboard.press('Control+s');
  const saved = await download;
  expect(saved.suggestedFilename()).toBe('macro-legacy.docm');
  expect(zipNames(readFileSync(await saved.path()))).toContain('word/vbaProject.bin');

  // Save As .docx: warned first, then saved without macros.
  const docx = await saveAs(page, 'docx');
  expect(docx.note).toContain(DROPPED);
  expect(docx.name).toBe('macro-legacy.docx');
  expect(zipNames(docx.bytes)).not.toContain('word/vbaProject.bin');
  expect(errors).toEqual([]);
});

test('BlueSheet: an .xls with macros saves as .xlsm and keeps its VBA project', async ({
  page,
}) => {
  await page.goto('/sheet');
  await expect(page.getByTestId('sheet-canvas')).toHaveAttribute('data-rendered', /\d+/);
  const chooser = page.waitForEvent('filechooser');
  await cmd(page, 'file.open').first().click();
  await (await chooser).setFiles(join(corpus, 'macro-legacy.xls'));
  await expect(page.getByTestId('macro-warning')).toBeVisible();

  const download = page.waitForEvent('download');
  await cmd(page, 'file.save').first().click();
  const saved = await download;
  expect(saved.suggestedFilename()).toBe('macro-legacy.xlsm');
  expect(zipNames(readFileSync(await saved.path()))).toContain('xl/vbaProject.bin');

  const xlsx = await saveAs(page, 'xlsx');
  expect(xlsx.note).toContain(DROPPED);
  expect(xlsx.name).toBe('macro-legacy.xlsx');
  expect(zipNames(xlsx.bytes)).not.toContain('xl/vbaProject.bin');
});
