import { expect, test, type Page } from '@playwright/test';

// SHEET-DATA-002 / SHEET-VIEW-001 / SHEET-AUDIT-001 through the real UI: the outline bar
// beside the grid (level buttons, −/+ at the summary row, one undo step each, Ctrl+8),
// Data › Show / Hide Detail, the three-step Text to Columns wizard, typing in Page Layout
// view, dragging the print area edge in Page Break Preview. Expected values are worked out
// by hand from the data typed in each test.

async function goTo(page: Page, ref: string) {
  const box = page.getByLabel('Hộp tên');
  await box.click();
  await box.fill(ref);
  await box.press('Enter');
  await expect(page.getByLabel('Ô đang nhập')).toBeFocused();
}

async function openSheet(page: Page) {
  const errors: string[] = [];
  page.on('pageerror', (e) => errors.push(e.message));
  await page.goto('/sheet');
  await expect(page.getByTestId('sheet-canvas')).toHaveAttribute('data-rendered', /\d+/);
  return errors;
}

async function typeColumn(page: Page, ref: string, values: string[]) {
  await goTo(page, ref);
  for (const v of values) {
    await page.keyboard.type(v);
    await page.keyboard.press('Enter');
  }
}

/** Down from A1: the first row not hidden below it. */
async function belowA1(page: Page) {
  await goTo(page, 'A1');
  await page.keyboard.press('ArrowDown');
  return page.getByLabel('Hộp tên').inputValue();
}

const cmd = (page: Page, id: string) => page.locator(`[data-command="${id}"]`);

test('outline bar: level buttons, −/+ on the summary row, Show / Hide Detail, Ctrl+8', async ({
  page,
}) => {
  const errors = await openSheet(page);
  // A1 caption, A2:A4 detail, A5 = their sum (the summary row below the detail).
  await typeColumn(page, 'A1', ['Nhóm', '1', '2', '3', '=SUM(A2:A4)']);
  await expect(page.getByTestId('outline-bar-rows')).toHaveCount(0);
  await goTo(page, 'A2:A4');
  await page.keyboard.press('Alt+Shift+ArrowRight');

  // The bar appears left of the row headers: levels 1 and 2, − on row 5.
  const bar = page.getByTestId('outline-bar-rows');
  await expect(bar).toBeVisible();
  await expect(page.getByTestId('outline-row-level-1')).toHaveText('1');
  await expect(page.getByTestId('outline-row-level-2')).toHaveText('2');
  const toggle = page.getByTestId('outline-row-toggle-4');
  await expect(toggle).toHaveText('−');
  await expect(toggle).toHaveAttribute('aria-expanded', 'true');

  // − hides rows 2–4: Down from A1 lands on A5; the button turns into +.
  await toggle.click();
  await expect(toggle).toHaveText('+');
  expect(await belowA1(page)).toBe('A5');
  // One undo step brings the detail back.
  await page.keyboard.press('Control+z');
  await expect(toggle).toHaveText('−');
  expect(await belowA1(page)).toBe('A2');

  // Level 1 shows the summaries only; level 2 everything.
  await page.getByTestId('outline-row-level-1').click();
  await expect(toggle).toHaveText('+');
  expect(await belowA1(page)).toBe('A5');
  await page.getByTestId('outline-row-level-2').click();
  await expect(toggle).toHaveText('−');

  // Data › Hide Detail / Show Detail with the summary cell active.
  await page.getByRole('tab', { name: 'Dữ liệu' }).click();
  await goTo(page, 'A5');
  await cmd(page, 'data.hideDetail').click();
  await expect(toggle).toHaveText('+');
  await goTo(page, 'A5');
  await cmd(page, 'data.showDetail').click();
  await expect(toggle).toHaveText('−');

  // Ctrl+8 hides the outline symbols and shows them again.
  await goTo(page, 'A1');
  await page.keyboard.press('Control+8');
  await expect(bar).toHaveCount(0);
  await page.keyboard.press('Control+8');
  await expect(page.getByTestId('outline-bar-rows')).toBeVisible();
  expect(errors).toEqual([]);
});

