import { expect, test, type Page } from '@playwright/test';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

// CORE-MISC-003: copying goes through the engine and then writes the system clipboard, both
// asynchronously. Ctrl+V pressed before the copy reached the clipboard must paste the copy,
// not what the clipboard held before. A slow clipboard (as with a permission prompt or the
// desktop bridge) makes the race certain: writes land 1.5 s late.

const here = dirname(fileURLToPath(import.meta.url));
const deck = join(here, '..', '..', 'corpus', 'slide', 'slide-text-bullets.pptx');

test.use({ permissions: ['clipboard-read', 'clipboard-write'] });

async function slowClipboard(page: Page) {
  await page.addInitScript(() => {
    const c = navigator.clipboard;
    const later = () => new Promise((r) => setTimeout(r, 1500));
    const write = c.write.bind(c);
    const writeText = c.writeText.bind(c);
    const w = window as unknown as { slowClipboard?: boolean };
    c.write = async (data) => {
      if (w.slowClipboard) await later();
      return write(data);
    };
    c.writeText = async (text) => {
      if (w.slowClipboard) await later();
      return writeText(text);
    };
  });
}

async function seed(page: Page, text: string) {
  await page.evaluate(async (t) => {
    await navigator.clipboard.writeText(t);
    (window as unknown as { slowClipboard?: boolean }).slowClipboard = true;
  }, text);
}

test('BlueDoc: Ctrl+V right after Ctrl+C pastes the new copy', async ({ page }) => {
  await slowClipboard(page);
  await page.goto('/doc');
  await page.getByTestId('new-document').click();
  await expect(page.getByTestId('caret')).toBeVisible();
  await seed(page, 'cũ');
  await page.keyboard.insertText('Một hai ba');
  const words = page.getByTestId('status-words');
  await expect(words).toHaveText('3 từ');
  await page.keyboard.press('Control+a');
  await page.keyboard.press('Control+c');
  await page.keyboard.press('Control+End');
  await page.keyboard.press('Enter');
  await page.keyboard.press('Control+v');
  // The paste waited for the copy: "Một hai ba" twice (a stale paste: 4 words with "cũ").
  await expect(words).toHaveText('6 từ');
  await expect.poll(() => page.evaluate(() => navigator.clipboard.readText())).toBe('Một hai ba');
});

test('BlueSlide: Ctrl+V right after Ctrl+C pastes the copied shape', async ({ page }) => {
  await slowClipboard(page);
  await page.goto('/slide');
  const chooser = page.waitForEvent('filechooser');
  await page.getByRole('button', { name: 'Mở', exact: true }).click();
  await (await chooser).setFiles(deck);
  await expect(page.getByTestId('slide-overlay')).toBeVisible();
  await page.getByRole('button', { name: '+ Thêm slide' }).click();
  await page.getByRole('tab', { name: 'Chèn' }).click();
  await page.getByRole('button', { name: 'Hộp văn bản' }).click();
  // The button arms the drawing pointer; Enter puts the object in the middle of the slide.
  await page.keyboard.press('Enter');
  const tb = (await page.getByTestId('selection-box').first().boundingBox())!;
  await page.mouse.dblclick(tb.x + tb.width / 2, tb.y + tb.height / 2);
  await expect(page.getByTestId('slide-text-editor')).toBeFocused();
  await page.keyboard.insertText('Hộp chữ');
  await page.keyboard.press('Escape');
  await page.mouse.click(tb.x + tb.width / 2, tb.y + tb.height / 2);
  await expect(page.getByTestId('selection-box')).toHaveCount(1);
  await seed(page, 'chữ của chương trình khác');
  await page.keyboard.press('Control+c');
  await page.keyboard.press('Control+v');
  // Our shapes were pasted (paste options are offered only for them), not a text box
  // holding the older clipboard text.
  await expect(page.getByTestId('paste-options')).toBeVisible({ timeout: 10_000 });
  await expect.poll(() => page.evaluate(() => navigator.clipboard.readText())).toBe('Hộp chữ');
});
