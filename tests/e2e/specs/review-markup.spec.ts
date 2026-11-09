import { expect, test, type Page } from '@playwright/test';

// Review display modes (DOC-REVIEW-001): change bars, balloons in the markup area,
// comment icons, markup options and the docked change list.

async function newDocument(page: Page) {
  await page.goto('/doc');
  await page.getByTestId('new-document').click();
  await expect(page.getByTestId('caret')).toBeVisible();
}

const cmd = (page: Page, id: string) => page.locator(`[data-command="${id}"]`);
const page1 = (page: Page) => page.locator('.bo-page[data-page="1"]');

/** Focus the document (the IME textarea) after using the ribbon or a pane. */
async function focusText(page: Page) {
  const box = (await page1(page).boundingBox())!;
  await page.mouse.click(box.x + box.width * 0.5, box.y + box.height * 0.08);
  await page.keyboard.press('Control+End');
}

/** The Tracking group's control (opening the group when the ribbon collapsed it). */
async function tracking(page: Page, testId: string) {
  const el = page.getByTestId(testId);
  if (!(await el.isVisible())) await page.getByTestId('ribbon-group-tracking').click();
  return el;
}

async function mode(page: Page, id: string) {
  await (await tracking(page, 'markup-mode')).selectOption(id);
}

test('markup modes: bars, balloons, icons, options and the change list', async ({ page }) => {
  const errors: string[] = [];
  page.on('pageerror', (e) => errors.push(e.message));
  await newDocument(page);
  await page.keyboard.insertText('Hợp đồng số 1');
  await page.getByRole('tab', { name: 'Xem lại' }).click();
  await cmd(page, 'review.trackChanges').click();
  await focusText(page);
  // A tracked deletion ("1") and insertion ("2 bản").
  await page.keyboard.press('Backspace');
  await page.keyboard.insertText('2 bản');

  // Every change shown (the default): one change bar beside the changed line.
  const bars = page1(page).getByTestId('change-bar');
  await expect(bars).toHaveCount(1);
  await expect(page1(page).getByTestId('markup-balloon')).toHaveCount(0);

  // A comment opens the markup area with its balloon and connector.
  await page.keyboard.press('Control+a');
  await cmd(page, 'review.newComment').click();
  await page.getByTestId('comment-input').fill('Làm rõ số hợp đồng');
  await page.getByTestId('comment-add').click();
  await expect(page.getByTestId('comment')).toHaveCount(1);
  // Close the comments pane so it does not cover the markup area.
  await cmd(page, 'review.comments').click();
  await expect(page.getByTestId('comments-pane')).toHaveCount(0);
  const balloon = page1(page).getByTestId('markup-balloon');
  await expect(balloon).toHaveCount(1);
  await expect(balloon).toContainText('Làm rõ số hợp đồng');
  await expect(page1(page)).toHaveAttribute('data-markup', 'true');
  await expect(page1(page).getByTestId('markup-area')).toBeVisible();
  await expect(page1(page).getByTestId('balloon-connector')).toHaveCount(1);
  // The balloon sits right of the page.
  const pageBox = (await page1(page).boundingBox())!;
  const balloonBox = (await balloon.boundingBox())!;
  expect(balloonBox.x).toBeGreaterThanOrEqual(pageBox.x + pageBox.width - 1);

  // Clean view with margin bars: the comment becomes an icon that opens its balloon.
  await mode(page, 'review.markup.simple');
  const icon = page1(page).getByTestId('markup-icon');
  await expect(icon).toHaveCount(1);
  await expect(balloon).toHaveCount(0);
  await expect(bars).toHaveCount(1);
  await icon.click();
  await expect(balloon).toHaveCount(1);
  await expect(icon).toHaveAttribute('aria-expanded', 'true');

  // Accepted text without marks, then the text before the changes: nothing beside it.
  await mode(page, 'review.markup.none');
  await expect(bars).toHaveCount(0);
  await expect(icon).toHaveCount(0);
  await expect(page1(page)).toHaveAttribute('data-markup', 'false');
  await mode(page, 'review.markup.original');
  await expect(bars).toHaveCount(0);

  // Back to every change; comments hidden from the "marks to show" menu.
  await mode(page, 'review.markup.all');
  await expect(balloon).toHaveCount(1);
  await (await tracking(page, 'show-markup')).click();
  await page.locator('[data-command="review.markup.comments"] input').click();
  await expect(balloon).toHaveCount(0);
  await expect(bars).toHaveCount(1);
  // Removed text moved into balloons.
  await page.locator('[data-command="review.markup.balloons"] input').click();
  await expect(
    page1(page).locator('[data-testid="markup-balloon"][data-kind="delete"]'),
  ).toHaveCount(1);
  await page.keyboard.press('Escape');

  // The docked change list: counts by type, accept from the list, dock at the bottom.
  await cmd(page, 'review.reviewingPane').click();
  const pane = page.getByTestId('reviewing-pane');
  await expect(pane).toBeVisible();
  await expect(pane).toHaveAttribute('data-dock', 'left');
  await expect(page.getByTestId('reviewing-count-insert')).toHaveText('Thêm: 1');
  await expect(page.getByTestId('reviewing-count-delete')).toHaveText('Xoá: 1');
  await expect(page.getByTestId('reviewing-count-comment')).toHaveText('Chú thích: 1');
  await expect(page.getByTestId('reviewing-item')).toHaveCount(3);
  await page.getByTestId('reviewing-accept').first().click();
  await expect(page.getByTestId('reviewing-item')).toHaveCount(2);
  await page.getByTestId('reviewing-dock').click();
  await expect(pane).toHaveAttribute('data-dock', 'bottom');
  expect(errors).toEqual([]);
});
