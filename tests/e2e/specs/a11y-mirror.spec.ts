import AxeBuilder from '@axe-core/playwright';
import { expect, test, type Page } from '@playwright/test';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

// Screen-reader access to document content (WCAG 2.2 1.3.1, 4.1.2): the pages, the grid and
// the slides are pixels on canvases, so each app keeps a visually hidden mirror in the
// accessibility tree — checked here through Playwright's ARIA snapshot of the tree — and
// announces caret / cell / shape changes in a polite live region. axe scans the mirrors too.

const corpus = join(dirname(fileURLToPath(import.meta.url)), '..', '..', 'corpus');
const TAGS = ['wcag2a', 'wcag2aa', 'wcag21a', 'wcag21aa', 'wcag22aa'];

async function axe(page: Page, include: string) {
  const r = await new AxeBuilder({ page }).withTags(TAGS).include(include).analyze();
  const list = r.violations.map((v) => `${v.id}: ${v.help}`);
  expect(list, list.join('\n')).toEqual([]);
}

async function openDoc(page: Page, file: string) {
  await page.goto('/doc');
  const chooser = page.waitForEvent('filechooser');
  await page.getByRole('button', { name: 'Mở tệp từ máy' }).click();
  await (await chooser).setFiles(join(corpus, 'doc', file));
  await expect(page.locator('.bo-page[data-page="1"]')).toHaveAttribute('data-rendered', 'true');
  return page.getByTestId('doc-a11y-mirror');
}

test('BlueDoc: headings, paragraphs, table cells, pictures and links are in the accessibility tree', async ({
  page,
}) => {
  let mirror = await openDoc(page, 'styles-01-headings.docx');
  // Hidden from sight, present for assistive technology.
  await expect(mirror).toBeAttached();
  expect(await mirror.evaluate((e) => getComputedStyle(e).display)).not.toBe('none');
  await expect(mirror).toHaveAttribute('aria-label', 'Văn bản trang 1');
  await expect(mirror).toMatchAriaSnapshot(`
    - region "Văn bản trang 1":
      - heading "Chương I" [level=1]
      - paragraph: 1 Căn cứ Luật Tổ chức Chính phủ ngày 19 tháng 6 năm 2015;
  `);
  await axe(page, '[data-testid=doc-a11y-mirror]');

  // The caret's paragraph is the one described to the typing box; moves are announced.
  const input = page.getByTestId('ime-input');
  await input.focus();
  await page.keyboard.press('Control+Home');
  const current = mirror.locator('[aria-current="true"]');
  await expect(current).toHaveText('Chương I');
  await expect(input).toHaveAttribute('aria-describedby', (await current.getAttribute('id'))!);
  const live = page.getByTestId('doc-a11y-live');
  await expect(live).toHaveAttribute('aria-live', 'polite');
  await page.keyboard.press('Control+ArrowRight');
  await expect(live).toHaveText('I');
  await page.keyboard.press('ArrowDown');
  await expect(live).toHaveText(/Căn cứ Luật Tổ chức/);
  await expect(current).toHaveText(/^1 Căn cứ Luật/);
  // Typing and deleting are announced.
  await page.keyboard.press('Home');
  await page.keyboard.type('X');
  await expect(live).toHaveText('X');
  await page.keyboard.press('Backspace');
  await expect(live).toHaveText('Đã xoá X');

  // A table with a header row: column headers and cells.
  mirror = await openDoc(page, 'tables-01-header-rows-long.docx');
  await expect(mirror.getByRole('columnheader', { name: 'Họ và tên' }).first()).toBeAttached();
  await expect(mirror.getByRole('cell', { name: 'Nguyễn Văn A' }).first()).toBeAttached();
  expect(await mirror.ariaSnapshot()).toContain('- columnheader "STT"');
  await axe(page, '[data-testid=doc-a11y-mirror]');

  // Pictures by their alternative text; hyperlinks.
  mirror = await openDoc(page, 'images-01-inline.docx');
  await expect(mirror.getByRole('img', { name: 'Ảnh: Ảnh minh hoạ' }).first()).toBeAttached();
  mirror = await openDoc(page, 'links-01-external.docx');
  await expect(mirror.getByRole('link', { name: 'trang thông tin' })).toBeAttached();
});

test('BlueSheet: the grid and the active cell are in the accessibility tree', async ({ page }) => {
  await page.goto('/sheet');
  await expect(page.getByTestId('sheet-canvas')).toHaveAttribute('data-rendered', /\d+/);
  const cellInput = page.getByLabel('Ô đang nhập');
  await cellInput.focus();
  await page.keyboard.type('41');
  await page.keyboard.press('Enter');
  await page.keyboard.type('=A1+1');
  await page.keyboard.press('Enter');
  await page.keyboard.press('ArrowUp');
  const live = page.getByTestId('sheet-a11y-live');
  await expect(live).toHaveText('A2: 42, công thức =A1+1');
  const grid = page.getByRole('grid', { name: 'Lưới bảng tính' });
  await expect(grid.getByRole('gridcell', { name: '41' })).toBeAttached();
  await expect(grid.locator('[aria-selected="true"]')).toHaveAttribute('data-cell', 'A2');
  await expect(grid).toMatchAriaSnapshot(`
    - grid:
      - row:
        - columnheader
        - columnheader "A"
  `);
  // Bold is part of the summary.
  await page.keyboard.press('Control+b');
  await expect(live).toHaveText('A2: 42, công thức =A1+1, đậm');
  await axe(page, '[data-testid=sheet-a11y-grid]');
});

test("BlueSlide: a slide's title and shapes are in the accessibility tree", async ({ page }) => {
  await page.goto('/slide');
  const chooser = page.waitForEvent('filechooser');
  await page.getByRole('button', { name: 'Mở', exact: true }).click();
  await (await chooser).setFiles(join(corpus, 'slide', 'slide-text-bullets.pptx'));
  await expect(page.getByTestId('slide-overlay')).toBeVisible();
  const mirror = page.getByTestId('slide-a11y-mirror');
  const title = mirror.getByRole('heading', { level: 2 });
  await expect(title).toBeAttached();
  const text = (await title.textContent())!.trim();
  expect(text.length).toBeGreaterThan(0);
  // The thumbnail is named after the slide's title.
  await expect(page.getByTestId('thumb-1')).toHaveAccessibleName(`Slide 1: ${text}`);
  expect(await mirror.ariaSnapshot()).toContain(`- heading "${text}" [level=2]`);
  await axe(page, '[data-testid=slide-a11y-mirror]');
  // Selecting a shape is announced.
  await page.getByTestId('slide-overlay').click({ position: { x: 5, y: 5 } });
  await page.keyboard.press('Control+a');
  await expect(page.getByTestId('slide-a11y-live')).toHaveText(/^Đã chọn /);
});
