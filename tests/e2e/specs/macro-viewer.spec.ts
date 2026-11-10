import { expect, test, type Page } from '@playwright/test';
import { readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { inflateRawSync } from 'node:zlib';

// CORE-VBA-002: the VBA macro source of an opened file is shown read-only (never run),
// one module can be sent to the BlueScript converter as a draft, and the VBA project is
// still in the saved file byte for byte. The files are generated
// (`bo-cli sample-macros tests/corpus/macro`).

const corpus = join(dirname(fileURLToPath(import.meta.url)), '..', '..', 'corpus', 'macro');
const cmd = (page: Page, id: string) => page.locator(`[data-command="${id}"]`);

/** One entry of a zip package (stored or deflated), from the central directory. */
function zipEntry(zip: Buffer, name: string): Buffer | null {
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
    if (zip.toString('utf8', p + 46, p + 46 + nameLen) === name) {
      const start = local + 30 + zip.readUInt16LE(local + 26) + zip.readUInt16LE(local + 28);
      const data = zip.subarray(start, start + size);
      return method === 8 ? inflateRawSync(data) : Buffer.from(data);
    }
    p += 46 + nameLen + extraLen + commentLen;
  }
  return null;
}

/** The viewer opened from the macro warning bar, showing the generated project. */
async function openViewer(page: Page, documentModule: string) {
  await expect(page.getByTestId('macro-warning')).toBeVisible();
  await page.getByTestId('macro-warning-view').click();
  const viewer = page.getByTestId('macro-viewer');
  await expect(viewer.getByTestId('macro-viewer-notice')).toContainText(
    'BlueOffice không bao giờ chạy VBA',
  );
  const modules = viewer.getByRole('option');
  await expect(modules).toHaveCount(2);
  await expect(modules.nth(0)).toContainText(documentModule);
  await expect(modules.nth(1)).toContainText('Module1');
  // The standard module with the code is the one shown.
  await expect(modules.nth(1)).toHaveAttribute('aria-selected', 'true');
  return viewer;
}

test('BlueSheet: macros of an .xlsm are shown read-only, drafted as BlueScript and kept on save', async ({
  page,
}) => {
  const errors: string[] = [];
  page.on('pageerror', (e) => errors.push(e.message));
  const file = join(corpus, 'macro-vba.xlsm');
  await page.goto('/sheet');
  await expect(page.getByTestId('sheet-canvas')).toHaveAttribute('data-rendered', /\d+/);
  const chooser = page.waitForEvent('filechooser');
  await cmd(page, 'file.open').click();
  await (await chooser).setFiles(file);

  const viewer = await openViewer(page, 'ThisWorkbook');
  const code = viewer.getByTestId('macro-viewer-code');
  const source =
    "Option Explicit\n\n' Ghi lời chào — mã này chỉ được hiển thị, không bao giờ chạy.\n" +
    'Sub ChaoMung()\n    Range("A1").Value = "Xin chào"\n    Range("A2").Value = 42\nEnd Sub\n';
  await expect(code).toHaveValue(source);
  // Read-only: typing into the code changes nothing.
  await expect(code).toHaveJSProperty('readOnly', true);
  await code.click();
  await page.keyboard.type('MsgBox 1');
  await expect(code).toHaveValue(source);
  // The document module is listed too.
  await viewer.getByRole('option').nth(0).click();
  await expect(code).toHaveValue('Option Explicit\n');
  await viewer.getByRole('option').nth(1).click();

  // Convert: the viewer closes and the draft is in the BlueScript editor, not run.
  await viewer.getByTestId('macro-viewer-convert').click();
  await expect(viewer).toHaveCount(0);
  const pane = page.getByTestId('script-pane');
  await expect(pane).toBeVisible();
  const script = pane.getByTestId('script-code');
  await expect(script).toHaveValue(/await sheet\.range\("A1"\)\.setValue\("Xin chào"\);/);
  await expect(script).toHaveValue(/await sheet\.range\("A2"\)\.setValue\(42\);/);
  await expect(pane.getByTestId('script-name')).toHaveValue('Bản nháp từ macro VBA: Module1');
  await expect(pane.getByTestId('script-list')).toHaveValue('');
  const out = pane.getByTestId('script-console');
  await expect(out).toContainText('Bản nháp chưa được chạy');

  // Neither the macro nor the draft ran: the cells still hold the text of the file.
  await script.fill(`console.log(JSON.stringify(await sheet.range('A1:A2').values));`);
  await pane.getByTestId('script-run').click();
  await expect(out).toContainText('[["Tệp mẫu có macro VBA"],["Mã chỉ được hiển thị"]]');

  // The registry command opens the same viewer.
  await page.getByRole('tab', { name: 'Xem', exact: true }).click();
  await cmd(page, 'script.viewMacros').click();
  await expect(page.getByTestId('macro-viewer-code')).toHaveValue(source);
  await page.getByTestId('macro-viewer').getByRole('button', { name: 'Đóng' }).click();
  await expect(page.getByTestId('macro-viewer')).toHaveCount(0);

  // Saving keeps the file macro-enabled with the very same VBA project.
  await page.getByRole('tab', { name: 'Trang đầu' }).click();
  const download = page.waitForEvent('download');
  await cmd(page, 'file.save').first().click();
  const saved = await download;
  expect(saved.suggestedFilename()).toBe('macro-vba.xlsm');
  const bytes = readFileSync(await saved.path());
  const original = zipEntry(readFileSync(file), 'xl/vbaProject.bin');
  expect(original).not.toBeNull();
  expect(zipEntry(bytes, 'xl/vbaProject.bin')?.equals(original!)).toBe(true);
  expect(errors).toEqual([]);
});

