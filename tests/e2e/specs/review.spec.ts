import { expect, test, type Page } from '@playwright/test';

async function newDocument(page: Page) {
  await page.goto('/doc');
  await page.getByTestId('new-document').click();
  await expect(page.getByTestId('caret')).toBeVisible();
}

const cmd = (page: Page, id: string) => page.locator(`[data-command="${id}"]`);

/** Focus the document (the IME textarea) after using the ribbon or a pane. */
async function focusText(page: Page) {
  const box = (await page.locator('.bo-page[data-page="1"]').boundingBox())!;
  await page.mouse.click(box.x + box.width * 0.5, box.y + box.height * 0.08);
  await page.keyboard.press('Control+End');
}

test('track changes: record edits, reject all, accept all', async ({ page }) => {
  const errors: string[] = [];
  page.on('pageerror', (e) => errors.push(e.message));
  await newDocument(page);
  await page.keyboard.insertText('Hợp đồng số 1');
  await expect(page.getByTestId('status-words')).toHaveText('4 từ');

  // Review tab → Track changes (ECMA-376 Part 1, §17.13.5).
  await page.getByRole('tab', { name: 'Xem lại' }).click();
  await cmd(page, 'review.trackChanges').click();
  await expect(cmd(page, 'review.trackChanges')).toHaveAttribute('aria-pressed', 'true');
  await focusText(page);

  // Replace "1" by "2": a tracked deletion and a tracked insertion.
  await page.keyboard.press('Backspace');
  await page.keyboard.insertText('2 bản');
  await cmd(page, 'review.pane').click();
  await expect(page.getByTestId('review-pane')).toBeVisible();
  await expect(page.getByTestId('revision-count')).toHaveText('2 thay đổi');
  await expect(page.getByTestId('revision')).toHaveCount(2);
  // Deleted text stays visible (struck through) until the change is resolved.
  await expect(page.getByTestId('status-words')).toHaveText('5 từ');

  // Reject all: back to the original text.
  await cmd(page, 'review.rejectAll').click();
  await expect(page.getByTestId('revision-count')).toHaveText('0 thay đổi');
  await expect(page.getByTestId('status-words')).toHaveText('4 từ');

  // New tracked edit, accepted from the pane, then accept all.
  await focusText(page);
  await page.keyboard.press('Backspace');
  await page.keyboard.insertText('3');
  await expect(page.getByTestId('revision-count')).toHaveText('2 thay đổi');
  await page.getByTestId('revision').first().getByRole('button', { name: 'Chấp nhận' }).click();
  await expect(page.getByTestId('revision-count')).toHaveText('1 thay đổi');
  await cmd(page, 'review.acceptAll').click();
  await expect(page.getByTestId('revision-count')).toHaveText('0 thay đổi');

  // The accepted text is the document text now.
  await page.keyboard.press('Control+f');
  await page.getByTestId('find-query').fill('số 3');
  await page.getByTestId('find-query').press('Enter');
  await expect(page.getByTestId('find-message')).toHaveText('1 kết quả');
  expect(errors).toEqual([]);
});

test('track changes: tracked table row insert, kept after reopen, rejected', async ({ page }) => {
  const errors: string[] = [];
  page.on('pageerror', (e) => errors.push(e.message));
  await newDocument(page);
  await page.getByRole('tab', { name: 'Chèn' }).click();
  await cmd(page, 'insert.table').click();
  await page.keyboard.insertText('A');
  await page.getByRole('tab', { name: 'Xem lại' }).click();
  await cmd(page, 'review.trackChanges').click();
  await expect(cmd(page, 'review.trackChanges')).toHaveAttribute('aria-pressed', 'true');
  // Insert a row below the first one (§17.13.5.19 ins in trPr).
  const box = (await page.locator('.bo-page[data-page="1"]').boundingBox())!;
  await page.mouse.click(box.x + box.width * 0.2, box.y + box.height * 0.08);
  await page.getByRole('tab', { name: 'Bảng' }).click();
  await cmd(page, 'table.insertRowBelow').click();
  await page.keyboard.insertText('Mới');
  await page.getByRole('tab', { name: 'Xem lại' }).click();
  await cmd(page, 'review.pane').click();
  await expect(page.getByTestId('review-pane')).toBeVisible();
  await expect(page.getByTestId('revision').filter({ hasText: 'Thêm hàng' })).toHaveCount(1);
  await expect(page.getByTestId('status-words')).toHaveText('2 từ');
  // Saved as w:trPr/w:ins and read back.
  const download = page.waitForEvent('download');
  await page.keyboard.press('Control+s');
  const path = await (await download).path();
  await page.reload();
  const chooser = page.waitForEvent('filechooser');
  await page.getByRole('button', { name: 'Mở tệp từ máy' }).click();
  await (await chooser).setFiles(path);
  await expect(page.locator('.bo-page[data-page="1"]')).toHaveAttribute('data-rendered', 'true');
  await page.getByRole('tab', { name: 'Xem lại' }).click();
  await cmd(page, 'review.pane').click();
  await expect(page.getByTestId('revision').filter({ hasText: 'Thêm hàng' })).toHaveCount(1);
  // Reject all: the inserted row (and its text) is gone.
  await cmd(page, 'review.rejectAll').click();
  await expect(page.getByTestId('revision-count')).toHaveText('0 thay đổi');
  await expect(page.getByTestId('status-words')).toHaveText('1 từ');
  expect(errors).toEqual([]);
});