test('Text to Columns walks through three steps', async ({ page }) => {
  const errors = await openSheet(page);
  await typeColumn(page, 'A1', ['An,7,15/03/2024', 'Bình,9,01/12/2023']);
  await goTo(page, 'A1:A2');
  await page.getByRole('tab', { name: 'Dữ liệu' }).click();
  await cmd(page, 'data.textToColumns').click();
  const dlg = page.getByTestId('split-dialog');
  await expect(dlg).toBeVisible();
  // Step 1: separators by default, Back disabled.
  await expect(dlg.getByTestId('split-delimited')).toBeChecked();
  await expect(dlg.getByTestId('split-back')).toBeDisabled();
  await dlg.getByTestId('split-next').click();
  // Step 2: Tab ticked by default; choose the comma instead.
  await expect(dlg.getByTestId('split-tab')).toBeChecked();
  await dlg.getByTestId('split-tab').uncheck();
  await dlg.getByTestId('split-comma').check();
  await expect(dlg.getByTestId('split-preview')).toContainText('Bình');
  await dlg.getByTestId('split-next').click();
  // Step 3: third column read as a day/month/year date; finish.
  await expect(dlg.getByTestId('split-next')).toBeDisabled();
  await dlg.getByTestId('split-col-2').click();
  await dlg.getByTestId('split-fmt-date').check();
  await expect(dlg.getByTestId('split-col-2')).toContainText('DMY');
  await dlg.getByTestId('split-ok').click();
  await expect(dlg).toBeHidden();
  await goTo(page, 'B2');
  await expect(page.getByLabel('Thanh công thức')).toHaveValue('9');
  await goTo(page, 'A2');
  await expect(page.getByLabel('Thanh công thức')).toHaveValue('Bình');
  // 01/12/2023 read day first: 1 December 2023.
  await goTo(page, 'C2');
  await expect(page.getByLabel('Thanh công thức')).toHaveValue(/^0?1\/12\/2023$/);
  // One undo step takes the whole split back.
  await page.keyboard.press('Control+z');
  await goTo(page, 'A2');
  await expect(page.getByLabel('Thanh công thức')).toHaveValue('Bình,9,01/12/2023');
  expect(errors).toEqual([]);
});

test('typing in Page Layout view, dragging the print area edge in Page Break Preview', async ({
  page,
}) => {
  const errors = await openSheet(page);
  // A1:N1: fourteen columns, wider than one portrait page — two pages side by side.
  await goTo(page, 'A1');
  for (let i = 1; i <= 14; i++) {
    await page.keyboard.type(String(i));
    await page.keyboard.press('Tab');
  }
  const view = page.getByRole('tab', { name: 'Xem', exact: true });
  await view.click();
  await cmd(page, 'view.pageLayout').click();
  const layout = page.getByTestId('page-layout-view');
  await expect(layout).toBeVisible();
  // Click B1 on the first page (left margin 0.7 in = 67 px, top 0.75 in = 72 px at 100 %,
  // columns 64 px wide), type, Enter: the cell takes the value.
  const pageBox = await page.getByTestId('page-layout-page-0').boundingBox();
  expect(pageBox).not.toBeNull();
  if (pageBox) await page.mouse.click(pageBox.x + 67 + 64 + 30, pageBox.y + 72 + 8);
  await expect(page.getByLabel('Hộp tên')).toHaveValue('B1');
  // The clicked cell is outlined on its page.
  await expect(page.getByTestId('page-layout-active-0')).toBeVisible();
  const typedAt = await page.getByLabel('Hộp tên').inputValue();
  await page.keyboard.type('777');
  await expect(page.getByLabel('Thanh công thức')).toHaveValue('777');
  await page.keyboard.press('Enter');
  // The selection moved down; the pages keep the keyboard.
  await expect(page.getByLabel('Hộp tên')).not.toHaveValue(typedAt);
  await expect(layout).toBeFocused();
  await cmd(page, 'view.normal').click();
  await goTo(page, typedAt);
  await expect(page.getByLabel('Thanh công thức')).toHaveValue('777');

  // Page Break Preview: drag the right edge of the print area two columns left of the
  // automatic break — everything fits one page.
  await cmd(page, 'view.pageBreakPreview').click();
  await expect(page.getByTestId('page-label-2')).toBeVisible();
  const brk = page.locator('[data-testid^="page-break-col-"]').first();
  const k = Number((await brk.getAttribute('data-testid'))!.replace('page-break-col-', ''));
  const brkBox = await brk.boundingBox();
  const edge = page.getByTestId('print-area-edge-0-right');
  const box = await edge.boundingBox();
  expect(brkBox && box).toBeTruthy();
  if (brkBox && box) {
    const w = (box.x + box.width / 2 - (brkBox.x + brkBox.width / 2)) / (14 - k);
    const x = box.x + box.width / 2;
    const y = box.y + Math.min(30, box.height / 2);
    const target = brkBox.x + brkBox.width / 2 - 2 * w - 4;
    await page.mouse.move(x, y);
    await page.mouse.down();
    await page.mouse.move((x + target) / 2, y, { steps: 4 });
    await page.mouse.move(target, y, { steps: 4 });
    await page.mouse.up();
  }
  await expect(page.getByTestId('page-label-2')).toHaveCount(0);
  await expect(page.getByTestId('page-label-1')).toBeVisible();
  // One undo step brings the second page back.
  await page.getByLabel('Ô đang nhập').focus();
  await page.keyboard.press('Control+z');
  await expect(page.getByTestId('page-label-2')).toBeVisible();
  expect(errors).toEqual([]);
});