test('BlueDoc: a module of a .docm becomes a draft and the document is not touched', async ({
  page,
}) => {
  await page.goto('/doc');
  const chooser = page.waitForEvent('filechooser');
  await page.getByRole('button', { name: 'Mở tệp từ máy' }).click();
  await (await chooser).setFiles(join(corpus, 'macro-vba.docm'));
  await expect(page.locator('.bo-page[data-page="1"]')).toHaveAttribute('data-rendered', 'true');

  const viewer = await openViewer(page, 'ThisDocument');
  await expect(viewer.getByTestId('macro-viewer-code')).toHaveValue(
    /Sub ChaoMung\(\)\n {4}Selection\.TypeText "Xin chào"\nEnd Sub\n$/,
  );
  await viewer.getByTestId('macro-viewer-convert').click();
  const pane = page.getByTestId('script-pane');
  const script = pane.getByTestId('script-code');
  await expect(script).toHaveValue(/await doc\.insertText\("Xin chào"\);/);
  await expect(pane.getByTestId('script-console')).toContainText('Bản nháp chưa được chạy');

  // The text of the document is what the file holds: nothing was typed by a macro.
  await script.fill(`console.log('text=[' + (await doc.text) + ']');`);
  await pane.getByTestId('script-run').click();
  await expect(pane.getByTestId('script-console')).toContainText(
    'text=[Tệp mẫu có macro VBA. BlueOffice chỉ hiển thị mã, không bao giờ chạy.]',
  );
});

test('BlueSlide: macros of a .pptm are shown read-only', async ({ page }) => {
  await page.goto('/slide');
  const chooser = page.waitForEvent('filechooser');
  await page.getByRole('button', { name: 'Mở', exact: true }).click();
  await (await chooser).setFiles(join(corpus, 'macro-vba.pptm'));
  await expect(page.getByTestId('slide-status')).toHaveText(/^Slide 1 \/ 1$/);

  const viewer = await openViewer(page, 'Slide1');
  const code = viewer.getByTestId('macro-viewer-code');
  await expect(code).toHaveValue(/Sub ChaoMung\(\)\n {4}MsgBox "Xin chào"\nEnd Sub\n$/);
  await expect(code).toHaveJSProperty('readOnly', true);
  await page.keyboard.press('Escape');
  await expect(viewer).toHaveCount(0);
  // The warning bar is still there: looking at the macros does not dismiss it.
  await expect(page.getByTestId('macro-warning')).toBeVisible();
});