test('comments: add, reply, resolve and delete', async ({ page }) => {
  await newDocument(page);
  await page.keyboard.insertText('Điều 1. Phạm vi điều chỉnh');
  await page.keyboard.press('Control+a');
  await page.getByRole('tab', { name: 'Xem lại' }).click();
  await cmd(page, 'review.newComment').click();
  await page.getByTestId('comment-input').fill('Làm rõ phạm vi');
  await page.getByTestId('comment-add').click();
  await expect(page.getByTestId('comment')).toHaveCount(1);
  await expect(page.getByTestId('comment-text')).toHaveText('Làm rõ phạm vi');
  await page.getByTestId('comment-reply-button').click();
  await page.getByTestId('comment-reply-input').fill('Đã sửa');
  await page.getByTestId('comment-reply-input').press('Enter');
  await expect(page.getByTestId('comment-reply')).toHaveCount(1);
  await page.getByTestId('comment-resolve').click();
  await expect(page.getByTestId('comment')).toHaveAttribute('data-done', 'true');
  await page.getByTestId('comment-delete').click();
  await expect(page.getByTestId('comment')).toHaveCount(0);
});

test('compare with the original: the differences become tracked changes', async ({ page }) => {
  const errors: string[] = [];
  page.on('pageerror', (e) => errors.push(e.message));
  await newDocument(page);
  await page.keyboard.insertText('Hợp đồng số 1');
  // The original: saved as it is now.
  const download = page.waitForEvent('download');
  await page.keyboard.press('Control+s');
  const original = await (await download).path();
  // Edited afterwards, without tracking.
  await focusText(page);
  await page.keyboard.insertText(' bản sửa');
  await expect(page.getByTestId('status-words')).toHaveText('6 từ');
  await page.getByRole('tab', { name: 'Xem lại' }).click();
  const chooser = page.waitForEvent('filechooser');
  await cmd(page, 'review.compare').click();
  await (await chooser).setFiles(original);
  await cmd(page, 'review.pane').click();
  await expect(page.getByTestId('revision-count')).toHaveText('1 thay đổi');
  // Rejecting the comparison's change gives the original text back.
  await cmd(page, 'review.rejectAll').click();
  await expect(page.getByTestId('status-words')).toHaveText('4 từ');
  expect(errors).toEqual([]);
});

test('combine with another copy: both reviewers changes as tracked changes', async ({ page }) => {
  const errors: string[] = [];
  page.on('pageerror', (e) => errors.push(e.message));
  await newDocument(page);
  await page.keyboard.insertText('Giá 100 triệu');
  // The other reviewer's copy adds " đồng" (no tracking: its file name names the reviewer).
  await page.keyboard.insertText(' đồng');
  const download = page.waitForEvent('download');
  await page.keyboard.press('Control+s');
  const copy = await (await download).path();
  for (let i = 0; i < 5; i++) await page.keyboard.press('Backspace');
  await expect(page.getByTestId('status-words')).toHaveText('3 từ');
  // This reviewer tracks an insertion.
  await page.getByRole('tab', { name: 'Xem lại' }).click();
  await cmd(page, 'review.trackChanges').click();
  await focusText(page);
  await page.keyboard.insertText(' VNĐ');
  await cmd(page, 'review.pane').click();
  await expect(page.getByTestId('revision-count')).toHaveText('1 thay đổi');
  const chooser = page.waitForEvent('filechooser');
  await cmd(page, 'review.combine').click();
  await (await chooser).setFiles(copy);
  await expect(page.getByTestId('revision-count')).toHaveText('2 thay đổi');
  await cmd(page, 'review.rejectAll').click();
  await expect(page.getByTestId('status-words')).toHaveText('3 từ');
  expect(errors).toEqual([]);
});
