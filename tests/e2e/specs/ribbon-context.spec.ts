import { expect, test, type Page } from '@playwright/test';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

// CORE-UI-001: contextual ribbon tabs, simplified ribbon, collapse / expand (Ctrl+F1).
// CORE-UI-008: equation / ink / embedded object tabs, auto-hidden ribbon.

const here = dirname(fileURLToPath(import.meta.url));
const equationDeck = join(here, '..', '..', 'corpus', 'slide', 'slide-equation.pptx');

const cmd = (page: Page, id: string) => page.locator(`[data-command="${id}"]`);
const ribbon = (page: Page) => page.getByTestId('ribbon');
const activeTab = (page: Page) => ribbon(page).locator('[role="tab"][aria-selected="true"]');

async function newDocument(page: Page) {
  await page.goto('/doc');
  await page.getByTestId('new-document').click();
  await expect(page.getByTestId('caret')).toBeVisible();
}

test('doc: the table tab follows the caret and is reachable by keyboard', async ({ page }) => {
  const errors: string[] = [];
  page.on('pageerror', (e) => errors.push(e.message));
  await newDocument(page);
  await expect(ribbon(page).locator('[data-context-group]')).toHaveCount(0);

  // Caret in a new table: the Table tab shows under "Công cụ bảng" (typing keeps the tab).
  await page.getByRole('tab', { name: 'Chèn' }).click();
  await cmd(page, 'insert.table').click();
  const band = ribbon(page).locator('[data-context-group="table"]');
  await expect(band).toContainText('Công cụ bảng');
  await expect(band.getByRole('tab', { name: 'Bảng' })).toBeVisible();
  await expect(activeTab(page)).toHaveText('Chèn');
  // One tab list: End moves to the last (contextual) tab and activates it.
  await activeTab(page).focus();
  await page.keyboard.press('End');
  await expect(activeTab(page)).toHaveText('Bảng');
  await expect(cmd(page, 'table.insertRowBelow')).toBeVisible();

  expect(errors).toEqual([]);
});

test('doc: a new SmartArt activates its tab once; it goes with the selection', async ({ page }) => {
  await newDocument(page);
  await page.getByRole('tab', { name: 'Chèn' }).click();
  await cmd(page, 'insert.smartart').click();
  const dialog = page.getByTestId('smartart-dialog');
  await dialog.getByTestId('smartart-text').fill('Tiếp nhận\nXử lý');
  await dialog.getByTestId('smartart-ok').click();
  await expect(dialog).toHaveCount(0);
  const band = ribbon(page).locator('[data-context-group="smartArt"]');
  await expect(band).toContainText('Công cụ sơ đồ');
  await expect(activeTab(page)).toHaveText('Sơ đồ thông minh');
  await expect(cmd(page, 'smartart.edit')).toBeVisible();
  // Back to Insert while the diagram stays selected: not activated again.
  await page.getByRole('tab', { name: 'Chèn' }).click();
  await expect(activeTab(page)).toHaveText('Chèn');
  await expect(band).toBeVisible();
  // Typing after it leaves the diagram: the tab goes, Insert stays.
  await page.keyboard.press('End');
  await page.keyboard.insertText(' x');
  await expect(band).toHaveCount(0);
  await expect(activeTab(page)).toHaveText('Chèn');
});

test('simplified ribbon is one row and remembered; Ctrl+F1 collapses and expands', async ({
  page,
}) => {
  await newDocument(page);
  const panel = ribbon(page).locator('[role="tabpanel"][data-state="active"]');
  const full = (await panel.boundingBox())!.height;

  await page.getByTestId('ribbon-simplified').click();
  await expect(panel).toHaveAttribute('data-simplified', 'true');
  const simple = (await panel.boundingBox())!.height;
  expect(simple).toBeLessThan(full);
  expect(simple).toBeLessThanOrEqual(48);
  // A group's other commands are in its "…" menu.
  await page.getByTestId('ribbon-group-more-font').click();
  await expect(page.getByTestId('ribbon-group-more-font-panel')).toBeVisible();
  await page.keyboard.press('Escape');
  await page.reload();
  await page.getByTestId('new-document').click();
  await expect(ribbon(page).locator('[role="tabpanel"][data-state="active"]')).toHaveAttribute(
    'data-simplified',
    'true',
  );
  await page.getByTestId('ribbon-simplified').click();
  await expect(ribbon(page).locator('[role="tabpanel"][data-state="active"]')).not.toHaveAttribute(
    'data-simplified',
    'true',
  );

  // Ctrl+F1: only the tab strip is left; a tab click shows its commands over the document.
  await page.keyboard.press('Control+F1');
  await expect(ribbon(page)).toHaveAttribute('data-collapsed', 'true');
  await expect(ribbon(page).locator('[role="tabpanel"][data-state="active"]')).toHaveCount(0);
  await page.getByRole('tab', { name: 'Chèn' }).click();
  await expect(page.getByTestId('ribbon-peek')).toBeVisible();
  await page.keyboard.press('Escape');
  await expect(page.getByTestId('ribbon-peek')).toHaveCount(0);
  await page.keyboard.press('Control+F1');
  await expect(ribbon(page)).not.toHaveAttribute('data-collapsed', 'true');
  await expect(ribbon(page).locator('[role="tabpanel"][data-state="active"]')).toBeVisible();
});

