import { expect, test, type Page } from '@playwright/test';

// SHEET-VIEW-001: show formulas, page break preview (drag a break, undo), page layout
// view, custom views and temporary sheet views.

async function goTo(page: Page, ref: string) {
  const box = page.getByLabel('Hộp tên');
  await box.click();
  await box.fill(ref);
  await box.press('Enter');
  await expect(page.getByLabel('Ô đang nhập')).toBeFocused();
}

const viewTab = (page: Page) => page.getByRole('tab', { name: 'Xem', exact: true });
const cmd = (page: Page, id: string) => page.locator(`[data-command="${id}"]`);

test('BlueSheet: workbook views — formulas, page breaks, page layout, custom views', async ({
  page,
}) => {
  const errors: string[] = [];
  page.on('pageerror', (e) => errors.push(e.message));
  await page.goto('/sheet');
  await expect(page.getByTestId('sheet-canvas')).toHaveAttribute('data-rendered', /\d+/);

  // A1:A10 = 1…10, A11 = their sum.
  await goTo(page, 'A1');
  for (let i = 1; i <= 10; i++) {
    await page.keyboard.type(String(i));
    await page.keyboard.press('Enter');
  }
  await page.keyboard.type('=SUM(A1:A10)');
  await page.keyboard.press('Enter');

  // Show formulas: ribbon toggle and Ctrl+`.
  await viewTab(page).click();
  await cmd(page, 'view.showFormulas').click();
  await expect(cmd(page, 'view.showFormulas')).toHaveAttribute('aria-pressed', 'true');
  await page.getByLabel('Ô đang nhập').focus();
  await page.keyboard.press('Control+`');
  await expect(cmd(page, 'view.showFormulas')).toHaveAttribute('aria-pressed', 'false');

  // A manual break before row 4 (Page layout tab), then page break preview.
  await goTo(page, 'A4');
  await page.getByRole('tab', { name: 'Bố cục trang', exact: true }).click();
  await cmd(page, 'page.insertBreak').click();
  await viewTab(page).click();
  await cmd(page, 'view.pageBreakPreview').click();
  await expect(cmd(page, 'view.pageBreakPreview')).toHaveAttribute('aria-pressed', 'true');
  const overlay = page.getByTestId('page-break-preview');
  await expect(overlay).toBeVisible();
  await expect(page.getByTestId('page-label-1')).toHaveText('Trang 1');
  await expect(page.getByTestId('page-label-2')).toHaveText('Trang 2');
  // Break before row 4 = index 3, drawn solid (manual).
  const brk = page.getByTestId('page-break-row-3');
  await expect(brk).toHaveAttribute('data-manual', 'true');

  // Drag it two rows down (rows are 20 px high at 100 %): the break moves to row 6.
  const handle = page.getByTestId('page-break-handle-row-3');
  const box = await handle.boundingBox();
  expect(box).not.toBeNull();
  if (box) {
    const x = box.x + Math.min(40, box.width / 2);
    const y = box.y + box.height / 2;
    await page.mouse.move(x, y);
    await page.mouse.down();
    await page.mouse.move(x, y + 25, { steps: 4 });
    await page.mouse.move(x, y + 42, { steps: 4 });
    await page.mouse.up();
  }
  await expect(page.getByTestId('page-break-row-5')).toHaveAttribute('data-manual', 'true');
  await expect(page.getByTestId('page-break-row-3')).toHaveCount(0);
  // One undo step.
  await page.getByLabel('Ô đang nhập').focus();
  await page.keyboard.press('Control+z');
  await expect(page.getByTestId('page-break-row-3')).toHaveAttribute('data-manual', 'true');

  // Page layout view: two printed pages; the header area opens Page Setup.
  await cmd(page, 'view.pageLayout').click();
  const layout = page.getByTestId('page-layout-view');
  await expect(layout).toBeVisible();
  await expect(page.getByTestId('page-layout-page-0')).toBeVisible();
  await expect(page.getByTestId('page-layout-page-1')).toBeAttached();
  await expect(page.getByTestId('sheet-canvas')).toBeHidden();
  await page.getByTestId('page-layout-header-0').click();
  const setup = page.getByRole('dialog');
  await expect(setup).toBeVisible();
  await page.keyboard.press('Escape');
  await expect(setup).toBeHidden();

  // Back to normal; save a custom view, change the view type, show the saved view again.
  await cmd(page, 'view.normal').click();
  await expect(page.getByTestId('sheet-canvas')).toBeVisible();
  await goTo(page, 'A5');
  await cmd(page, 'view.customViews').click();
  const dlg = page.getByTestId('custom-views');
  await expect(dlg).toBeVisible();
  await dlg.getByTestId('custom-view-name').fill('Bản in');
  await dlg.getByTestId('custom-view-add').click();
  await expect(dlg.getByTestId('custom-view-item-0')).toContainText('Bản in');
  await page.keyboard.press('Escape');
  await expect(dlg).toBeHidden();
  await cmd(page, 'view.pageBreakPreview').click();
  await expect(page.getByTestId('page-break-preview')).toBeVisible();
  await cmd(page, 'view.customViews').click();
  await dlg.getByTestId('custom-view-show-0').click();
  await expect(dlg).toBeHidden();
  await expect(cmd(page, 'view.normal')).toHaveAttribute('aria-pressed', 'true');
  await expect(page.getByTestId('page-break-preview')).toHaveCount(0);
  await expect(page.getByLabel('Hộp tên')).toHaveValue('A5');

  // Temporary view: kept for the session, shown again from another cell.
  await cmd(page, 'view.tempViews').click();
  const temp = page.getByTestId('temp-views');
  await temp.getByTestId('temp-view-name').fill('Của tôi');
  await temp.getByTestId('temp-view-keep').click();
  await expect(temp.getByTestId('temp-view-item-0')).toContainText('Của tôi');
  await page.keyboard.press('Escape');
  await goTo(page, 'C9');
  await cmd(page, 'view.tempViews').click();
  await temp.getByTestId('temp-view-show-0').click();
  await expect(page.getByLabel('Hộp tên')).toHaveValue('A5');

  expect(errors).toEqual([]);
});
