import { expect, test, type Page } from '@playwright/test';
import { copyFileSync, mkdirSync, readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { inflateRawSync } from 'node:zlib';

const here = dirname(fileURLToPath(import.meta.url));
const outDir = join(here, '..', '..', 'fidelity', 'output');
const corpus = join(here, '..', '..', 'corpus', 'doc', 'math-06-unsupported.docx');
const foreign = join(here, '..', '..', 'corpus', 'doc', 'math-07-foreign-style.docx');

/** One entry of a zip package (stored or deflated), from the central directory. */
function zipEntry(zip: Buffer, want: string): string {
  let eocd = zip.length - 22;
  while (eocd >= 0 && zip.readUInt32LE(eocd) !== 0x06054b50) eocd--;
  if (eocd < 0) throw new Error('not a zip');
  const entries = zip.readUInt16LE(eocd + 10);
  let p = zip.readUInt32LE(eocd + 16);
  for (let i = 0; i < entries; i++) {
    const method = zip.readUInt16LE(p + 10);
    const size = zip.readUInt32LE(p + 20);
    const nameLen = zip.readUInt16LE(p + 28);
    const extraLen = zip.readUInt16LE(p + 30);
    const commentLen = zip.readUInt16LE(p + 32);
    const local = zip.readUInt32LE(p + 42);
    const name = zip.toString('utf8', p + 46, p + 46 + nameLen);
    if (name === want) {
      const start = local + 30 + zip.readUInt16LE(local + 26) + zip.readUInt16LE(local + 28);
      const data = zip.subarray(start, start + size);
      return (method === 8 ? inflateRawSync(data) : Buffer.from(data)).toString('utf8');
    }
    p += 46 + nameLen + extraLen + commentLen;
  }
  throw new Error(`${want} not in the package`);
}

const count = (s: string, part: string) => s.split(part).length - 1;

// CORE-MATH-010: edit an equation in place — click into a slot, type, Tab, autobuild, Backspace,
// undo, save and reopen.
const cmd = (page: Page, id: string) => page.locator(`[data-command="${id}"]`);
const page1 = (page: Page) => page.locator('.bo-page[data-page="1"]');

async function box(page: Page, testId: string) {
  const el = page.getByTestId(testId).first();
  // The caret is redrawn after each key: wait until the new one is laid out.
  await expect(el).toBeVisible();
  let b = await el.boundingBox();
  await expect.poll(async () => (b = await el.boundingBox()) !== null).toBe(true);
  if (!b) throw new Error(`${testId} has no box`);
  return b;
}

test('edits an equation in place', async ({ page }) => {
  const errors: string[] = [];
  page.on('pageerror', (e) => errors.push(e.message));
  await page.goto('/doc');
  await page.getByTestId('new-document').click();
  await expect(page.getByTestId('caret')).toBeVisible();
  await page.keyboard.insertText('Ta có ');

  // Insert a/b.
  await page.getByRole('tab', { name: 'Chèn' }).click();
  await cmd(page, 'insert.equation').click();
  const dialog = page.getByTestId('equation-dialog');
  await dialog.getByTestId('equation-input').fill('a/b');
  await dialog.getByTestId('equation-ok').click();
  await expect(dialog).toHaveCount(0);

  // Into the equation: its box is shown; Esc leaves it.
  await cmd(page, 'equation.editInPlace').click();
  await expect(page.getByTestId('bo-math-zone')).toBeVisible();
  const zone = await box(page, 'bo-math-zone');
  await page.keyboard.press('Escape');
  await expect(page.getByTestId('bo-math-zone')).toHaveCount(0);

  // Click into the denominator (below the fraction bar, in the middle).
  await page.mouse.click(zone.x + zone.width / 2, zone.y + zone.height * 0.78);
  await expect(page.getByTestId('bo-math-zone')).toBeVisible();
  const inDen = await box(page, 'caret');
  expect(inDen.y).toBeGreaterThan(zone.y + zone.height / 2 - 2);
  expect(inDen.height).toBeLessThan(zone.height * 0.8);
  await page.keyboard.type('c');

  // Tab: the next slot (after the fraction, full height).
  await page.keyboard.press('Tab');
  const after = await box(page, 'caret');
  expect(after.height).toBeGreaterThan(inDen.height);
  expect(after.x).toBeGreaterThan(inDen.x);

  // Typing x^2 and a space builds the superscript: one step left enters it (a small, raised caret).
  await page.keyboard.type('+x^2 ');
  await page.keyboard.press('ArrowLeft');
  const inSup = await box(page, 'caret');
  expect(inSup.height).toBeLessThan(after.height * 0.85);
  expect(inSup.y + inSup.height).toBeLessThan(after.y + after.height - 2);
  // Out of the superscript: just after the structure.
  await page.keyboard.press('ArrowRight');

  // Backspace selects the superscript structure, a second one removes it.
  await page.keyboard.press('Backspace');
  await expect(page.locator('.bo-sel')).toHaveCount(1);
  await page.keyboard.press('Backspace');
  await expect(page.locator('.bo-sel')).toHaveCount(0);
  await page.keyboard.press('ArrowLeft');
  const noSup = await box(page, 'caret');
  expect(noSup.height).toBeGreaterThan(inSup.height);

  // Undo brings it back (still editing in place).
  await page.keyboard.press('Control+z');
  await expect(page.getByTestId('bo-math-zone')).toBeVisible();
  await page.keyboard.press('End');
  await page.keyboard.press('ArrowLeft');
  const supAgain = await box(page, 'caret');
  expect(supAgain.height).toBeLessThan(after.height * 0.85);
  await page.keyboard.press('Escape');

  // The equation dialog reads the edited equation back.
  await cmd(page, 'insert.equation').click();
  await expect(dialog.getByTestId('equation-input')).toHaveValue(/^a\/\((cb|bc)\)\+x\^2$/);
  const linear = await dialog.getByTestId('equation-input').inputValue();
  await page.keyboard.press('Escape');
  await expect(dialog).toHaveCount(0);

  // Save, reload, reopen: the equation reads back the same.
  const download = page.waitForEvent('download');
  await page.keyboard.press('Control+s');
  const path = await (await download).path();
  // Kept for validation (bo-cli validate tests/fidelity/output/web-equation-inline.docx).
  mkdirSync(outDir, { recursive: true });
  copyFileSync(path, join(outDir, 'web-equation-inline.docx'));
  await page.reload();
  const chooser = page.waitForEvent('filechooser');
  await page.getByRole('button', { name: 'Mở tệp từ máy' }).click();
  await (await chooser).setFiles(path);
  await expect(page1(page)).toHaveAttribute('data-rendered', 'true');
  await page.keyboard.press('Control+End');
  await page.getByRole('tab', { name: 'Chèn' }).click();
  await cmd(page, 'insert.equation').click();
  await expect(page.getByTestId('equation-input')).toHaveValue(linear);
  expect(errors).toEqual([]);
});

test('with track changes an in-place edit is one revision; rejecting it restores the equation', async ({
  page,
}) => {
  const errors: string[] = [];
  page.on('pageerror', (e) => errors.push(e.message));
  await page.goto('/doc');
  await page.getByTestId('new-document').click();
  await expect(page.getByTestId('caret')).toBeVisible();
  await page.keyboard.insertText('Ta có ');
  await page.getByRole('tab', { name: 'Chèn' }).click();
  await cmd(page, 'insert.equation').click();
  const dialog = page.getByTestId('equation-dialog');
  await dialog.getByTestId('equation-input').fill('a/b');
  await dialog.getByTestId('equation-ok').click();
  await expect(dialog).toHaveCount(0);

  // Track changes on, then edit the equation in place: several keys, one change.
  await page.getByRole('tab', { name: 'Xem lại' }).click();
  await cmd(page, 'review.trackChanges').click();
  await expect(cmd(page, 'review.trackChanges')).toHaveAttribute('aria-pressed', 'true');
  await page.getByRole('tab', { name: 'Chèn' }).click();
  await cmd(page, 'equation.editInPlace').click();
  await expect(page.getByTestId('bo-math-zone')).toBeVisible();
  await page.keyboard.type('+c');
  await page.keyboard.type('+x^2 ');
  await page.keyboard.press('Escape');
  await expect(page.getByTestId('bo-math-zone')).toHaveCount(0);
  await page.getByRole('tab', { name: 'Xem lại' }).click();
  await cmd(page, 'review.pane').click();
  await expect(page.getByTestId('review-pane')).toBeVisible();
  await expect(page.getByTestId('revision-count')).toHaveText('1 thay đổi');
  await expect(page.getByTestId('revision')).toHaveCount(1);
  await expect(page.getByTestId('revision').first()).toContainText('Sửa phương trình');

  // Kept for validation (bo-cli validate tests/fidelity/output/web-equation-tracked.docx).
  const download = page.waitForEvent('download');
  await page.keyboard.press('Control+s');
  const path = await (await download).path();
  mkdirSync(outDir, { recursive: true });
  copyFileSync(path, join(outDir, 'web-equation-tracked.docx'));

  // Reject: the original equation, no change left.
  await page.getByTestId('revision').first().getByRole('button', { name: 'Từ chối' }).click();
  await expect(page.getByTestId('revision-count')).toHaveText('0 thay đổi');
  await page.keyboard.press('Control+End');
  await page.getByRole('tab', { name: 'Chèn' }).click();
  await cmd(page, 'insert.equation').click();
  await expect(dialog.getByTestId('equation-input')).toHaveValue('a/b');
  await page.keyboard.press('Escape');
  expect(errors).toEqual([]);
});

test('an equation the in-place editor cannot keep is selected, not rewritten', async ({ page }) => {
  const errors: string[] = [];
  page.on('pageerror', (e) => errors.push(e.message));
  await page.goto('/doc');
  const chooser = page.waitForEvent('filechooser');
  await page.getByRole('button', { name: 'Mở tệp từ máy' }).click();
  await (await chooser).setFiles(corpus);
  await expect(page1(page)).toHaveAttribute('data-rendered', 'true');
  // The first paragraph starts with an equation whose numerator has argument properties (m:argPr).
  await page.keyboard.press('Control+Home');
  const start = await box(page, 'caret');
  await page.mouse.click(start.x + 4, start.y + start.height / 2);
  await expect(page.getByTestId('status-info')).toHaveText(
    'Phương trình này có định dạng mà chế độ sửa tại chỗ không giữ được. Nhấp đúp hoặc nhấn Enter để sửa trong hộp phương trình.',
  );
  await expect(page.getByTestId('bo-math-zone')).toHaveCount(0);
  await expect(page.locator('.bo-sel')).toHaveCount(1);
  // Typing does not replace or rewrite it.
  const words = await page.getByTestId('status-words').textContent();
  await page.keyboard.type('q');
  await expect(page.getByTestId('bo-math-zone')).toHaveCount(0);
  await expect(page.getByTestId('status-words')).toHaveText(words ?? '');
  // Enter opens the equation dialog for it; so does a double click.
  await page.keyboard.press('Enter');
  const dialog = page.getByTestId('equation-dialog');
  await expect(dialog).toBeVisible();
  await expect(dialog.getByTestId('equation-input')).toHaveValue(/a\+b\+c/);
  await page.keyboard.press('Escape');
  await expect(dialog).toHaveCount(0);
  await page.mouse.dblclick(start.x + 4, start.y + start.height / 2);
  await expect(dialog).toBeVisible();
  await page.keyboard.press('Escape');
  // Saved, the equation's markup is unchanged.
  const download = page.waitForEvent('download');
  await page.keyboard.press('Control+s');
  const saved = await (await download).path();
  mkdirSync(outDir, { recursive: true });
  copyFileSync(saved, join(outDir, 'web-equation-unsupported.docx'));
  expect(errors).toEqual([]);
});

test('an equation written by another application is edited in place and keeps its formatting', async ({
  page,
}) => {
  const errors: string[] = [];
  page.on('pageerror', (e) => errors.push(e.message));
  await page.goto('/doc');
  const chooser = page.waitForEvent('filechooser');
  await page.getByRole('button', { name: 'Mở tệp từ máy' }).click();
  await (await chooser).setFiles(foreign);
  await expect(page1(page)).toHaveAttribute('data-rendered', 'true');
  // The first paragraph starts with y=(a+b)/c+x^2: the math font on every run, m:ctrlPr in every
  // structure. A click enters it in place.
  await page.keyboard.press('Control+Home');
  const start = await box(page, 'caret');
  await page.mouse.click(start.x + 4, start.y + start.height / 2);
  await expect(page.getByTestId('bo-math-zone')).toBeVisible();
  // To the end of the zone; type and build up a subscript (a small caret inside it).
  await page.keyboard.press('End');
  await page.keyboard.type('+z_1 ');
  await page.keyboard.press('ArrowLeft');
  const inSub = await box(page, 'caret');
  expect(inSub.height).toBeLessThan(start.height);
  // Esc leaves the zone, the caret after it: the equation dialog reads it back.
  await page.keyboard.press('Escape');
  await expect(page.getByTestId('bo-math-zone')).toHaveCount(0);
  await page.getByRole('tab', { name: 'Chèn' }).click();
  await cmd(page, 'insert.equation').click();
  const dialog = page.getByTestId('equation-dialog');
  await expect(dialog.getByTestId('equation-input')).toHaveValue('y=(a+b)/c+x^2+z_1');
  await page.keyboard.press('Escape');
  await expect(dialog).toHaveCount(0);
  // Saved: every math run still has the math font, the typed ones too; the built subscript has
  // control properties with it; the other properties are all kept.
  const download = page.waitForEvent('download');
  await page.keyboard.press('Control+s');
  const saved = await (await download).path();
  mkdirSync(outDir, { recursive: true });
  copyFileSync(saved, join(outDir, 'web-equation-foreign.docx'));
  const before = zipEntry(readFileSync(foreign), 'word/document.xml');
  const after = zipEntry(readFileSync(saved), 'word/document.xml');
  const runs = after
    .split('<m:r>')
    .slice(1)
    .map((r) => r.split('</m:r>')[0] ?? '');
  expect(runs.length).toBeGreaterThan(count(before, '<m:r>'));
  for (const r of runs) expect(r).toContain('<w:rFonts ');
  expect(after).toContain('<m:sSub><m:sSubPr><m:ctrlPr><w:rPr><w:rFonts ');
  expect(count(after, '<m:ctrlPr>')).toBe(count(before, '<m:ctrlPr>') + 1);
  for (const keep of ['<m:grow ', '<m:mcs>', '<m:scr ', '<m:jc ', '<m:opEmu ', '<m:show '])
    expect(count(after, keep)).toBe(count(before, keep));
  expect(errors).toEqual([]);
});
