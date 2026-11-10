import { expect, test, type Page } from '@playwright/test';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

// File screen (CORE-UI-002): navigation, Info properties kept through save and
// reopen, every output format on Save As, Escape closes.
const here = dirname(fileURLToPath(import.meta.url));
const deck = join(here, '..', '..', 'corpus', 'slide', 'slide-text-bullets.pptx');

const nav = (page: Page, name: string) =>
  page.getByRole('navigation', { name: 'Tệp' }).getByRole('button', { name, exact: true });

async function formats(page: Page) {
  await page.getByTestId('file-tab').click();
  await expect(page.getByRole('dialog', { name: 'Tệp' })).toBeVisible();
  await nav(page, 'Lưu thành').click();
  await expect(page.getByTestId('backstage')).toHaveAttribute('data-page', 'saveAs');
  const list = await page
    .locator('[data-format]')
    .evaluateAll((els) => els.map((e) => (e as HTMLElement).dataset.format));
  await page.keyboard.press('Escape');
  await expect(page.getByTestId('backstage')).toHaveCount(0);
  return list;
}

test('edits the title on File › Info, saves, reopens: the title is kept', async ({ page }) => {
  const errors: string[] = [];
  page.on('pageerror', (e) => errors.push(e.message));
  await page.goto('/doc');
  await page.getByTestId('new-document').click();
  await expect(page.getByTestId('doc-name')).toHaveText('Tài liệu mới.docx');

  await page.getByTestId('file-tab').click();
  const screen = page.getByRole('dialog', { name: 'Tệp' });
  await expect(screen).toBeVisible();
  await expect(page.getByTestId('backstage')).toHaveAttribute('data-page', 'info');
  // Keyboard: the arrows move through the navigation column.
  await expect(nav(page, 'Thông tin')).toBeFocused();
  await page.keyboard.press('ArrowDown');
  await expect(nav(page, 'Lưu')).toBeFocused();
  // Info hosts protection, check document and versions.
  await expect(screen.locator('[data-command="file.protect"]')).toBeVisible();
  await expect(screen.locator('[data-command="review.checkDocument"]')).toBeVisible();
  await expect(screen.locator('[data-command="review.versions"]')).toBeVisible();

  const title = page.getByTestId('prop-title');
  await title.fill('Báo cáo quý <3> & tổng kết');
  await title.press('Enter');
  await page.getByTestId('prop-author').fill('Nguyễn Văn A');
  await page.getByTestId('prop-author').press('Tab');
  // Save from the File screen: it closes and the .docx downloads.
  const download = page.waitForEvent('download');
  await nav(page, 'Lưu').click();
  const d = await download;
  await expect(page.getByTestId('backstage')).toHaveCount(0);
  expect(d.suggestedFilename()).toBe('Tài liệu mới.docx');
  const path = await d.path();

  // Reopen through File › Open › browse.
  await page.reload();
  await page.getByTestId('file-tab').click();
  await expect(page.getByTestId('backstage')).toHaveAttribute('data-page', 'open');
  const chooser = page.waitForEvent('filechooser');
  await page.locator('[data-testid="backstage"] [data-command="file.open"]').click();
  await (await chooser).setFiles(path);
  await expect(page.locator('.bo-page[data-page="1"]')).toHaveAttribute('data-rendered', 'true');
  await page.getByTestId('file-tab').click();
  await expect(page.getByTestId('prop-title')).toHaveValue('Báo cáo quý <3> & tổng kết');
  await expect(page.getByTestId('prop-author')).toHaveValue('Nguyễn Văn A');
  // Back closes.
  await page.getByTestId('backstage-back').click();
  await expect(page.getByTestId('backstage')).toHaveCount(0);
  expect(errors).toEqual([]);
});

test('Save As lists every output format of each app', async ({ page }) => {
  await page.goto('/doc');
  await page.getByTestId('new-document').click();
  await expect(page.getByTestId('doc-name')).toHaveText('Tài liệu mới.docx');
  expect(await formats(page)).toEqual(['.docx', '.docm', '.dotx', '.dotm', '.pdf']);

  await page.goto('/sheet');
  await expect(page.getByRole('tab', { name: 'Sheet1' })).toBeVisible();
  expect(await formats(page)).toEqual([
    '.xlsx',
    '.xlsm',
    '.xltx',
    '.xltm',
    '.csv',
    '.pdf',
    '.thmx',
  ]);

  await page.goto('/slide');
  const chooser = page.waitForEvent('filechooser');
  await page.getByRole('button', { name: 'Mở', exact: true }).click();
  await (await chooser).setFiles(deck);
  await expect(page.getByTestId('slide-status')).toHaveText(/^Slide 1 \/ \d+$/);
  expect(await formats(page)).toEqual([
    '.pptx',
    '.pptm',
    '.potx',
    '.potm',
    '.ppsx',
    '.ppsm',
    '.pdf',
    '.png',
    '.gif',
    '.thmx',
  ]);
});
