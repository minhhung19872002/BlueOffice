import { expect, test, type Page } from '@playwright/test';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const here = dirname(fileURLToPath(import.meta.url));
const deck = join(here, '..', '..', 'corpus', 'slide', 'slide-text-bullets.pptx');

// CORE-THES-001: thesaurus (Open English WordNet data, loaded on first use) in BlueDoc — the
// context menu's "Từ đồng nghĩa" submenu and the task pane; a replacement keeps the word's
// capitalisation and is one undo step.
const cmd = (page: Page, id: string) => page.locator(`[data-command="${id}"]`);

async function newDoc(page: Page, text: string) {
  await page.goto('/doc');
  await page.getByTestId('new-document').click();
  await expect(page.getByTestId('caret')).toBeVisible();
  await page.keyboard.insertText(text);
}

test('replaces the word at the caret from the context menu submenu', async ({ page }) => {
  const errors: string[] = [];
  page.on('pageerror', (e) => errors.push(e.message));
  const fetched: string[] = [];
  page.on('request', (r) => {
    if (r.url().includes('/thesaurus/')) fetched.push(new URL(r.url()).pathname);
  });
  await newDoc(page, 'She is Happy today');
  await expect(page.getByTestId('status-words')).toHaveText('4 từ');
  // Nothing is downloaded before the thesaurus is used.
  expect(fetched).toEqual([]);

  // Caret inside "Happy", then the context menu of the caret (Shift+F10).
  await page.keyboard.press('Home');
  for (let i = 0; i < 9; i++) await page.keyboard.press('ArrowRight');
  await page.keyboard.press('Shift+F10');
  const menu = page.getByTestId('context-menu-text');
  await expect(menu).toBeVisible();
  await menu.getByRole('menuitem', { name: 'Từ đồng nghĩa' }).hover();
  const sub = page.getByTestId('thesaurus-menu');
  await expect(sub).toBeVisible();
  // WordNet's first sense of "happy": blessed, blissful, bright … — in the word's capitals.
  await expect(sub.getByRole('menuitem').first()).toHaveText('Blessed', { timeout: 30_000 });
  await sub.getByRole('menuitem', { name: 'Blissful', exact: true }).click();
  await expect(menu).toHaveCount(0);
  expect(fetched).toEqual(['/thesaurus/en/en.json.gz']);

  // The pane (Shift+F7) shows the word now at the caret.
  await page.keyboard.press('Shift+F7');
  const pane = page.getByTestId('thesaurus-pane');
  await expect(pane.getByTestId('thesaurus-search')).toHaveValue('Blissful');
  await expect(page.getByTestId('status-words')).toHaveText('4 từ');

  // One undo step brings "Happy" back.
  await page.keyboard.press('Control+z');
  await page.keyboard.press('Shift+F7');
  await expect(pane.getByTestId('thesaurus-search')).toHaveValue('Happy');
  expect(errors).toEqual([]);
});

test('replaces a word from the task pane, looks up inflected words and searches', async ({
  page,
}) => {
  const errors: string[] = [];
  page.on('pageerror', (e) => errors.push(e.message));
  await newDoc(page, 'The weather is cold');

  await page.getByRole('tab', { name: 'Xem lại' }).click();
  await cmd(page, 'review.thesaurus').click();
  const pane = page.getByTestId('thesaurus-pane');
  await expect(pane).toBeVisible();
  await expect(pane.getByTestId('thesaurus-search')).toHaveValue('cold');
  await expect(pane.getByTestId('thesaurus-results')).toBeVisible({ timeout: 30_000 });
  // Grouped by part of speech; the adjective group has WordNet's "chilly" and the antonym "hot".
  const adj = pane.locator('[data-testid="thesaurus-group"][data-pos="a"]');
  await expect(adj.locator('h3')).toHaveText('Tính từ');
  await expect(
    adj.locator('[data-testid="thesaurus-antonym"][data-word="hot"]').first(),
  ).toBeVisible();
  await adj.locator('[data-testid="thesaurus-synonym"][data-word="chilly"]').first().click();
  await pane.getByTestId('thesaurus-replace').click();
  await expect(pane.getByTestId('thesaurus-notice')).toHaveText('Đã thay “cold” bằng “chilly”.');
  await expect(pane.getByTestId('thesaurus-credit')).toContainText('Open English WordNet');

  // The document has the new word (the pane reopened on the word at the caret) …
  await cmd(page, 'review.thesaurus').click();
  await expect(pane.getByTestId('thesaurus-search')).toHaveValue('chilly');
  // … and one undo step restores the old one.
  await page.keyboard.press('Control+z');
  await cmd(page, 'review.thesaurus').click();
  await expect(pane.getByTestId('thesaurus-search')).toHaveValue('cold');

  // Search box: an inflected word is found through its base form.
  await pane.getByTestId('thesaurus-search').fill('faster');
  await pane.getByTestId('thesaurus-search').press('Enter');
  await expect(pane.getByTestId('thesaurus-lemma')).toContainText('fast');
  await pane.getByTestId('thesaurus-search').fill('xyzzyq');
  await pane.getByTestId('thesaurus-search').press('Enter');
  await expect(pane.getByTestId('thesaurus-none')).toContainText('“xyzzyq”');
  await page.getByTestId('thesaurus-close').click();
  await expect(pane).toHaveCount(0);
  expect(errors).toEqual([]);
});

test('BlueSlide: synonyms of the word at the caret of an edited text box', async ({ page }) => {
  const errors: string[] = [];
  page.on('pageerror', (e) => errors.push(e.message));
  await page.goto('/slide');
  const chooser = page.waitForEvent('filechooser');
  await page.getByRole('button', { name: 'Mở', exact: true }).click();
  await (await chooser).setFiles(deck);
  await expect(page.getByTestId('slide-overlay')).toBeVisible();
  await page.getByRole('button', { name: '+ Thêm slide' }).click();
  await expect(page.getByTestId('slide-status')).toHaveText(/^Slide 2 \/ \d+$/);
  await page.getByRole('tab', { name: 'Chèn' }).click();
  await page.getByRole('button', { name: 'Hộp văn bản' }).click();
  const tb = (await page.getByTestId('selection-box').first().boundingBox())!;
  await page.mouse.dblclick(tb.x + tb.width / 2, tb.y + tb.height / 2);
  const editor = page.getByTestId('slide-text-editor');
  await expect(editor).toBeFocused();
  await page.keyboard.insertText('A Quick fox');
  // Caret inside "Quick", then the menu of the caret.
  for (let i = 0; i < 6; i++) await page.keyboard.press('ArrowLeft');
  await page.keyboard.press('Shift+F10');
  const menu = page.getByTestId('context-menu-text');
  await menu.getByRole('menuitem', { name: 'Từ đồng nghĩa' }).hover();
  const sub = page.getByTestId('thesaurus-menu');
  // WordNet: quick — speedy, fast (in the word's capitalisation).
  await sub.getByRole('menuitem', { name: 'Speedy', exact: true }).click({ timeout: 30_000 });
  await expect(editor).toHaveValue('A Speedy fox', { timeout: 15_000 });
  expect(errors).toEqual([]);
});
