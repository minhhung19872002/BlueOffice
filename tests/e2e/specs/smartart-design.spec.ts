import AxeBuilder from '@axe-core/playwright';
import { expect, test, type Page } from '@playwright/test';
import { copyFileSync, mkdirSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

// CORE-SMART-004: the SmartArt Design tab and the text pane in BlueSlide, BlueDoc and BlueSheet,
// as in the office suites they are compatible with: the text pane opens from the arrow on the
// frame (or the Text pane button); Enter adds a shape at the same level, Tab / Shift+Tab demote /
// promote, Backspace on an empty line deletes it; Add Shape (after …), Right to Left, the layout
// gallery with a live preview, Change Colours, Reset Graphic, Convert — each one undo step.
const here = dirname(fileURLToPath(import.meta.url));
const deck = join(here, '..', '..', 'corpus', 'slide', 'slide-text-bullets.pptx');
const outDir = join(here, '..', '..', 'fidelity', 'output');
const cmd = (page: Page, id: string) => page.locator(`[data-command="${id}"]`).first();
const TAB = 'Sơ đồ thông minh';

async function insertProcess(page: Page) {
  await page.getByRole('tab', { name: 'Chèn' }).click();
  await cmd(page, 'insert.smartart').click();
  const dialog = page.getByTestId('smartart-dialog');
  await dialog.getByTestId('smartart-text').fill('Tiếp nhận\nXử lý\nTrả kết quả');
  await dialog.getByTestId('smartart-ok').click();
  await expect(dialog).toHaveCount(0);
  await expect(page.getByRole('tab', { name: TAB })).toBeVisible();
  await page.getByRole('tab', { name: TAB }).click();
}

/** WCAG 2.2 AA (axe) on a part of the page: no violation. */
async function axe(page: Page, include: string) {
  await page.waitForTimeout(150);
  const r = await new AxeBuilder({ page })
    .withTags(['wcag2a', 'wcag2aa', 'wcag21a', 'wcag21aa', 'wcag22aa'])
    .include(include)
    .analyze();
  expect(
    r.violations.map((v) => `${v.id}: ${v.nodes.map((n) => n.target.join(' ')).join(', ')}`),
  ).toEqual([]);
}

/** The pane's lines (text, level). */
async function lines(page: Page): Promise<[string, number][]> {
  const pane = page.getByTestId('smartart-pane');
  const rows = pane.locator('li');
  const out: [string, number][] = [];
  for (let i = 0; i < (await rows.count()); i++) {
    const row = rows.nth(i);
    out.push([
      await row.locator('input').inputValue(),
      Number(await row.getAttribute('data-level')),
    ]);
  }
  return out;
}

/** What the engine has: the edit dialog's outline, layout and colours. */
async function expectDiagram(page: Page, want: { text: string; layout?: string; colors?: string }) {
  await page.getByRole('tab', { name: TAB }).click();
  await cmd(page, 'smartart.edit').click();
  const dialog = page.getByTestId('smartart-dialog');
  await expect(dialog.getByTestId('smartart-text')).toHaveValue(want.text);
  if (want.layout) await expect(dialog.getByTestId(`smartart-layout-${want.layout}`)).toBeChecked();
  if (want.colors) await expect(dialog.getByTestId('smartart-colors')).toHaveValue(want.colors);
  await dialog.getByRole('button', { name: 'Huỷ' }).click();
  await expect(dialog).toHaveCount(0);
}

/**
 * The shared flow: text pane keys, Add Shape After + one undo step, Right to Left, the layout
 * gallery with live preview, colours, Reset Graphic. `openPane` opens the text pane, `undo` takes
 * one step back from the app's document.
 */
async function designFlow(page: Page, openPane: () => Promise<void>, undo: () => Promise<void>) {
  await openPane();
  const pane = page.getByTestId('smartart-pane');
  await expect(pane).toBeVisible();
  expect(await lines(page)).toEqual([
    ['Tiếp nhận', 0],
    ['Xử lý', 0],
    ['Trả kết quả', 0],
  ]);
  // Enter at the end of the last line: a new shape at the same level; type its text.
  const last = page.getByTestId('smartart-pane-line-2');
  await last.click();
  await last.press('End');
  await last.press('Enter');
  const added = page.getByTestId('smartart-pane-line-3');
  await expect(added).toBeFocused();
  await page.keyboard.type('Lưu trữ');
  // Tab demotes it under "Trả kết quả", Shift+Tab promotes it back.
  await added.press('Tab');
  await expect(pane.locator('li').nth(3)).toHaveAttribute('data-level', '1');
  await added.press('Shift+Tab');
  await expect(pane.locator('li').nth(3)).toHaveAttribute('data-level', '0');
  // Enter on it, then Backspace on the empty line: the empty shape is deleted.
  await added.press('End');
  await added.press('Enter');
  await expect(pane.locator('li')).toHaveCount(5);
  await page.getByTestId('smartart-pane-line-4').press('Backspace');
  await expect(pane.locator('li')).toHaveCount(4);
  await expect(added).toBeFocused();
  // Closing the pane commits the typing: the engine has four shapes.
  await page.getByTestId('smartart-pane-close').click();
  await expect(pane).toHaveCount(0);
  await expectDiagram(page, {
    text: 'Tiếp nhận\nXử lý\nTrả kết quả\nLưu trữ',
    layout: 'process',
  });

  // Add Shape › After, with the first shape current (its line in the pane has the caret).
  await openPane();
  await page.getByTestId('smartart-pane-line-0').click();
  await page.getByTestId('smartart-add-shape').click();
  await page.getByTestId('smartart-addAfter').click();
  await expect(pane.locator('li')).toHaveCount(5);
  expect((await lines(page))[1]).toEqual(['', 0]);
  await page.getByTestId('smartart-pane-close').click();
  // One undo step takes it away.
  await undo();
  await expectDiagram(page, { text: 'Tiếp nhận\nXử lý\nTrả kết quả\nLưu trữ' });

  // Right to Left: a toggle.
  await page.getByRole('tab', { name: TAB }).click();
  await cmd(page, 'smartart.rtl').click();
  await expect(cmd(page, 'smartart.rtl')).toHaveAttribute('aria-pressed', 'true');

  // Layouts gallery: pointing at a layout previews it on the diagram; clicking applies it.
  await page.getByTestId('smartart-layouts').click();
  await page.getByTestId('smartart-gallery-cycle').hover();
  await expect(page.getByTestId('smartart-live-preview')).toBeVisible();
  await page.getByTestId('smartart-gallery-cycle').click();
  await expect(page.getByTestId('smartart-live-preview')).toHaveCount(0);
  // Change Colours, then Reset Graphic back to the default colours.
  await page.getByTestId('smartart-colors-menu').click();
  await page.getByTestId('smartart-colors-list-green').click();
  await expectDiagram(page, {
    text: 'Tiếp nhận\nXử lý\nTrả kết quả\nLưu trữ',
    layout: 'cycle',
    colors: 'green',
  });
  await page.getByRole('tab', { name: TAB }).click();
  await cmd(page, 'smartart.reset').click();
  await expectDiagram(page, {
    text: 'Tiếp nhận\nXử lý\nTrả kết quả\nLưu trữ',
    layout: 'cycle',
    colors: 'colorful',
  });
  // The direction stayed.
  await page.getByRole('tab', { name: TAB }).click();
  await expect(cmd(page, 'smartart.rtl')).toHaveAttribute('aria-pressed', 'true');
}

test('BlueSlide: text pane, Design tab and Convert to text', async ({ page }) => {
  const errors: string[] = [];
  page.on('pageerror', (e) => errors.push(e.message));
  await page.goto('/slide');
  const chooser = page.waitForEvent('filechooser');
  await page.getByRole('button', { name: 'Mở', exact: true }).click();
  await (await chooser).setFiles(deck);
  await expect(page.getByTestId('slide-overlay')).toBeVisible();
  await page.getByRole('button', { name: '+ Thêm slide' }).click();
  await insertProcess(page);
  // Accessibility of the new pieces: the Design tab, the text pane, the layout gallery.
  await axe(page, '[role="tabpanel"]');
  await page.getByTestId('smartart-pane-toggle').click();
  await axe(page, '[data-testid="smartart-pane"]');
  await page.getByTestId('smartart-pane-close').click();
  await page.getByTestId('smartart-layouts').click();
  await axe(page, '[data-testid="smartart-layouts-panel"]');
  await page.keyboard.press('Escape');
  await expect(page.getByTestId('smartart-layouts-panel')).toHaveCount(0);
  await designFlow(
    page,
    // The arrow on the frame.
    () => page.getByTestId('smartart-pane-toggle').click(),
    async () => {
      await page.getByTestId('slide-overlay').focus();
      await page.keyboard.press('Control+z');
    },
  );
  // Saved with the direction; Convert › to text then leaves a bulleted text box.
  const download = page.waitForEvent('download');
  await page.keyboard.press('Control+s');
  const saved = await (await download).path();
  mkdirSync(outDir, { recursive: true });
  copyFileSync(saved, join(outDir, 'web-smartart-design.pptx'));
  await page.getByRole('tab', { name: TAB }).click();
  await cmd(page, 'smartart.toText').click();
  await expect(page.getByRole('tab', { name: TAB })).toHaveCount(0);
  expect(errors).toEqual([]);
});

test('BlueDoc: text pane and Design tab', async ({ page }) => {
  const errors: string[] = [];
  page.on('pageerror', (e) => errors.push(e.message));
  await page.goto('/doc');
  await page.getByTestId('new-document').click();
  await expect(page.getByTestId('caret')).toBeVisible();
  await insertProcess(page);
  await designFlow(
    page,
    async () => {
      await page.getByRole('tab', { name: TAB }).click();
      await cmd(page, 'smartart.textPane').click();
    },
    async () => {
      await page.locator('.bo-page[data-page="1"]').first().focus();
      await page.keyboard.press('Control+z');
    },
  );
  const download = page.waitForEvent('download');
  await page.keyboard.press('Control+s');
  const saved = await (await download).path();
  mkdirSync(outDir, { recursive: true });
  copyFileSync(saved, join(outDir, 'web-smartart-design.docx'));
  // Convert to shapes ends the diagram.
  await page.getByRole('tab', { name: TAB }).click();
  await cmd(page, 'smartart.toShapes').click();
  await expect(page.getByRole('tab', { name: TAB })).toHaveCount(0);
  expect(errors).toEqual([]);
});

test('BlueSheet: text pane and Design tab', async ({ page }) => {
  const errors: string[] = [];
  page.on('pageerror', (e) => errors.push(e.message));
  await page.goto('/sheet');
  await expect(page.getByTestId('sheet-canvas')).toHaveAttribute('data-rendered', /\d+/);
  await insertProcess(page);
  await expect(page.getByTestId('smartart-selection')).toBeVisible();
  await designFlow(
    page,
    () => page.getByTestId('smartart-pane-toggle').click(),
    async () => {
      await page.getByLabel('Ô đang nhập').focus();
      await page.keyboard.press('Control+z');
    },
  );
  await page.getByLabel('Ô đang nhập').focus();
  const download = page.waitForEvent('download');
  await page.keyboard.press('Control+s');
  const saved = await (await download).path();
  mkdirSync(outDir, { recursive: true });
  copyFileSync(saved, join(outDir, 'web-smartart-design.xlsx'));
  await page.getByRole('tab', { name: TAB }).click();
  await cmd(page, 'smartart.toShapes').click();
  await expect(page.getByTestId('smartart-selection')).toHaveCount(0);
  expect(errors).toEqual([]);
});
