import { expect, test, type Page } from '@playwright/test';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

// Track changes for table structure, headers/footers, notes and floating pictures,
// and footnotes continued on the next page (ECMA-376 Part 1, §17.13.5, §17.11).

const corpus = join(dirname(fileURLToPath(import.meta.url)), '..', '..', 'corpus', 'doc');
const cmd = (page: Page, id: string) => page.locator(`[data-command="${id}"]`);
const pageN = (page: Page, n: number) => page.locator(`.bo-page[data-page="${n}"]`);

async function newDocument(page: Page) {
  await page.goto('/doc');
  await page.getByTestId('new-document').click();
  await expect(page.getByTestId('caret')).toBeVisible();
}

async function openCorpus(page: Page, name: string) {
  await page.goto('/doc');
  const chooser = page.waitForEvent('filechooser');
  await page.getByRole('button', { name: 'Mở tệp từ máy' }).click();
  await (await chooser).setFiles(join(corpus, name));
  await expect(pageN(page, 1)).toHaveAttribute('data-rendered', 'true');
}

async function trackOn(page: Page) {
  await page.getByRole('tab', { name: 'Xem lại' }).click();
  await cmd(page, 'review.trackChanges').click();
  await expect(cmd(page, 'review.trackChanges')).toHaveAttribute('aria-pressed', 'true');
}

async function reviewPane(page: Page) {
  await page.getByRole('tab', { name: 'Xem lại' }).click();
  if (!(await page.getByTestId('review-pane').isVisible())) await cmd(page, 'review.pane').click();
  await expect(page.getByTestId('review-pane')).toBeVisible();
}

async function caretBox(page: Page) {
  for (let i = 0; i < 40; i++) {
    const b = await page.getByTestId('caret').boundingBox();
    if (b) return b;
    await page.waitForTimeout(50);
  }
  throw new Error('no caret');
}

test('tracked column insert and cell delete: listed, rejected', async ({ page }) => {
  const errors: string[] = [];
  page.on('pageerror', (e) => errors.push(e.message));
  await newDocument(page);
  await page.getByRole('tab', { name: 'Chèn' }).click();
  await cmd(page, 'insert.table').click();
  await page.keyboard.insertText('A');
  await trackOn(page);
  const box = (await pageN(page, 1).boundingBox())!;
  await page.mouse.click(box.x + box.width * 0.2, box.y + box.height * 0.08);
  await page.getByRole('tab', { name: 'Bảng' }).click();
  // §17.13.5.4 cellIns in every row, §17.13.5.3 cellDel for the deleted cell.
  await cmd(page, 'table.insertColumnRight').click();
  // The first tracked change adds the markup area beside the page: measure it again.
  let box2 = (await pageN(page, 1).boundingBox())!;
  for (let i = 0; i < 20; i++) {
    await page.waitForTimeout(100);
    const b = (await pageN(page, 1).boundingBox())!;
    if (b.x === box2.x && b.width === box2.width) break;
    box2 = b;
  }
  await page.mouse.click(box2.x + box2.width * 0.15, box2.y + box2.height * 0.08);
  await cmd(page, 'table.deleteCell').click();
  await reviewPane(page);
  await expect(page.getByTestId('revision').filter({ hasText: 'Thêm ô/cột' })).toHaveCount(1);
  await expect(page.getByTestId('revision').filter({ hasText: 'Xoá ô/cột' })).toHaveCount(1);
  // The deleted cell's text is still counted until the change is accepted.
  await expect(page.getByTestId('status-words')).toHaveText('1 từ');
  await cmd(page, 'review.rejectAll').click();
  await expect(page.getByTestId('revision-count')).toHaveText('0 thay đổi');
  await expect(page.getByTestId('status-words')).toHaveText('1 từ');
  expect(errors).toEqual([]);
});

test('tracked header edit is listed with its story and rejected', async ({ page }) => {
  await newDocument(page);
  await page.keyboard.insertText('Thân văn bản');
  const box = (await pageN(page, 1).boundingBox())!;
  await page.mouse.dblclick(box.x + box.width * 0.4, box.y + 30);
  await expect(page.getByTestId('hf-bar')).toBeVisible();
  await page.keyboard.insertText('Đầu trang gốc');
  await page.getByTestId('hf-close').click();
  await trackOn(page);
  // The ribbon switched tabs: measure the page again before double clicking its header.
  const box2 = (await pageN(page, 1).boundingBox())!;
  await page.mouse.dblclick(box2.x + box2.width * 0.4, box2.y + 30);
  await expect(page.getByTestId('hf-bar')).toBeVisible();
  await page.keyboard.press('End');
  await page.keyboard.insertText(' sửa');
  await page.getByTestId('hf-close').click();
  await expect(page.getByTestId('hf-bar')).toHaveCount(0);
  await reviewPane(page);
  const rev = page.getByTestId('revision').filter({ hasText: 'Đầu trang' });
  await expect(rev).toHaveCount(1);
  await expect(rev.getByTestId('revision-story')).toContainText('Đầu trang');
  await rev.getByRole('button', { name: 'Từ chối' }).click();
  await expect(page.getByTestId('revision-count')).toHaveText('0 thay đổi');
});

test('floating picture moved with track changes: moveFrom/moveTo, rejected', async ({ page }) => {
  const errors: string[] = [];
  page.on('pageerror', (e) => errors.push(e.message));
  await openCorpus(page, 'images-04-wrap-square-sides.docx');
  await trackOn(page);
  await page.getByRole('tab', { name: 'Bố cục' }).click();
  await expect(cmd(page, 'picture.position')).toBeEnabled();
  await cmd(page, 'picture.position').click();
  await expect(page.getByTestId('picture-position')).toBeVisible();
  await page.getByTestId('picture-x').fill('3');
  await page.getByTestId('picture-position-ok').click();
  await reviewPane(page);
  await expect(page.getByTestId('revision').filter({ hasText: 'Chuyển đi' })).toHaveCount(1);
  await expect(page.getByTestId('revision').filter({ hasText: 'Chuyển đến' })).toHaveCount(1);
  await cmd(page, 'review.rejectAll').click();
  await expect(page.getByTestId('revision-count')).toHaveText('0 thay đổi');
  expect(errors).toEqual([]);
});

test('long footnote continues on the next page and can be edited there', async ({ page }) => {
  await openCorpus(page, 'notes-05-long-footnote.docx');
  // The note starts on page 2 and continues at the bottom of page 3 (§17.11.1).
  const p3 = pageN(page, 3);
  // Bring the bottom of page 3 into view.
  await p3.evaluate((el) => el.scrollIntoView({ block: 'end' }));
  await expect(p3).toHaveAttribute('data-rendered', 'true');
  await page.waitForTimeout(300);
  const b = (await p3.boundingBox())!;
  // A click on the continued part edits the note in place on page 3.
  await page.mouse.click(b.x + b.width * 0.4, b.y + b.height * 0.88);
  await expect
    .poll(async () => {
      const c = await caretBox(page);
      const now = (await p3.boundingBox())!;
      return (c.y - now.y) / now.height;
    })
    .toBeGreaterThan(0.7);
});
