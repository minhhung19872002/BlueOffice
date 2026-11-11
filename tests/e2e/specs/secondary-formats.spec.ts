import { expect, test, type Page } from '@playwright/test';
import { readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { inflateRawSync } from 'node:zlib';

// CORE-FMT-004: BlueSheet opens SYLK, DIF, formatted text (.prn) and dBASE tables and
// saves a sheet as SYLK / DIF / formatted text; BlueDoc exports an EPUB 3 publication
// and opens PDF files (best effort, the text).
const here = dirname(fileURLToPath(import.meta.url));
const corpus = (name: string) => join(here, '..', '..', 'corpus', 'sheet', name);

async function goTo(page: Page, ref: string) {
  const box = page.getByLabel('Hộp tên');
  await box.click();
  await box.fill(ref);
  await box.press('Enter');
  await expect(page.getByLabel('Ô đang nhập')).toBeFocused();
}

async function valueOf(page: Page, ref: string) {
  await goTo(page, ref);
  return page.getByLabel('Thanh công thức');
}

async function openSheet(page: Page, path: string) {
  await page.goto('/sheet');
  await expect(page.getByRole('tab', { name: 'Sheet1' })).toBeVisible();
  const chooser = page.waitForEvent('filechooser');
  await page.locator('[data-command="file.open"]').first().click();
  await (await chooser).setFiles(path);
}

/** File › Save As › the format with this extension; the downloaded file. */
async function saveAs(page: Page, ext: string) {
  await page.getByTestId('file-tab').click();
  await page
    .getByRole('navigation', { name: 'Tệp' })
    .getByRole('button', { name: 'Lưu thành', exact: true })
    .click();
  await expect(page.getByTestId('backstage')).toHaveAttribute('data-page', 'saveAs');
  const download = page.waitForEvent('download');
  await page.locator(`[data-format=".${ext}"]`).click();
  const d = await download;
  await expect(page.getByTestId('backstage')).toHaveCount(0);
  const path = await d.path();
  return { name: d.suggestedFilename(), path, bytes: readFileSync(path) };
}

/** Entries of a ZIP file in the order of its central directory: name → bytes. */
function unzip(zip: Buffer): Map<string, Buffer> {
  const out = new Map<string, Buffer>();
  const end = zip.lastIndexOf(Buffer.from([0x50, 0x4b, 0x05, 0x06]));
  let at = zip.readUInt32LE(end + 16);
  for (let n = zip.readUInt16LE(end + 10); n > 0; n--) {
    const method = zip.readUInt16LE(at + 10);
    const size = zip.readUInt32LE(at + 20);
    const nameLen = zip.readUInt16LE(at + 28);
    const skip = zip.readUInt16LE(at + 30) + zip.readUInt16LE(at + 32);
    const local = zip.readUInt32LE(at + 42);
    const name = zip.subarray(at + 46, at + 46 + nameLen).toString('utf8');
    const data = local + 30 + zip.readUInt16LE(local + 26) + zip.readUInt16LE(local + 28);
    const raw = zip.subarray(data, data + size);
    out.set(name, method === 0 ? raw : inflateRawSync(raw));
    at += 46 + nameLen + skip;
  }
  return out;
}

test('BlueSheet opens SYLK, DIF, formatted text and dBASE tables', async ({ page }) => {
  const errors: string[] = [];
  page.on('pageerror', (e) => errors.push(e.message));

  // SYLK: text with `;` and quotes, R1C1 expressions as formulas, a shared expression.
  await openSheet(page, corpus('tabular-1-orders.slk'));
  await expect(await valueOf(page, 'A2')).toHaveValue('Bút bi; xanh');
  await expect(await valueOf(page, 'A3')).toHaveValue('Vở "Hồng Hà"');
  await expect(await valueOf(page, 'D2')).toHaveValue('=B2*C2');
  await expect(await valueOf(page, 'D3')).toHaveValue('=B3*C3');
  await expect(await valueOf(page, 'D6')).toHaveValue('=SUM(D2:D4)');
  // The formulas calculate: change a quantity and the total follows.
  await goTo(page, 'B2');
  await page.keyboard.type('200');
  await page.keyboard.press('Enter');
  const csv = (await saveAs(page, 'csv')).bytes.toString('utf8').split('\r\n');
  expect(csv[1]).toBe('"Bút bi; xanh";200;3500;700000;45366');
  expect(csv[5]).toContain(';1320000;');

  // DIF: numbers, booleans, a text with a doubled quote.
  await openSheet(page, corpus('tabular-1-orders.dif'));
  await expect(await valueOf(page, 'A3')).toHaveValue('Vở "Hồng Hà"');
  await expect(await valueOf(page, 'B2')).toHaveValue('120');
  await expect(await valueOf(page, 'C3')).toHaveValue('12500');
  await expect(await valueOf(page, 'D2')).toHaveValue('TRUE');

  // Formatted text: columns from the blank positions.
  await openSheet(page, corpus('tabular-1-orders.prn'));
  await expect(await valueOf(page, 'A3')).toHaveValue('Vở Hồng Hà');
  await expect(await valueOf(page, 'B1')).toHaveValue('SL');
  await expect(await valueOf(page, 'C3')).toHaveValue('12500');
  await expect(await valueOf(page, 'D4')).toHaveValue('120000');

  // dBASE: field names in the first row, Windows-1252 text, the deleted record skipped.
  await openSheet(page, corpus('tabular-1-customers.dbf'));
  await expect(await valueOf(page, 'A1')).toHaveValue('NAME');
  await expect(await valueOf(page, 'A2')).toHaveValue('Café Crème');
  await expect(await valueOf(page, 'B3')).toHaveValue('København');
  await expect(await valueOf(page, 'D2')).toHaveValue('4,5');
  await expect(await valueOf(page, 'E2')).toHaveValue('15/03/2024');
  await expect(await valueOf(page, 'A4')).toHaveValue('Jalapeño');
  expect(errors).toEqual([]);
});

test('BlueSheet saves the sheet as SYLK, DIF and formatted text; they open again', async ({
  page,
}) => {
  const errors: string[] = [];
  page.on('pageerror', (e) => errors.push(e.message));
  await page.goto('/sheet');
  await expect(page.getByTestId('sheet-canvas')).toHaveAttribute('data-rendered', /\d+/);
  const rows = [
    ['Tên', 'SL'],
    ['Bút; "xanh"', '12'],
    ['Vở', '=B2*2'],
  ];
  for (const [i, row] of rows.entries()) {
    await goTo(page, `A${i + 1}`);
    for (const v of row) {
      await page.keyboard.type(v);
      await page.keyboard.press('Tab');
    }
  }
  await expect(await valueOf(page, 'B3')).toHaveValue('=B2*2');

  const slk = await saveAs(page, 'slk');
  expect(slk.name).toMatch(/\.slk$/);
  const sylk = slk.bytes.toString('utf8');
  expect(sylk.startsWith('ID;PBLUEOFFICE;N;E\r\n')).toBe(true);
  expect(sylk).toContain('C;Y2;X1;K"Bút;; ""xanh"""\r\n');
  expect(sylk).toContain('C;Y3;X2;K24;ER[-1]C*2\r\n');
  expect(sylk.endsWith('E\r\n')).toBe(true);

  const dif = await saveAs(page, 'dif');
  expect(dif.name).toMatch(/\.dif$/);
  const difText = dif.bytes.toString('utf8');
  expect(difText.startsWith('TABLE\r\n0,1\r\n')).toBe(true);
  expect(difText).toContain('1,0\r\n"Bút; ""xanh"""\r\n0,12\r\nV\r\n');
  expect(difText.endsWith('-1,0\r\nEOD\r\n')).toBe(true);

  const prn = await saveAs(page, 'prn');
  expect(prn.name).toMatch(/\.prn$/);
  const lines = prn.bytes.toString('utf8').split('\r\n');
  expect(lines[0]).toMatch(/^Tên +SL$/);
  expect(lines[1]).toMatch(/^Bút; "xanh" +12$/);
  expect(lines[2]).toMatch(/^Vở +24$/);
  // Numbers end at the same position: the column is right-aligned.
  expect(lines[1]!.length).toBe(lines[2]!.length);

  // The SYLK file opens with its formula; the DIF file with the values.
  await openSheet(page, slk.path);
  await expect(await valueOf(page, 'A2')).toHaveValue('Bút; "xanh"');
  await expect(await valueOf(page, 'B3')).toHaveValue('=B2*2');
  await openSheet(page, dif.path);
  await expect(await valueOf(page, 'A2')).toHaveValue('Bút; "xanh"');
  await expect(await valueOf(page, 'B3')).toHaveValue('24');
  expect(errors).toEqual([]);
});

test('BlueDoc exports an EPUB with a chapter per Heading 1', async ({ page }) => {
  const errors: string[] = [];
  page.on('pageerror', (e) => errors.push(e.message));
  await page.goto('/doc');
  await page.getByTestId('new-document').click();
  await expect(page.getByTestId('doc-name')).toHaveText('Tài liệu mới.docx');
  await expect(page.getByTestId('caret')).toBeVisible();
  await page.keyboard.insertText('Chương một');
  await page.keyboard.press('Enter');
  await page.keyboard.insertText('Nội dung <của> chương & hơn nữa');
  await page.keyboard.press('Enter');
  await page.keyboard.insertText('Chương hai');
  await page.keyboard.press('Control+Home');
  await page.locator('[data-style-command="style.heading1"]').first().click();
  await page.keyboard.press('Control+End');
  await page.locator('[data-style-command="style.heading1"]').first().click();

  const epub = await saveAs(page, 'epub');
  expect(epub.name).toBe('Tài liệu mới.epub');
  // The container rule: `mimetype` first, stored, so the media type is at byte 38.
  expect(epub.bytes.subarray(0, 4).toString('latin1')).toBe('PK\u0003\u0004');
  expect(epub.bytes.subarray(30, 58).toString('latin1')).toBe('mimetypeapplication/epub+zip');
  const files = unzip(epub.bytes);
  expect([...files.keys()]).toEqual([
    'mimetype',
    'META-INF/container.xml',
    'OEBPS/content.opf',
    'OEBPS/nav.xhtml',
    'OEBPS/style.css',
    'OEBPS/chapter-1.xhtml',
    'OEBPS/chapter-2.xhtml',
  ]);
  const text = (name: string) => files.get(name)!.toString('utf8');
  expect(text('META-INF/container.xml')).toContain('full-path="OEBPS/content.opf"');
  const opf = text('OEBPS/content.opf');
  expect(opf).toContain('<dc:title>Tài liệu mới</dc:title>');
  expect(opf).toMatch(/<dc:identifier id="pub-id">urn:uuid:[0-9a-f-]{36}<\/dc:identifier>/);
  expect(opf).toMatch(/<meta property="dcterms:modified">\d{4}-\d\d-\d\dT\d\d:\d\d:\d\dZ<\/meta>/);
  expect(opf).toContain('<itemref idref="chapter-1"/>\n<itemref idref="chapter-2"/>');
  const nav = text('OEBPS/nav.xhtml');
  expect(nav).toContain('<li><a href="chapter-1.xhtml">Chương một</a></li>');
  expect(nav).toContain('<li><a href="chapter-2.xhtml">Chương hai</a></li>');
  const one = text('OEBPS/chapter-1.xhtml');
  expect(one).toContain('Chương một</h1>');
  expect(one).toContain('Nội dung &lt;của&gt; chương &amp; hơn nữa');
  expect(one).not.toContain('Chương hai');
  expect(text('OEBPS/chapter-2.xhtml')).toContain('Chương hai</h1>');
  // The document itself is unchanged and still a .docx.
  await expect(page.getByTestId('doc-name')).toHaveText('Tài liệu mới.docx');
  expect(errors).toEqual([]);
});

test('BlueDoc opens a PDF file as a document of its text', async ({ page }) => {
  const errors: string[] = [];
  page.on('pageerror', (e) => errors.push(e.message));
  await page.goto('/doc');
  await page.getByTestId('new-document').click();
  await expect(page.getByTestId('caret')).toBeVisible();
  await page.keyboard.insertText('Báo cáo tháng mười');
  await page.keyboard.press('Enter');
  await page.keyboard.insertText('Doanh thu tăng đều');
  const pdf = await saveAs(page, 'pdf');
  expect(pdf.bytes.subarray(0, 5).toString('latin1')).toBe('%PDF-');

  // Open it again: recognised by content, the text comes back, saved as .docx.
  await page.getByTestId('file-tab').click();
  await page
    .getByRole('navigation', { name: 'Tệp' })
    .getByRole('button', { name: 'Mở', exact: true })
    .click();
  await expect(page.getByTestId('backstage')).toHaveAttribute('data-page', 'open');
  const chooser = page.waitForEvent('filechooser');
  await page.locator('[data-testid="backstage"] [data-command="file.open"]').click();
  await (
    await chooser
  ).setFiles({
    name: 'Báo cáo.pdf',
    mimeType: 'application/pdf',
    buffer: pdf.bytes,
  });
  await expect(page.getByTestId('doc-name')).toHaveText('Báo cáo.docx');
  await expect(page.locator('.bo-page[data-page="1"]')).toHaveAttribute('data-rendered', 'true');
  // The words are in the document: its web page export has both lines.
  const html = (await saveAs(page, 'html')).bytes.toString('utf8');
  expect(html).toContain('Báo cáo tháng mười');
  expect(html).toContain('Doanh thu tăng đều');
  expect(errors).toEqual([]);
});