test('doc: the Equation tab shows while the caret is at an equation', async ({ page }) => {
  const errors: string[] = [];
  page.on('pageerror', (e) => errors.push(e.message));
  await newDocument(page);
  await page.keyboard.insertText('Ta có ');
  await page.getByRole('tab', { name: 'Chèn' }).click();
  await cmd(page, 'insert.equation').click();
  const dialog = page.getByTestId('equation-dialog');
  await dialog.getByTestId('equation-input').fill('x^2+1');
  await dialog.getByTestId('equation-ok').click();
  await expect(dialog).toHaveCount(0);
  // The caret follows the equation: its tab shows (a caret context: Insert stays active).
  const band = ribbon(page).locator('[data-context-group="equation"]');
  await expect(band).toContainText('Công cụ phương trình');
  await expect(activeTab(page)).toHaveText('Chèn');
  await band.getByRole('tab', { name: 'Phương trình' }).click();
  await expect(cmd(page, 'equation.linear')).toBeEnabled();
  await cmd(page, 'equation.linear').click();
  await expect(cmd(page, 'equation.professional')).toBeEnabled();
  // Typing after it on a new line leaves the equation: the tab goes. The format switch
  // leaves the equation selected, and Enter on a selected equation opens its dialog
  // (CORE-MATH-010): End puts the caret after it first.
  await page.keyboard.press('End');
  await page.keyboard.press('Enter');
  await page.keyboard.insertText('tiếp');
  await expect(band).toHaveCount(0);
  expect(errors).toEqual([]);
});

test('doc: an embedded object activates the Object tab', async ({ page }) => {
  const errors: string[] = [];
  page.on('pageerror', (e) => errors.push(e.message));
  await newDocument(page);
  await page.getByRole('tab', { name: 'Chèn' }).click();
  await cmd(page, 'object.insert').click();
  const dialog = page.getByTestId('object-insert-dialog');
  await dialog.getByTestId('object-insert-ok').click();
  await expect(dialog).toHaveCount(0);
  const band = ribbon(page).locator('[data-context-group="object"]');
  await expect(band).toContainText('Công cụ đối tượng');
  await expect(activeTab(page)).toHaveText('Đối tượng');
  await expect(cmd(page, 'object.saveCopy')).toBeVisible();
  await cmd(page, 'object.edit').click();
  await expect(page.getByTestId('object-edit-dialog')).toBeVisible();
  await page.keyboard.press('Escape');
  await expect(page.getByTestId('object-edit-dialog')).toHaveCount(0);
  expect(errors).toEqual([]);
});

async function openSlideDeck(page: Page, path: string) {
  await page.goto('/slide');
  await page.getByRole('tab', { name: 'Trang đầu' }).click();
  const chooser = page.waitForEvent('filechooser');
  await page.getByRole('button', { name: 'Mở', exact: true }).click();
  await (await chooser).setFiles(path);
  await expect(page.getByTestId('slide-status')).toHaveText(/^Slide 1 \/ \d+$/);
  await expect(page.getByTestId('slide-overlay')).toBeVisible();
}

/** Click a slide point (points, 960 × 540 slide). */
async function clickSlide(page: Page, x: number, y: number) {
  const o = (await page.getByTestId('slide-overlay').boundingBox())!;
  await page.mouse.click(o.x + (o.width * x) / 960, o.y + (o.height * y) / 540);
}

