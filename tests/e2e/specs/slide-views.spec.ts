import { expect, test, type Page } from '@playwright/test';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

// SLIDE-VIEW-001: outline view, notes pages, handout / notes master views, sections.

const here = dirname(fileURLToPath(import.meta.url));
const deck = join(here, '..', '..', 'corpus', 'slide', 'slide-text-bullets.pptx');

async function openDeck(page: Page, path: string) {
  const chooser = page.waitForEvent('filechooser');
  await page.getByRole('button', { name: 'Mở', exact: true }).click();
  await (await chooser).setFiles(path);
  await expect(page.getByTestId('slide-status')).toHaveText(/^Slide 1 \/ \d+$/);
  await expect(page.getByTestId('slide-overlay')).toBeVisible();
}

const thumbs = (page: Page) => page.locator('[data-testid^="thumb-"]');
const outlineSlides = (page: Page) => page.locator('[data-testid^="outline-slide-"]');

async function viewTab(page: Page) {
  await page.getByRole('tab', { name: 'Xem', exact: true }).click();
}

async function button(page: Page, name: string) {
  await page.getByRole('button', { name, exact: true }).click();
}

test('outline view: Ctrl+Enter, Enter, Tab / Shift+Tab edit the slides', async ({ page }) => {
  const errors: string[] = [];
  page.on('pageerror', (e) => errors.push(e.message));
  await page.goto('/slide');
  await openDeck(page, deck);
  const n = await thumbs(page).count();
  await viewTab(page);
  await button(page, 'Dàn ý');
  await expect(page.getByTestId('outline-view')).toBeVisible();
  await expect(outlineSlides(page)).toHaveCount(n);
  const first = page.getByTestId('outline-line-0');
  await expect(first).toHaveAttribute('data-kind', 'title');
  await expect(first).not.toHaveValue('');

  // Ctrl+Enter: a new slide after slide 1; type its title.
  await first.click();
  await page.keyboard.press('End');
  await page.keyboard.press('Control+Enter');
  await expect(outlineSlides(page)).toHaveCount(n + 1);
  await page.keyboard.insertText('Slide mới từ dàn ý');
  // Enter: a first bullet under the title; Tab: one level down.
  await page.keyboard.press('Enter');
  await page.keyboard.insertText('Ý thứ nhất');
  await page.keyboard.press('Tab');
  const focused = page.locator('input:focus');
  await expect(focused).toHaveValue('Ý thứ nhất');
  await expect(focused).toHaveAttribute('data-kind', 'bullet');
  await expect(focused).toHaveAttribute('data-level', '1');
  // Shift+Tab twice: level 0, then the title of another new slide.
  await page.keyboard.press('Shift+Tab');
  await page.keyboard.press('Shift+Tab');
  await expect(outlineSlides(page)).toHaveCount(n + 2);
  await expect(focused).toHaveAttribute('data-kind', 'title');
  // Tab: back to a bullet of the new slide.
  await page.keyboard.press('Tab');
  await expect(outlineSlides(page)).toHaveCount(n + 1);
  await expect(focused).toHaveAttribute('data-level', '0');

  // The slides follow: n + 1 thumbnails, the new title on slide 2.
  await button(page, 'Thường');
  await expect(thumbs(page)).toHaveCount(n + 1);
  await page.getByTestId('thumb-2').click();
  await expect(page.getByTestId('slide-status')).toHaveText(`Slide 2 / ${n + 1}`);
  // Undo of the last outline step: the bullet is a title again.
  await page.keyboard.press('Control+z');
  await expect(thumbs(page)).toHaveCount(n + 2);
  await page.keyboard.press('Control+y');
  await expect(thumbs(page)).toHaveCount(n + 1);
  expect(errors).toEqual([]);
});

