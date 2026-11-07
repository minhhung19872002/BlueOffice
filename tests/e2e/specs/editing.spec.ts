import { expect, test, type Page } from '@playwright/test';
import { mkdirSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const outDir = join(dirname(fileURLToPath(import.meta.url)), '..', '..', 'fidelity', 'output');

async function caretReady(page: Page) {
  await expect(page.getByTestId('caret')).toBeVisible();
}

test('types Vietnamese, bolds it, saves .docx and reopens it', async ({ page }) => {
  const errors: string[] = [];
  page.on('pageerror', (e) => errors.push(e.message));
  await page.goto('/doc');
  await page.getByTestId('new-document').click();
  await expect(page.getByTestId('doc-name')).toHaveText('Tài liệu mới.docx');
  await caretReady(page);

  // Typing goes through the hidden IME textarea (insertText = committed IME text).
  await page.keyboard.type('Xin chào ');
  await page.keyboard.insertText('Việt Nam');
  await expect(page.getByTestId('status-words')).toHaveText('4 từ');
  await expect(page.getByTestId('status-modified')).toBeVisible();

  // Select all and bold (Ctrl+B); the ribbon reflects the state.
  await page.keyboard.press('Control+a');
  await page.keyboard.press('Control+b');
  const bold = page.locator('[data-command="format.bold"]');
  await expect(bold).toHaveAttribute('aria-pressed', 'true');
  mkdirSync(outDir, { recursive: true });
  await page.screenshot({ path: join(outDir, 'web-editing.png') });

  // Undo/redo round trip.
  await page.keyboard.press('Control+z');
  await expect(bold).toHaveAttribute('aria-pressed', 'false');
  await page.keyboard.press('Control+y');
  await expect(bold).toHaveAttribute('aria-pressed', 'true');

  // Save (Ctrl+S) downloads the .docx.
  const download = page.waitForEvent('download');
  await page.keyboard.press('Control+s');
  const d = await download;
  expect(d.suggestedFilename()).toBe('Tài liệu mới.docx');
  const path = await d.path();
  await expect(page.getByTestId('status-modified')).toHaveCount(0);

  // Reopen the saved file through the file picker.
  await page.reload();
  const chooser = page.waitForEvent('filechooser');
  await page.getByRole('button', { name: 'Mở tệp từ máy' }).click();
  await (await chooser).setFiles(path);
  await expect(page.getByTestId('status-words')).toHaveText('4 từ');
  await expect(page.locator('.bo-page[data-page="1"]')).toHaveAttribute('data-rendered', 'true');
  // Click into the text: the formatting there is bold.
  const box = (await page.locator('.bo-page[data-page="1"]').boundingBox())!;
  await page.mouse.click(box.x + box.width * 0.2, box.y + box.height * 0.08);
  await expect(bold).toHaveAttribute('aria-pressed', 'true');
  // Find the text.
  await page.keyboard.press('Control+f');
  await page.getByTestId('find-query').fill('việt');
  await page.getByTestId('find-query').press('Enter');
  await expect(page.getByTestId('find-message')).toHaveText('1 kết quả');
  expect(errors).toEqual([]);
});

test('Enter, Backspace and arrows edit paragraphs', async ({ page }) => {
  await page.goto('/doc');
  await page.getByTestId('new-document').click();
  await caretReady(page);
  await page.keyboard.type('một');
  await page.keyboard.press('Enter');
  await page.keyboard.type('hai ba');
  await expect(page.getByTestId('status-words')).toHaveText('3 từ');
  await page.keyboard.press('Backspace');
  await page.keyboard.press('Backspace');
  await page.keyboard.press('Backspace');
  await expect(page.getByTestId('status-words')).toHaveText('2 từ');
  await page.keyboard.press('Home');
  await page.keyboard.press('Backspace'); // merge with the first paragraph
  await expect(page.getByTestId('status-words')).toHaveText('1 từ');
});
