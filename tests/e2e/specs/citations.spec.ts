import { expect, test, type Page } from '@playwright/test';

// Citations and bibliography (CORE-CITE-001): a new source, a citation with a page, the
// bibliography, switching the reference style (every field follows, one undo step), save and
// reopen. The page is drawn on a canvas, so the text is checked through the word count of the
// status bar and the source manager's preview of the citation and the entry.

const cmd = (page: Page, id: string) => page.locator(`[data-command="${id}"]`);
const page1 = (page: Page) => page.locator('.bo-page[data-page="1"]');
const words = (page: Page) => page.getByTestId('status-words');

/** Caret box (retried: the caret element is replaced after every command). */
async function caretBox(page: Page) {
  for (let i = 0; i < 40; i++) {
    const b = await page.getByTestId('caret').boundingBox();
    if (b) return b;
    await page.waitForTimeout(50);
  }
  throw new Error('no caret');
}

/** Click the caret's current position (keeps the selection, focuses the document). */
async function clickCaret(page: Page) {
  const c = await caretBox(page);
  await page.mouse.click(c.x + 1, c.y + c.height / 2);
}

/** Save with Ctrl+S, reload and open the downloaded file. */
async function saveAndReopen(page: Page) {
  const download = page.waitForEvent('download');
  await page.keyboard.press('Control+s');
  const path = await (await download).path();
  await page.reload();
  const chooser = page.waitForEvent('filechooser');
  await page.getByRole('button', { name: 'Mở tệp từ máy' }).click();
  await (await chooser).setFiles(path);
  await expect(page1(page)).toHaveAttribute('data-rendered', 'true');
}

test('source, citation, bibliography, style switch, save and reopen', async ({ page }) => {
  const errors: string[] = [];
  page.on('pageerror', (e) => errors.push(e.message));
  await page.goto('/doc');
  await page.getByTestId('new-document').click();
  await expect(page.getByTestId('caret')).toBeVisible();
  await page.keyboard.insertText('Theo ');
  await page.getByRole('tab', { name: 'Tham chiếu' }).click();
  // A new document uses APA.
  await expect(page.getByTestId('cite-style')).toHaveValue('APA');

  // Insert citation → new source.
  await cmd(page, 'references.insertCitation').click();
  await expect(page.getByTestId('cite-insert-dialog')).toBeVisible();
  await expect(page.getByTestId('cite-insert-ok')).toBeDisabled();
  await page.getByTestId('cite-new-source').click();
  await expect(page.getByTestId('cite-source-dialog')).toBeVisible();
  await page.getByTestId('cite-language').selectOption('1033');
  await page.getByTestId('cite-author').fill('Smith, John');
  await page.getByTestId('cite-field-Title').fill('Research methods');
  await page.getByTestId('cite-field-Year').fill('2020');
  await page.getByTestId('cite-field-Publisher').fill('Blue Press');
  await page.getByTestId('cite-source-save').click();
  // Back in the citation dialog: the new source (tag from author and year) is chosen.
  await expect(page.getByTestId('cite-insert-dialog')).toBeVisible();
  const item = page.getByTestId('cite-items').locator('[data-tag="Smi20"]');
  await expect(item).toContainText('Smith (2020). Research methods');
  await item.getByTestId('cite-item-pages').fill('12');
  await page.getByTestId('cite-insert-ok').click();
  await expect(page.getByTestId('cite-insert-dialog')).toHaveCount(0);
  // "Theo (Smith, 2020, tr. 12)" — the Vietnamese interface cites with "tr.".
  await expect(words(page)).toHaveText('5 từ');

  // Bibliography with the default heading "Tài liệu tham khảo":
  // "Smith, J. (2020). Research methods. Blue Press."
  await page.keyboard.press('Enter');
  await cmd(page, 'references.insertBibliography').click();
  await expect(page.getByTestId('cite-bibliography-dialog')).toBeVisible();
  await expect(page.getByTestId('cite-heading')).toHaveValue('references');
  await page.getByTestId('cite-bibliography-ok').click();
  await expect(page.getByTestId('cite-bibliography-dialog')).toHaveCount(0);
  await expect(words(page)).toHaveText('16 từ');

  // The source manager shows the citation and the entry in the current style.
  await cmd(page, 'references.manageSources').click();
  const manager = page.getByTestId('cite-manager-dialog');
  await expect(manager).toBeVisible();
  const inDocument = page.getByTestId('cite-document-list').locator('[data-tag="Smi20"]');
  await expect(inDocument).toContainText('đã trích dẫn');
  // The new source went to the master list too.
  await expect(page.getByTestId('cite-master-list').locator('[data-tag="Smi20"]')).toBeVisible();
  await inDocument.click();
  await expect(page.getByTestId('cite-preview')).toContainText('(Smith, 2020)');
  await expect(page.getByTestId('cite-preview')).toContainText(
    'Smith, J. (2020). Research methods. Blue Press.',
  );
  // A cited source is not deleted.
  await page.getByTestId('cite-delete-source').click();
  await expect(page.getByTestId('cite-error')).toContainText('đang được trích dẫn');
  await page.keyboard.press('Escape');
  await expect(manager).toHaveCount(0);

  // MLA: "Theo (Smith 12)" and "Smith, John. Research methods. Blue Press, 2020."
  await page.getByTestId('cite-style').selectOption('MLA');
  await expect(words(page)).toHaveText('14 từ');
  await cmd(page, 'references.manageSources').click();
  await page.getByTestId('cite-document-list').locator('[data-tag="Smi20"]').click();
  await expect(page.getByTestId('cite-preview')).toContainText('(Smith)');
  await expect(page.getByTestId('cite-preview')).toContainText(
    'Smith, John. Research methods. Blue Press, 2020.',
  );
  await page.keyboard.press('Escape');
  // Switching style is one undo step for the citation and the list together.
  await clickCaret(page);
  await page.keyboard.press('Control+z');
  await expect(words(page)).toHaveText('16 từ');
  await page.keyboard.press('Control+y');
  await expect(words(page)).toHaveText('14 từ');

  // Saved and opened again: same text, same style, same sources.
  await saveAndReopen(page);
  await expect(words(page)).toHaveText('14 từ');
  await page.getByRole('tab', { name: 'Tham chiếu' }).click();
  await expect(page.getByTestId('cite-style')).toHaveValue('MLA');
  // IEEE in the reopened file: "Theo [1, tr. 12]", "[1] J. Smith, Research methods. Blue
  // Press, 2020."
  await page.getByTestId('cite-style').selectOption('IEEE');
  await expect(words(page)).toHaveText('16 từ');
  await cmd(page, 'references.manageSources').click();
  const reopened = page.getByTestId('cite-document-list').locator('[data-tag="Smi20"]');
  await expect(reopened).toContainText('đã trích dẫn');
  await reopened.click();
  await expect(page.getByTestId('cite-preview')).toContainText('[1]');
  await expect(page.getByTestId('cite-preview')).toContainText(
    'J. Smith, Research methods. Blue Press, 2020.',
  );
  expect(errors).toEqual([]);
});