test('slide: equation shape, inking and embedded object tabs', async ({ page }) => {
  const errors: string[] = [];
  page.on('pageerror', (e) => errors.push(e.message));
  await openSlideDeck(page, equationDeck);
  const equation = ribbon(page).locator('[data-context-group="equation"]');
  await expect(equation).toHaveCount(0);
  // The equation text box (4 in, 4 in; 5 × 1 in): its tab appears and activates.
  await clickSlide(page, 468, 324);
  await expect(equation).toContainText('Công cụ phương trình');
  await expect(activeTab(page)).toHaveText('Phương trình');
  await expect(cmd(page, 'arrange.front')).toBeVisible();
  // The title holds no equation.
  await clickSlide(page, 100, 40);
  await expect(equation).toHaveCount(0);

  // Inking from the Draw tab shows the Ink tab (the Draw tab stays active).
  await page.getByRole('tab', { name: 'Vẽ', exact: true }).click();
  await cmd(page, 'draw.pen').click();
  const ink = ribbon(page).locator('[data-context-group="ink"]');
  await expect(ink).toContainText('Công cụ mực');
  await expect(activeTab(page)).toHaveText('Vẽ');
  await ink.getByRole('tab', { name: 'Mực' }).click();
  await expect(cmd(page, 'draw.pen')).toHaveAttribute('aria-pressed', 'true');
  await cmd(page, 'draw.eraserStroke').click();
  await expect(cmd(page, 'draw.eraserStroke')).toHaveAttribute('aria-pressed', 'true');
  await cmd(page, 'draw.select').click();
  await expect(ink).toHaveCount(0);

  // A new embedded workbook is selected: the Object tab activates.
  await page.getByRole('tab', { name: 'Chèn' }).click();
  await cmd(page, 'object.insert').click();
  await page.getByTestId('object-insert-dialog').getByTestId('object-insert-ok').click();
  const object = ribbon(page).locator('[data-context-group="object"]');
  await expect(object).toContainText('Công cụ đối tượng');
  await expect(activeTab(page)).toHaveText('Đối tượng');
  await expect(cmd(page, 'object.edit')).toBeVisible();
  await expect(cmd(page, 'object.updateLinks')).toBeVisible();
  expect(errors).toEqual([]);
});

test('auto-hidden ribbon: shown by a click on the bar or Alt, remembered', async ({ page }) => {
  await newDocument(page);
  await page.getByTestId('ribbon-autohide').click();
  const bar = page.getByTestId('ribbon-autohide-bar');
  await expect(bar).toBeVisible();
  await expect(ribbon(page).getByRole('tab')).toHaveCount(0);
  await expect(ribbon(page)).toHaveAttribute('data-autohide', 'true');
  // The document moves up: the bar is much lower than the ribbon.
  expect((await ribbon(page).boundingBox())!.height).toBeLessThan(20);

  // Click on the bar: the ribbon shows over the document; a command hides it again.
  await bar.click();
  await expect(page.getByTestId('ribbon-revealed')).toBeVisible();
  await page.getByRole('tab', { name: 'Chèn' }).click();
  await expect(page.getByTestId('ribbon-revealed')).toBeVisible();
  await cmd(page, 'insert.table').click();
  await expect(page.getByTestId('ribbon-revealed')).toHaveCount(0);
  await expect(bar).toBeVisible();

  // Alt: the ribbon shows with its KeyTips; Escape twice hides it.
  await page.keyboard.press('Alt');
  await expect(page.getByTestId('ribbon-revealed')).toBeVisible();
  await expect(page.getByTestId('keytips')).toBeVisible();
  await page.keyboard.press('Escape');
  await expect(page.getByTestId('keytips')).toHaveCount(0);
  await page.keyboard.press('Escape');
  await expect(page.getByTestId('ribbon-revealed')).toHaveCount(0);

  // Remembered after a reload; the toggle in the shown ribbon turns the mode off.
  await page.reload();
  await page.getByTestId('new-document').click();
  await expect(page.getByTestId('ribbon-autohide-bar')).toBeVisible();
  await page.getByTestId('ribbon-autohide-bar').click();
  await page.getByTestId('ribbon-autohide').click();
  await expect(page.getByTestId('ribbon-autohide-bar')).toHaveCount(0);
  await expect(ribbon(page).locator('[role="tabpanel"][data-state="active"]')).toBeVisible();
  await page.reload();
  await page.getByTestId('new-document').click();
  await expect(ribbon(page).getByRole('tab', { name: 'Chèn' })).toBeVisible();
});
