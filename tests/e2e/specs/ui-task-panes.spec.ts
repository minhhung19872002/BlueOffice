import { expect, test, type Page } from '@playwright/test';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const here = dirname(fileURLToPath(import.meta.url));
const deck = join(here, '..', '..', 'corpus', 'slide', 'slide-text-bullets.pptx');

// CORE-UI-006: task panes dock left / right, float, tabs, splitter, F6 / Escape, layout remembered.

const cmd = (page: Page, id: string) => page.locator(`[data-command="${id}"]`);

async function newDoc(page: Page) {
  await page.goto('/doc');
  await page.getByTestId('new-document').click();
  await expect(page.getByTestId('caret')).toBeVisible();
  await page.keyboard.insertText('Đoạn một');
  await expect(page.locator('.bo-page[data-page="1"]')).toHaveAttribute('data-rendered', 'true');
}

async function viewTab(page: Page) {
  await page.getByRole('tab', { name: 'Xem', exact: true }).click();
}

test('BlueDoc styles pane: apply, float, dock back, tabs, keyboard, remembered', async ({
  page,
}) => {
  const errors: string[] = [];
  page.on('pageerror', (e) => errors.push(e.message));
  await newDoc(page);
  await viewTab(page);
  await cmd(page, 'view.stylesPane').click();

  // Docked right by default, as a labelled complementary region.
  const right = page.getByTestId('taskpane-dock-right');
  await expect(right).toBeVisible();
  await expect(right).toHaveAttribute('role', 'complementary');
  await expect(right).toHaveAttribute('aria-label', 'Kiểu');
  const items = right.getByTestId('style-item');
  await expect(items.first()).toBeVisible();
  await expect(right.locator('[data-testid="style-item"][data-style-id="Normal"]')).toHaveAttribute(
    'aria-pressed',
    'true',
  );
  // Click applies the style to the caret paragraph.
  await right.locator('[data-testid="style-item"][data-style-id="Heading1"]').click();
  await expect(
    right.locator('[data-testid="style-item"][data-style-id="Heading1"]'),
  ).toHaveAttribute('aria-pressed', 'true');

  // The document view leaves room for the dock.
  const view = page.getByTestId('document-view');
  const viewBox = await view.boundingBox();
  const dockBox = await right.boundingBox();
  expect(viewBox!.x + viewBox!.width).toBeLessThanOrEqual(dockBox!.x + 1);

  // Float it and drag it by its title bar.
  await right.getByTestId('taskpane-to-float').click();
  await expect(right).toHaveCount(0);
  const win = page.getByTestId('taskpane-float-styles');
  await expect(win).toBeVisible();
  const before = (await win.boundingBox())!;
  const handle = win.getByTestId('taskpane-handle');
  const hb = (await handle.boundingBox())!;
  await page.mouse.move(hb.x + 20, hb.y + hb.height / 2);
  await page.mouse.down();
  await page.mouse.move(hb.x - 180, hb.y + hb.height / 2 + 60, { steps: 6 });
  await page.mouse.up();
  const after = (await win.boundingBox())!;
  expect(Math.round(after.x)).toBe(Math.round(before.x) - 200);
  expect(Math.round(after.y)).toBe(Math.round(before.y) + 60);

  // Dock it back on the left; the navigation pane joins the same dock as a tab.
  await win.getByTestId('taskpane-to-left').click();
  const left = page.getByTestId('taskpane-dock-left');
  await expect(left).toHaveAttribute('data-active', 'styles');
  await cmd(page, 'view.navigationPane').click();
  await expect(left.getByRole('tablist', { name: 'Các ngăn' }).getByRole('tab')).toHaveCount(2);
  await expect(left).toHaveAttribute('data-active', 'navigation');
  await left.getByTestId('taskpane-tab-styles').click();
  await expect(left).toHaveAttribute('data-active', 'styles');

  // Splitter: keyboard resize.
  const splitter = left.getByTestId('taskpane-splitter-left');
  await splitter.focus();
  await page.keyboard.press('Shift+ArrowRight');
  await expect(splitter).toHaveAttribute('aria-valuenow', '364');

  // F6 moves into the pane, Escape gives the keyboard back to the document.
  await page.locator('.bo-page[data-page="1"]').click();
  await page.keyboard.press('F6');
  await expect(left).toBeFocused();
  await page.keyboard.press('Escape');
  await expect(left).not.toBeFocused();
  await page.keyboard.insertText(' tiếp');
  await expect(page.getByTestId('status-words')).toHaveText('3 từ');

  // Reload: the pane opens where it was left, with its width.
  await page.reload();
  await page.getByTestId('new-document').click();
  await expect(page.getByTestId('caret')).toBeVisible();
  await viewTab(page);
  await cmd(page, 'view.stylesPane').click();
  const again = page.getByTestId('taskpane-dock-left');
  await expect(again).toHaveAttribute('data-active', 'styles');
  await expect(again.getByTestId('taskpane-splitter-left')).toHaveAttribute('aria-valuenow', '364');
  await again.getByTestId('taskpane-close').click();
  await expect(page.getByTestId('taskpane-dock-left')).toHaveCount(0);
  expect(errors).toEqual([]);
});

test('BlueSheet comments and BlueSlide selection panes use the shared host', async ({ page }) => {
  const errors: string[] = [];
  page.on('pageerror', (e) => errors.push(e.message));
  await page.goto('/sheet');
  await expect(page.getByTestId('sheet-canvas')).toHaveAttribute('data-rendered', /\d+/);
  await page.getByRole('tab', { name: 'Chèn', exact: true }).click();
  await cmd(page, 'review.comments').click();
  const sheetDock = page.getByTestId('taskpane-dock-right');
  await expect(sheetDock.getByTestId('comment-threads')).toBeVisible();
  await sheetDock.getByTestId('taskpane-to-float').click();
  await expect(page.getByTestId('taskpane-float-comments')).toBeVisible();
  await page.reload();
  await expect(page.getByTestId('sheet-canvas')).toHaveAttribute('data-rendered', /\d+/);
  await page.getByRole('tab', { name: 'Chèn', exact: true }).click();
  await cmd(page, 'review.comments').click();
  await expect(page.getByTestId('taskpane-float-comments')).toBeVisible();

  await page.goto('/slide');
  const chooser = page.waitForEvent('filechooser');
  await page.getByRole('button', { name: 'Mở', exact: true }).click();
  await (await chooser).setFiles(deck);
  await expect(page.getByTestId('slide-status')).toHaveText(/^Slide 1 \/ \d+$/);
  await page.getByRole('tab', { name: 'Sắp xếp' }).click();
  await cmd(page, 'view.selectionPane').click();
  const slideDock = page.getByTestId('taskpane-dock-right');
  const objects = slideDock.getByTestId('selection-item');
  await expect(objects.first()).toBeVisible();
  await objects.first().click();
  await expect(objects.first()).toHaveAttribute('aria-selected', 'true');
  expect(errors).toEqual([]);
});
