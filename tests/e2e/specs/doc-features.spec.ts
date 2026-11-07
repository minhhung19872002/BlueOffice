import { expect, test, type Page } from '@playwright/test';
import { mkdirSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const outDir = join(dirname(fileURLToPath(import.meta.url)), '..', '..', 'fidelity', 'output');
const cmd = (page: Page, id: string) => page.locator(`[data-command="${id}"]`);
const page1 = (page: Page) => page.locator('.bo-page[data-page="1"]');

async function newDocument(page: Page) {
  await page.goto('/doc');
  await page.getByTestId('new-document').click();
  await expect(page.getByTestId('caret')).toBeVisible();
}

/** Caret box (retried: the caret element is replaced after every command). */
async function caretBox(page: Page) {
  for (let i = 0; i < 40; i++) {
    const b = await page.getByTestId('caret').boundingBox();
    if (b) return b;
    await page.waitForTimeout(50);
  }
  throw new Error('no caret');
}

/** Click the caret's current position (keeps the selection, focuses the document). */
async function clickCaret(page: Page) {
  const c = await caretBox(page);
  await page.mouse.click(c.x + 1, c.y + c.height / 2);
}

/** Save with Ctrl+S, reload and open the downloaded file. */
async function saveAndReopen(page: Page) {
  const download = page.waitForEvent('download');
  await page.keyboard.press('Control+s');
  const path = await (await download).path();
  await page.reload();
  const chooser = page.waitForEvent('filechooser');
  await page.getByRole('button', { name: 'Mở tệp từ máy' }).click();
  await (await chooser).setFiles(path);
  await expect(page1(page)).toHaveAttribute('data-rendered', 'true');
}

/** Click into the first body line of page 1. */
async function clickFirstLine(page: Page, fx = 0.3) {
  const box = (await page1(page).boundingBox())!;
  await page.mouse.click(box.x + box.width * fx, box.y + box.height * 0.08);
}

test('font picker, size, colour, format painter; kept after save and reopen', async ({ page }) => {
  const errors: string[] = [];
  page.on('pageerror', (e) => errors.push(e.message));
  await newDocument(page);
  await page.keyboard.insertText('Tiêu đề');
  await page.keyboard.press('Enter');
  await page.keyboard.insertText('Nội dung');
  // First line: Liberation Sans 18 pt, red, bold.
  await page.keyboard.press('Control+Home');
  await page.keyboard.press('Shift+End');
  const family = page.getByTestId('font-family');
  await expect(family.locator('option', { hasText: 'Times New Roman → Tinos' })).toHaveCount(1);
  await family.selectOption('Liberation Sans');
  await expect(family).toHaveValue('Liberation Sans');
  const size = page.getByTestId('font-size');
  await size.fill('18');
  await size.press('Enter');
  await expect(size).toHaveValue('18');
  await page.getByTestId('text-color').click();
  await page.getByTestId('text-color-panel').locator('[data-color="C62828"]').click();
  await page.keyboard.press('Control+b');
  await expect(cmd(page, 'format.bold')).toHaveAttribute('aria-pressed', 'true');

  // Format Painter: pick from the title, click the second paragraph's word.
  await page.keyboard.press('Home');
  await cmd(page, 'format.painter').click();
  await expect(cmd(page, 'format.painter')).toHaveAttribute('aria-pressed', 'true');
  await page.keyboard.press('Control+End');
  await clickCaret(page);
  await expect(cmd(page, 'format.painter')).toHaveAttribute('aria-pressed', 'false');
  await expect(cmd(page, 'format.bold')).toHaveAttribute('aria-pressed', 'true');
  await expect(family).toHaveValue('Liberation Sans');
  await expect(size).toHaveValue('18');

  // Sticky painter (double click) stops with Esc.
  await cmd(page, 'format.painter').dblclick();
  await expect(cmd(page, 'format.painter')).toHaveAttribute('aria-pressed', 'true');
  await page.keyboard.press('Escape');
  await expect(cmd(page, 'format.painter')).toHaveAttribute('aria-pressed', 'false');
  mkdirSync(outDir, { recursive: true });
  await page.screenshot({ path: join(outDir, 'web-doc-features.png') });

  await saveAndReopen(page);
  await clickFirstLine(page, 0.2);
  await expect(family).toHaveValue('Liberation Sans');
  await expect(size).toHaveValue('18');
  await expect(cmd(page, 'format.bold')).toHaveAttribute('aria-pressed', 'true');
  expect(errors).toEqual([]);
});

test('table: insert, Tab adds a row, ribbon row insert; kept after reopen', async ({ page }) => {
  await newDocument(page);
  await page.getByRole('tab', { name: 'Chèn' }).click();
  await cmd(page, 'insert.table').click();
  await page.keyboard.insertText('A');
  // 3 × 3 table: Tab through 8 cells, the 9th Tab adds a fourth row.
  for (let i = 0; i < 9; i++) await page.keyboard.press('Tab');
  await page.keyboard.insertText('B');
  await expect(page.getByTestId('status-words')).toHaveText('2 từ');
  // The contextual Table tab: insert a row below and a column.
  await page.getByRole('tab', { name: 'Bảng' }).click();
  await cmd(page, 'table.insertRowBelow').click();
  await page.keyboard.insertText('C');
  await expect(page.getByTestId('status-words')).toHaveText('3 từ');
  await cmd(page, 'table.headerRow').click();
  await expect(cmd(page, 'table.headerRow')).toHaveAttribute('aria-pressed', 'true');
  await page.screenshot({ path: join(outDir, 'web-doc-table.png') });
  await saveAndReopen(page);
  await expect(page.getByTestId('status-words')).toHaveText('3 từ');
  await clickFirstLine(page, 0.15);
  await page.getByRole('tab', { name: 'Bảng' }).click();
  await expect(cmd(page, 'table.headerRow')).toHaveAttribute('aria-pressed', 'true');
});

test('header: double-click to edit, page number, kept after reopen', async ({ page }) => {
  await newDocument(page);
  await page.keyboard.insertText('Thân văn bản');
  const box = (await page1(page).boundingBox())!;
  // Double-click in the top margin opens the header.
  await page.mouse.dblclick(box.x + box.width * 0.4, box.y + 30);
  await expect(page.getByTestId('hf-bar')).toBeVisible();
  await expect(page.getByTestId('hf-kind')).toContainText('Đầu trang');
  await page.keyboard.insertText('Công ty Bluestar — trang ');
  await page.getByTestId('hf-page-number').click();
  const caret = await caretBox(page);
  expect(caret.y).toBeLessThan(box.y + 75);
  await page.screenshot({ path: join(outDir, 'web-doc-header.png') });
  // The checkboxes follow the engine state (controlled).
  await page.getByTestId('hf-first').click();
  await expect(page.getByTestId('hf-first')).toBeChecked();
  await expect(page.getByTestId('hf-kind')).toContainText('Trang đầu');
  await page.getByTestId('hf-first').click();
  await expect(page.getByTestId('hf-first')).not.toBeChecked();
  await expect(page.getByTestId('hf-kind')).toContainText('Mọi trang');
  await page.getByTestId('hf-close').click();
  await expect(page.getByTestId('hf-bar')).toHaveCount(0);
  await clickFirstLine(page);
  await saveAndReopen(page);
  await page.mouse.dblclick(box.x + box.width * 0.4, box.y + 30);
  await expect(page.getByTestId('hf-bar')).toBeVisible();
  await page.keyboard.press('End');
  const end = await caretBox(page);
  // The header text is there: the caret ends far from the left margin.
  expect(end.x - box.x).toBeGreaterThan(200);
  await page.keyboard.press('Escape');
  await expect(page.getByTestId('hf-bar')).toHaveCount(0);
});

test('ruler: drag the left indent and add a tab stop; kept after reopen', async ({ page }) => {
  await newDocument(page);
  await page.keyboard.insertText('Đoạn văn có thụt lề trái');
  const left = page.getByTestId('ruler-left-indent');
  await expect(left).toHaveAttribute('data-twips', '0');
  const b = (await left.boundingBox())!;
  await page.mouse.move(b.x + b.width / 2, b.y + b.height / 2);
  await page.mouse.down();
  await page.mouse.move(b.x + b.width / 2 + 40, b.y + b.height / 2, { steps: 5 });
  await page.mouse.move(b.x + b.width / 2 + 76, b.y + b.height / 2, { steps: 5 });
  await page.mouse.up();
  // ~2 cm at 100 % zoom (76 px ≈ 1140 twips), snapped to 1/8 cm.
  await expect(left).not.toHaveAttribute('data-twips', '0');
  const twips = Number(await left.getAttribute('data-twips'));
  expect(twips).toBeGreaterThan(900);
  expect(twips).toBeLessThan(1300);
  // Tab stop: centre kind, click on the ruler.
  await page.getByTestId('ruler-tab-type').click();
  await expect(page.getByTestId('ruler-tab-type')).toHaveAttribute('data-kind', 'center');
  const track = (await page.getByTestId('ruler').boundingBox())!;
  await page.mouse.click(b.x + 260, track.y + track.height - 6);
  await expect(page.getByTestId('ruler-tab')).toHaveCount(1);
  await expect(page.getByTestId('ruler-tab')).toHaveAttribute('data-kind', 'center');
  await saveAndReopen(page);
  await clickFirstLine(page, 0.5);
  await expect(left).toHaveAttribute('data-twips', String(twips));
  await expect(page.getByTestId('ruler-tab')).toHaveCount(1);
  // Drag the tab stop off the ruler: removed.
  const tb = (await page.getByTestId('ruler-tab').boundingBox())!;
  await page.mouse.move(tb.x + 3, tb.y + 4);
  await page.mouse.down();
  await page.mouse.move(tb.x + 3, tb.y + 80, { steps: 5 });
  await page.mouse.up();
  await expect(page.getByTestId('ruler-tab')).toHaveCount(0);
});

test('page setup dialog and the administrative template', async ({ page }) => {
  await page.goto('/doc');
  await page.getByTestId('new-from-template').click();
  await page.getByTestId('template-nd30').click();
  await expect(page.getByTestId('doc-name')).toHaveText('Văn bản hành chính.docx');
  await expect(page1(page)).toHaveAttribute('data-rendered', 'true');
  await page.screenshot({ path: join(outDir, 'web-doc-template.png') });
  await expect(page.getByTestId('ruler-margin-left')).toHaveAttribute('data-twips', '1701');
  await expect(page.getByTestId('ruler-margin-top')).toHaveAttribute('data-twips', '1134');
  await expect(page.getByTestId('status-page')).toHaveText('Trang 1/1');
  // Landscape A5 with normal margins for the whole document.
  await page.getByRole('tab', { name: 'Bố cục' }).click();
  await cmd(page, 'layout.pageSetup').click();
  await expect(page.getByTestId('page-setup')).toBeVisible();
  await page.getByTestId('page-paper').selectOption('a5');
  await page.getByTestId('page-landscape').check();
  await page.getByTestId('page-margins').selectOption('normal');
  await page.screenshot({ path: join(outDir, 'web-doc-page-setup.png') });
  await page.getByTestId('page-setup-ok').click();
  await expect(page.getByTestId('ruler-margin-left')).toHaveAttribute('data-twips', '1440');
  const box = (await page1(page).boundingBox())!;
  expect(box.width).toBeGreaterThan(box.height);
  await expect(cmd(page, 'layout.orientation')).toHaveAttribute('aria-pressed', 'true');
});
