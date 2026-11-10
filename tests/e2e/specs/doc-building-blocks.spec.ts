import { expect, test, type Page } from '@playwright/test';

// DOC-BB-001: save the selection as a building block (the user's store), insert it in a
// new document by typing its name and pressing F3 and from the gallery; a block kept in
// the document survives save and reopen; the header gallery of the shipped library.

const cmd = (page: Page, id: string) => page.locator(`[data-command="${id}"]`);
const page1 = (page: Page) => page.locator('.bo-page[data-page="1"]');
const words = (page: Page) => page.getByTestId('status-words');

async function newDocument(page: Page) {
  await page.goto('/doc');
  await page.getByTestId('new-document').click();
  await expect(page.getByTestId('caret')).toBeVisible();
}

async function insertTab(page: Page) {
  await page.getByRole('tab', { name: 'Chèn' }).click();
}

/** Save the whole text as a building block through the dialog. */
async function saveSelection(page: Page, name: string, store: 'user' | 'document') {
  await page.keyboard.press('Control+a');
  await insertTab(page);
  await page.getByTestId('building-blocks').click();
  await cmd(page, 'insert.saveBlock').click();
  const dlg = page.getByTestId('save-block');
  await expect(dlg).toBeVisible();
  await dlg.getByTestId('block-name').fill(name);
  await dlg.getByTestId('block-gallery').selectOption('autoTxt');
  await dlg.getByTestId('block-store').selectOption(store);
  await dlg.getByTestId('save-block-ok').click();
  await expect(dlg).toHaveCount(0);
}

/** A click on the page gives the document the keyboard back, the caret at the end. */
async function focusEnd(page: Page) {
  const box = (await page1(page).boundingBox())!;
  await page.mouse.click(box.x + box.width * 0.3, box.y + box.height * 0.08);
  await page.keyboard.press('Control+End');
}

test('save a block, insert it in a new document with F3 and from the gallery', async ({ page }) => {
  const errors: string[] = [];
  page.on('pageerror', (e) => errors.push(e.message));
  await newDocument(page);
  await page.keyboard.insertText('Trân trọng kính chào và hợp tác');
  await expect(words(page)).toHaveText('7 từ');
  await saveSelection(page, 'tkc', 'user');

  // A new document in a fresh page load: the block comes from the user's store.
  await newDocument(page);
  await page.keyboard.insertText('Xin tkc');
  await expect(words(page)).toHaveText('2 từ');
  await page.keyboard.press('F3');
  await expect(words(page)).toHaveText('8 từ');
  // One undo step brings the typed name back.
  await page.keyboard.press('Control+z');
  await expect(words(page)).toHaveText('2 từ');
  await page.keyboard.press('Control+y');
  await expect(words(page)).toHaveText('8 từ');

  // The gallery lists it under the user's blocks, with a preview drawn by the engine.
  await page.keyboard.press('Enter');
  await insertTab(page);
  await page.getByTestId('building-blocks').click();
  const item = page.locator('[data-testid="block-gallery-quickParts"] [data-block="tkc"]');
  await expect(item).toHaveAttribute('data-store', 'user');
  await expect(item.getByTestId('block-preview')).toBeVisible();
  await item.click();
  await expect(words(page)).toHaveText('15 từ');

  // The Organizer shows the block and deletes it from the store.
  await page.getByTestId('building-blocks').click();
  await cmd(page, 'insert.blocksOrganizer').click();
  const org = page.getByTestId('blocks-organizer');
  const row = org.locator('[data-block="tkc"]');
  await expect(row).toHaveAttribute('data-store', 'user');
  await row.click();
  await expect(org.getByTestId('block-preview')).toBeVisible();
  await org.getByTestId('organizer-delete').click();
  await expect(org.locator('[data-block="tkc"]')).toHaveCount(0);
  // Library blocks are there and read-only.
  await org.locator('[data-block="Nơi nhận"]').click();
  await expect(org.getByTestId('organizer-delete')).toBeDisabled();
  await org.getByTestId('organizer-close').click();

  // F3 no longer finds the deleted block: the text stays.
  await focusEnd(page);
  await page.keyboard.insertText(' tkc');
  await expect(words(page)).toHaveText('16 từ');
  await page.keyboard.press('F3');
  await expect(words(page)).toHaveText('16 từ');
  expect(errors).toEqual([]);
});

test('a block kept in the document survives save and reopen', async ({ page }) => {
  const errors: string[] = [];
  page.on('pageerror', (e) => errors.push(e.message));
  await newDocument(page);
  await page.keyboard.insertText('Căn cứ quy chế làm việc');
  await expect(words(page)).toHaveText('6 từ');
  await saveSelection(page, 'ccqc', 'document');
  await expect(page.getByTestId('status-modified')).toBeVisible();

  const download = page.waitForEvent('download');
  await page.keyboard.press('Control+s');
  const path = await (await download).path();
  await page.reload();
  const chooser = page.waitForEvent('filechooser');
  await page.getByRole('button', { name: 'Mở tệp từ máy' }).click();
  await (await chooser).setFiles(path);
  await expect(page1(page)).toHaveAttribute('data-rendered', 'true');
  await expect(words(page)).toHaveText('6 từ');

  await focusEnd(page);
  await page.keyboard.press('Enter');
  await insertTab(page);
  await page.getByTestId('building-blocks').click();
  const item = page.locator('[data-testid="block-gallery-quickParts"] [data-block="ccqc"]');
  await expect(item).toHaveAttribute('data-store', 'document');
  await item.click();
  await expect(words(page)).toHaveText('12 từ');
  expect(errors).toEqual([]);
});

test('header, page number and cover page galleries of the library', async ({ page }) => {
  const errors: string[] = [];
  page.on('pageerror', (e) => errors.push(e.message));
  await newDocument(page);
  await page.keyboard.insertText('Thân văn bản');
  await insertTab(page);

  await page.getByTestId('header-gallery').click();
  const headers = page.getByTestId('block-gallery-header');
  await expect(headers.locator('[data-block]')).toHaveCount(4);
  await expect(headers.getByTestId('block-preview').first()).toBeVisible();
  await headers.locator('[data-block="Đầu trang – Tên tài liệu"]').click();
  await expect(page.getByTestId('status-modified')).toBeVisible();
  // The header is not part of the word count; the text is unchanged.
  await expect(words(page)).toHaveText('3 từ');

  await page.getByTestId('page-number-gallery').click();
  await page
    .locator('[data-testid="block-gallery-pageNumber"] [data-block="Số trang – Trang X / Y"]')
    .click();

  // A cover page adds a page before the text.
  await expect(page.locator('.bo-page')).toHaveCount(1);
  await page.getByTestId('cover-page-gallery').click();
  await page
    .locator('[data-testid="block-gallery-coverPage"] [data-block="Bìa – Khung giữa"]')
    .click();
  await expect(page.locator('.bo-page')).toHaveCount(2);

  // The existing "edit header" command is the action below the gallery.
  await page.getByTestId('header-gallery').click();
  await cmd(page, 'insert.header').click();
  await expect(cmd(page, 'hf.close')).toBeVisible();
  expect(errors).toEqual([]);
});
