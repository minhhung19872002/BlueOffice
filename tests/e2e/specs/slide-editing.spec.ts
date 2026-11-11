import { expect, test, type Page } from '@playwright/test';
import { mkdirSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const here = dirname(fileURLToPath(import.meta.url));
const outDir = join(here, '..', '..', 'fidelity', 'output');
const deck = join(here, '..', '..', 'corpus', 'slide', 'slide-text-bullets.pptx');

async function openDeck(page: Page, path: string) {
  const chooser = page.waitForEvent('filechooser');
  await page.getByRole('button', { name: 'Mở', exact: true }).click();
  await (await chooser).setFiles(path);
  await expect(page.getByTestId('slide-status')).toHaveText(/^Slide 1 \/ \d+$/);
  await expect(page.getByTestId('slide-overlay')).toBeVisible();
}

/** Bounding box of the selection outline in page pixels. */
async function selectionBox(page: Page) {
  const box = await page.getByTestId('selection-box').first().boundingBox();
  expect(box).not.toBeNull();
  return box!;
}

test('moves a shape, edits its text in Vietnamese, saves .pptx and reopens it', async ({
  page,
}) => {
  const errors: string[] = [];
  page.on('pageerror', (e) => errors.push(e.message));
  await page.goto('/slide');
  await openDeck(page, deck);

  // Select the title placeholder (centre of the title area of the title slide).
  const overlay = page.getByTestId('slide-overlay');
  const o = (await overlay.boundingBox())!;
  await page.mouse.click(o.x + o.width * 0.48, o.y + o.height * 0.3);
  await expect(page.getByTestId('selection-box')).toHaveCount(1);
  await expect(page.getByTestId('handle-se')).toBeVisible();
  const before = await selectionBox(page);

  // Drag it down-right; the drag shows a preview and moves the shape on release.
  const sx = before.x + before.width / 2;
  const sy = before.y + before.height / 2;
  await page.mouse.move(sx, sy);
  await page.mouse.down();
  await page.mouse.move(sx + 40, sy + 30, { steps: 4 });
  await page.mouse.move(sx + 60, sy + 70, { steps: 4 });
  await expect(page.locator('[data-testid="slide-overlay"] rect[class*="ghost"]')).toHaveCount(1);
  await page.mouse.up();
  await expect(page.getByTestId('status-modified')).toBeVisible();
  const after = await selectionBox(page);
  expect(after.x - before.x).toBeGreaterThan(40);
  expect(after.y - before.y).toBeGreaterThan(50);

  // Double-click: edit the text in the overlay textarea (IME commits arrive as insertText).
  await page.mouse.dblclick(after.x + after.width / 2, after.y + after.height / 2);
  const editor = page.getByTestId('slide-text-editor');
  await expect(editor).toBeVisible();
  await expect(editor).toHaveValue('Báo cáo kết quả kinh doanh');
  await page.keyboard.press('Control+a');
  await page.keyboard.insertText('Xin chào ');
  await page.keyboard.insertText('Việt Nam');
  await page.keyboard.press('Escape');
  await expect(editor).toHaveCount(0);
  mkdirSync(outDir, { recursive: true });
  await page.screenshot({ path: join(outDir, 'web-slide-editing.png') });

  // Undo/redo the text edit.
  await page.keyboard.press('Control+z');
  await page.keyboard.press('Control+y');

  // Save (Ctrl+S) downloads the deck and clears the modified flag.
  const download = page.waitForEvent('download');
  await page.keyboard.press('Control+s');
  const d = await download;
  expect(d.suggestedFilename()).toBe('slide-text-bullets.pptx');
  const saved = await d.path();
  await expect(page.getByTestId('status-modified')).toHaveCount(0);

  // Reopen the saved file: the shape is at its new place with the new text.
  await page.reload();
  await openDeck(page, saved);
  await page.mouse.click(after.x + after.width / 2, after.y + after.height / 2);
  const again = await selectionBox(page);
  expect(Math.abs(again.x - after.x)).toBeLessThan(2);
  expect(Math.abs(again.y - after.y)).toBeLessThan(2);
  await page.mouse.dblclick(again.x + again.width / 2, again.y + again.height / 2);
  await expect(page.getByTestId('slide-text-editor')).toHaveValue('Xin chào Việt Nam');
  await page.keyboard.press('Escape');
  // The old position is empty now.
  await page.mouse.click(before.x + 4, before.y + 4);
  await expect(page.getByTestId('selection-box')).toHaveCount(0);
  expect(errors).toEqual([]);
});

test('inserts a shape, shows smart guides, adds a slide and edits notes', async ({ page }) => {
  await page.goto('/slide');
  await openDeck(page, deck);
  await page.getByRole('tab', { name: 'Chèn' }).click();
  await page.getByRole('button', { name: 'Hình elip' }).click();
  // The button arms the drawing pointer; Enter puts the object in the middle of the slide.
  await page.keyboard.press('Enter');
  await expect(page.getByTestId('selection-box')).toHaveCount(1);
  // A new shape is centred: dragging it slightly keeps it on the centre guide.
  const b = await selectionBox(page);
  await page.mouse.move(b.x + b.width / 2, b.y + b.height / 2);
  await page.mouse.down();
  await page.mouse.move(b.x + b.width / 2 + 2, b.y + b.height / 2 + 40, { steps: 5 });
  // Guide lines are zero-width SVG lines: attached, not "visible".
  await expect(page.locator('[data-testid="smart-guide"][x1="480"]')).toBeAttached();
  await page.mouse.up();
  // Delete it again, then add a slide.
  await page.keyboard.press('Delete');
  await expect(page.getByTestId('selection-box')).toHaveCount(0);
  const count = await page.locator('[data-testid^="thumb-"]').count();
  await page.getByRole('button', { name: '+ Thêm slide' }).click();
  await expect(page.locator('[data-testid^="thumb-"]')).toHaveCount(count + 1);
  await expect(page.getByTestId('slide-status')).toHaveText(`Slide 2 / ${count + 1}`);
  const notes = page.getByTestId('notes-editor');
  await notes.click();
  await page.keyboard.insertText('Ghi chú cho slide mới');
  await page.getByTestId('thumb-1').click();
  await page.getByTestId('thumb-2').click();
  await expect(notes).toHaveValue('Ghi chú cho slide mới');
});