test('notes page view edits the notes of a slide', async ({ page }) => {
  await page.goto('/slide');
  await openDeck(page, deck);
  await viewTab(page);
  await button(page, 'Trang ghi chú');
  await expect(page.getByTestId('notes-page-view')).toBeVisible();
  await expect(page.getByTestId('notes-page-1')).toBeVisible();
  const text = page.getByTestId('notes-page-text-1');
  await text.click();
  await page.keyboard.press('Control+a');
  await page.keyboard.insertText('Ghi chú từ trang ghi chú');
  // Leaving the box saves the notes (one undoable step).
  await page.getByTestId('notes-page-2').click({ position: { x: 4, y: 4 } });
  await button(page, 'Thường');
  await page.getByTestId('thumb-1').click();
  await expect(page.getByTestId('notes-editor')).toHaveValue('Ghi chú từ trang ghi chú');
});

test('sections: add, rename, collapse, move; handout and notes masters; saved', async ({
  page,
}) => {
  await page.goto('/slide');
  await openDeck(page, deck);
  const n = await thumbs(page).count();
  expect(n).toBeGreaterThan(1);
  await page.getByTestId('thumb-2').click();
  await expect(page.getByTestId('slide-status')).toHaveText(`Slide 2 / ${n}`);
  await viewTab(page);
  await button(page, 'Thêm phần');
  await expect(page.getByTestId('section-header-1')).toBeVisible();
  await expect(page.getByTestId('section-name-1')).toContainText('Phần đầu');
  await expect(page.getByTestId('section-name-2')).toContainText('Phần mới');
  await expect(page.getByTestId('section-name-2')).toContainText(`(${n - 1})`);

  // Rename the second section from its header.
  await page.getByTestId('section-name-2').dblclick();
  await expect(page.getByTestId('section-rename-dialog')).toBeVisible();
  await page.getByTestId('section-rename-input').fill('Kết quả');
  await page.getByTestId('section-rename-ok').click();
  await expect(page.getByTestId('section-name-2')).toContainText('Kết quả');

  // Collapse the first section: its slide disappears from the pane.
  await page.getByTestId('section-toggle-1').click();
  await expect(thumbs(page)).toHaveCount(n - 1);
  await page.getByTestId('section-toggle-1').click();
  await expect(thumbs(page)).toHaveCount(n);

  // Move "Phần đầu" below "Kết quả".
  await page.getByTestId('section-name-1').click();
  await button(page, 'Đưa phần xuống');
  await expect(page.getByTestId('section-name-1')).toContainText('Kết quả');
  await expect(page.getByTestId('section-name-2')).toContainText('Phần đầu');

  // Handout master: switch the date off, landscape pages.
  await viewTab(page);
  await button(page, 'Tài liệu phát tay cái');
  await expect(page.getByTestId('handout-slots')).toBeVisible();
  await page.getByRole('tab', { name: 'Tài liệu phát tay cái', exact: true }).click();
  await button(page, 'Ngày');
  await button(page, 'Trang ngang');
  await button(page, '4 slide mỗi trang');
  await expect(page.getByTestId('handout-slot-4')).toBeAttached();
  await button(page, 'Đóng dạng xem trang cái');
  await expect(thumbs(page)).toHaveCount(n);

  // Notes master: add a header placeholder.
  await viewTab(page);
  await button(page, 'Ghi chú cái');
  await expect(thumbs(page)).toHaveCount(1);
  await page.getByRole('tab', { name: 'Ghi chú cái', exact: true }).click();
  await button(page, 'Đầu trang');
  await button(page, 'Đóng dạng xem trang cái');
  await expect(thumbs(page)).toHaveCount(n);

  // Save and reopen: the sections are kept.
  const download = page.waitForEvent('download');
  await page.keyboard.press('Control+s');
  const saved = await (await download).path();
  await page.reload();
  await openDeck(page, saved);
  await expect(page.getByTestId('section-name-1')).toContainText('Kết quả');
  await expect(page.getByTestId('section-name-2')).toContainText('Phần đầu');
  await expect(thumbs(page)).toHaveCount(n);
});
