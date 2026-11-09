import { expect, test, type Page } from '@playwright/test';

// DOC-VIEW-001: read mode, web layout, outline, draft, focus, navigation pane, formatting marks.

const cmd = (page: Page, id: string) => page.locator(`[data-command="${id}"]`);
const page1 = (page: Page) => page.locator('.bo-page[data-page="1"]');

async function viewTab(page: Page) {
  await page.getByRole('tab', { name: 'Xem', exact: true }).click();
}

/** Two Heading 1 sections, each with one body paragraph. */
async function headedDocument(page: Page) {
  await page.goto('/doc');
  await page.getByTestId('new-document').click();
  await expect(page.getByTestId('caret')).toBeVisible();
  await page.keyboard.insertText('Mở đầu');
  await page.keyboard.press('Control+Alt+1');
  await page.keyboard.press('Enter');
  await page.keyboard.insertText('Nội dung một');
  await page.keyboard.press('Enter');
  await page.keyboard.insertText('Kết luận');
  await page.keyboard.press('Control+Alt+1');
  await page.keyboard.press('Enter');
  await page.keyboard.insertText('Nội dung hai');
  await expect(page1(page)).toHaveAttribute('data-rendered', 'true');
}

test('navigation pane: heading tree, find results, page thumbnails', async ({ page }) => {
  const errors: string[] = [];
  page.on('pageerror', (e) => errors.push(e.message));
  await headedDocument(page);
  await viewTab(page);
  await cmd(page, 'view.navigationPane').click();
  const pane = page.getByTestId('navigation-pane');
  await expect(pane).toBeVisible();
  const heads = pane.getByTestId('nav-heading');
  await expect(heads).toHaveCount(2);
  await expect(heads.nth(0)).toContainText('Mở đầu');
  // The caret is in the second section.
  await expect(heads.nth(1)).toHaveAttribute('aria-selected', 'true');
  await heads.nth(0).getByRole('button', { name: 'Mở đầu' }).click();
  await expect(heads.nth(0)).toHaveAttribute('aria-selected', 'true');
  // Drag "Kết luận" before "Mở đầu": its body text moves with it.
  await heads.nth(1).dragTo(heads.nth(0));
  await expect(heads.nth(0)).toContainText('Kết luận');
  await page.keyboard.press('Control+z');
  await expect(heads.nth(0)).toContainText('Mở đầu');
  // Results: matches with context, highlighted on the page.
  await page.getByTestId('nav-tab-results').click();
  await page.getByTestId('nav-search').fill('Nội dung');
  await expect(pane.getByTestId('nav-result')).toHaveCount(2);
  await expect(page.getByTestId('find-highlight')).toHaveCount(2);
  await pane.getByTestId('nav-result').nth(1).click();
  // Pages: one thumbnail per page.
  await page.getByTestId('nav-tab-pages').click();
  await expect(pane.getByTestId('nav-thumbnail')).toHaveCount(1);
  await pane.getByTestId('nav-thumbnail').first().click();
  await page.getByTestId('navigation-close').click();
  await expect(pane).toBeHidden();
  expect(errors).toEqual([]);
});

test('formatting marks toggle with Ctrl+Shift+8', async ({ page }) => {
  await headedDocument(page);
  await expect(page1(page)).toHaveAttribute('data-marks', '0');
  await page.keyboard.press('Control+Shift+8');
  await expect(page1(page)).not.toHaveAttribute('data-marks', '0');
  await viewTab(page);
  await expect(cmd(page, 'view.formattingMarks')).toHaveAttribute('aria-pressed', 'true');
  await cmd(page, 'view.formattingMarks').click();
  await expect(page1(page)).toHaveAttribute('data-marks', '0');
});

test('outline view: levels, move with Alt+Shift+Up, promote, drag, undo', async ({ page }) => {
  await headedDocument(page);
  await viewTab(page);
  await cmd(page, 'view.outline').click();
  const outline = page.getByTestId('outline-view');
  await expect(outline).toBeVisible();
  const rows = outline.getByTestId('outline-item');
  await expect(rows).toHaveCount(4);
  await expect(rows.nth(0)).toHaveAttribute('data-level', '0');
  await expect(rows.nth(1)).toHaveAttribute('data-level', 'body');
  // Move "Kết luận" (with its text) above "Mở đầu".
  await rows.nth(2).click();
  await page.getByTestId('outline-list').focus();
  await page.keyboard.press('Alt+Shift+ArrowUp');
  await expect(rows.nth(0)).toContainText('Kết luận');
  await expect(rows.nth(1)).toContainText('Nội dung hai');
  await page.keyboard.press('Control+z');
  await expect(rows.nth(0)).toContainText('Mở đầu');
  // Tab lowers the selected heading to level 2.
  await rows.nth(2).click();
  await page.getByTestId('outline-list').focus();
  await page.keyboard.press('Tab');
  await expect(rows.nth(2)).toHaveAttribute('data-level', '1');
  await page.keyboard.press('Shift+Tab');
  await expect(rows.nth(2)).toHaveAttribute('data-level', '0');
  // Show level 1 only.
  await page.getByTestId('outline-show-level').selectOption('1');
  await expect(rows).toHaveCount(2);
  await page.getByTestId('outline-show-level').selectOption('');
  // Drag "Mở đầu" to the end.
  await rows.nth(0).dragTo(outline.getByTestId('outline-drop-end'));
  await expect(rows.nth(0)).toContainText('Kết luận');
  await expect(rows.nth(3)).toContainText('Nội dung một');
  await page.getByTestId('outline-close').click();
  await expect(outline).toBeHidden();
  await expect(page1(page)).toHaveAttribute('data-rendered', 'true');
});

test('web layout reflows to the window; draft hides page decorations', async ({ page }) => {
  await headedDocument(page);
  const printWidth = (await page1(page).boundingBox())!.width;
  await viewTab(page);
  await cmd(page, 'view.webLayout').click();
  await expect(page.locator('main[data-view="web"]')).toBeVisible();
  const host = (await page.getByTestId('document-view').boundingBox())!;
  await expect
    .poll(async () => (await page1(page).boundingBox())?.width ?? 0)
    .toBeGreaterThan(host.width - 40);
  expect(host.width - 40).toBeGreaterThan(printWidth);
  // Typing still edits the document.
  await page.keyboard.insertText(' thêm');
  await expect(page.getByTestId('status-modified')).toBeVisible();
  await cmd(page, 'view.draft').click();
  await expect
    .poll(async () => (await page1(page).boundingBox())?.width ?? 0)
    .toBeLessThan(printWidth + 2);
  await expect(page1(page)).toHaveCSS('box-shadow', 'none');
  await cmd(page, 'view.printLayout').click();
  await expect(page1(page)).not.toHaveCSS('box-shadow', 'none');
});

test('read mode and focus mode', async ({ page }) => {
  await headedDocument(page);
  await viewTab(page);
  await cmd(page, 'view.readMode').click();
  const read = page.getByTestId('read-mode');
  await expect(read).toBeVisible();
  await expect(read.getByTestId('read-page').first()).toBeVisible();
  await page.keyboard.press('PageDown');
  await page.keyboard.press('Escape');
  await expect(read).toBeHidden();
  await cmd(page, 'view.focus').click();
  await expect(page.getByTestId('ribbon')).toBeHidden();
  await expect(page.getByTestId('status-page')).toBeHidden();
  await expect(page.getByTestId('focus-exit')).toBeVisible();
  await page.keyboard.press('Escape');
  await expect(page.getByTestId('ribbon')).toBeVisible();
  await expect(page.getByTestId('status-page')).toBeVisible();
});
