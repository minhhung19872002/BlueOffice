import { expect, test, type Page } from '@playwright/test';

// CORE-UI-005: customise the ribbon and the Quick Access Toolbar; kept per user, exported and
// imported as one file.
const cmd = (page: Page, id: string) => page.locator(`[data-command="${id}"]`);

async function newDocument(page: Page) {
  await page.getByTestId('new-document').click();
  await expect(page.getByTestId('caret')).toBeVisible();
}

async function pick(page: Page, query: string, id: string) {
  const dialog = page.getByTestId('customize-dialog');
  await dialog.getByTestId('customize-search').fill(query);
  await dialog.locator(`[data-command-option="${id}"]`).click();
  await dialog.getByTestId('customize-add').click();
}

test('doc: a custom tab with two commands is used, kept after reload, exported and imported', async ({
  page,
}) => {
  const errors: string[] = [];
  page.on('pageerror', (e) => errors.push(e.message));
  await page.goto('/doc');
  await newDocument(page);

  // File › Options › Customise the ribbon.
  await page.getByTestId('file-tab').click();
  await page
    .getByRole('navigation', { name: 'Tệp' })
    .getByRole('button', { name: 'Tùy chọn', exact: true })
    .click();
  await page.getByRole('button', { name: 'Tùy biến ribbon…' }).click();
  const dialog = page.getByTestId('customize-dialog');
  await expect(dialog).toBeVisible();
  await dialog.locator('[data-node="tab:home"]').click();
  await dialog.getByTestId('customize-new-tab').click();
  await expect(dialog.locator('[data-node="group:custom.tab1/custom.group1"]')).toHaveAttribute(
    'aria-selected',
    'true',
  );
  await pick(page, 'dam', 'format.bold');
  await pick(page, 'nghieng', 'format.italic');
  await expect(dialog.locator('[data-node^="cmd:custom.tab1/custom.group1/"]')).toHaveCount(2);
  await dialog.getByTestId('customize-close').click();
  await expect(dialog).toHaveCount(0);

  // The tab is on the ribbon right after Home, with KeyTips.
  const tabs = page.locator('[data-testid="ribbon"] [role="tab"]');
  await expect(tabs.nth(1)).toHaveAttribute('data-tab', 'custom.tab1');
  await expect(tabs.nth(1)).toHaveText('Tab mới (Tùy chỉnh)');
  await page.keyboard.insertText('Xin chào');
  await page.keyboard.press('Shift+Home');
  await tabs.nth(1).click();
  const group = page.locator('[data-group="custom.group1"]');
  await expect(group.locator('button[data-command]')).toHaveCount(2);
  await group.locator('[data-command="format.bold"]').click();
  await expect(group.locator('[data-command="format.bold"]')).toHaveAttribute(
    'aria-pressed',
    'true',
  );

  // Right click a ribbon command: add it to the Quick Access Toolbar.
  await group.locator('[data-command="format.italic"]').click({ button: 'right' });
  const menu = page.getByTestId('context-menu-ribbon');
  await menu.locator('[data-menu-command="ribbon.addToQat"]').click();
  await expect(cmd(page, 'qat:format.italic')).toBeVisible();

  // Kept after reload.
  await page.reload();
  await newDocument(page);
  await expect(page.locator('[data-tab="custom.tab1"]')).toBeVisible();
  await expect(cmd(page, 'qat:format.italic')).toBeVisible();

  // Export, reset everything, import: restored.
  await page.locator('[data-tab="home"]').click({ button: 'right' });
  await page
    .getByTestId('context-menu-ribbon')
    .locator('[data-menu-command="options.customizeRibbon"]')
    .click();
  await expect(dialog).toBeVisible();
  const download = page.waitForEvent('download');
  await dialog.getByTestId('customize-export').click();
  const file = await (await download).path();
  await expect(dialog.getByTestId('customize-notice')).toBeVisible();
  await dialog.getByTestId('customize-reset-all').click();
  await dialog.getByTestId('customize-page-qat').click();
  await dialog.getByTestId('customize-qat-reset').click();
  await expect(page.locator('[data-tab="custom.tab1"]')).toHaveCount(0);
  await expect(cmd(page, 'qat:format.italic')).toHaveCount(0);
  const chooser = page.waitForEvent('filechooser');
  await dialog.getByTestId('customize-import').click();
  await (await chooser).setFiles(file);
  await expect(dialog.getByTestId('customize-notice')).toHaveText(
    'Đã nhập ribbon, thanh truy cập nhanh và phím tắt.',
  );
  await expect(page.locator('[data-tab="custom.tab1"]')).toBeVisible();
  await expect(cmd(page, 'qat:format.italic')).toBeVisible();

  // The toolbar can move below the ribbon.
  await dialog.getByTestId('customize-qat-below').check();
  await expect(
    page.getByTestId('qat-below').locator('[data-command="qat:format.italic"]'),
  ).toBeVisible();
  await dialog.getByTestId('customize-close').click();
  expect(errors).toEqual([]);
});

test('sheet: a built-in group hidden from the ribbon menu stays hidden after reload', async ({
  page,
}) => {
  await page.goto('/sheet');
  await expect(page.getByTestId('ribbon')).toBeVisible();
  await page.getByTestId('ribbon').locator('[data-tab="home"]').click({ button: 'right' });
  await page
    .getByTestId('context-menu-ribbon')
    .locator('[data-menu-command="options.customizeRibbon"]')
    .click();
  const dialog = page.getByTestId('customize-dialog');
  const home = dialog.locator('[data-node="tab:home"]');
  await home.getByTestId('customize-twisty').click();
  const first = dialog.locator('[data-node^="group:home/"]').first();
  const id = (await first.getAttribute('data-node'))!.split('/')[1]!;
  await first.getByTestId('customize-visible').uncheck();
  await dialog.getByTestId('customize-close').click();
  await expect(page.locator(`[data-testid="ribbon"] [data-group="${id}"]`)).toHaveCount(0);
  await page.reload();
  await expect(page.getByTestId('ribbon')).toBeVisible();
  await expect(page.locator(`[data-testid="ribbon"] [data-group="${id}"]`)).toHaveCount(0);
});
