import { expect, test, type Page } from '@playwright/test';

// CORE-UI-001: contextual ribbon tabs, simplified ribbon, collapse / expand (Ctrl+F1).

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
  await expect(activeTab(page)).toHaveText('SmartArt');
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
