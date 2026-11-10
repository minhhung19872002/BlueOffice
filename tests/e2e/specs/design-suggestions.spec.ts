import { expect, test, type Page } from '@playwright/test';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

/**
 * CORE-DESIGN-001: BlueSlide design suggestions (apply a layout, undo, save / reopen) and
 * BlueSheet Quick Analysis (data bars, a total row as one undo step, a recommended chart,
 * save / reopen).
 */

const here = dirname(fileURLToPath(import.meta.url));
const deck = join(here, '..', '..', 'corpus', 'slide', 'slide-text-bullets.pptx');
const cmd = (page: Page, id: string) => page.locator(`[data-command="${id}"]`);

async function openDeck(page: Page, path: string) {
  await page.getByRole('tab', { name: 'Trang đầu' }).click();
  const chooser = page.waitForEvent('filechooser');
  await page.getByRole('button', { name: 'Mở', exact: true }).click();
  await (await chooser).setFiles(path);
  await expect(page.getByTestId('slide-status')).toHaveText(/^Slide 1 \/ \d+$/);
  await expect(page.getByTestId('slide-overlay')).toBeVisible();
}

test('BlueSlide: a design suggestion is applied, undone and saved', async ({ page }) => {
  const errors: string[] = [];
  page.on('pageerror', (e) => errors.push(e.message));
  await page.goto('/slide');
  await openDeck(page, deck);
  await page.getByTestId('thumb-2').click();
  await expect(page.getByTestId('slide-status')).toHaveText('Slide 2 / 3');

  // The objects of the slide in the Selection pane show the accent shapes a layout adds.
  await page.getByRole('tab', { name: 'Sắp xếp' }).click();
  await cmd(page, 'view.selectionPane').click();
  const objects = page.getByTestId('selection-item');
  await expect(objects.first()).toBeVisible();
  const before = await objects.count();
  await expect(objects.filter({ hasText: 'Design Accent' })).toHaveCount(0);

  // Design › Design suggestions: live thumbnails of the layouts for a title + text slide.
  await page.getByRole('tab', { name: 'Thiết kế', exact: true }).click();
  await cmd(page, 'design.ideas').click();
  const pane = page.getByTestId('design-ideas');
  await expect(pane).toBeVisible();
  await expect(pane).toContainText('Tiêu đề và văn bản');
  const ideas = pane.locator('[data-testid^="design-idea-"]');
  expect(await ideas.count()).toBeGreaterThanOrEqual(4);
  await expect(ideas.first()).toHaveAttribute('data-idea', /^bullets\./);

  // Apply the second layout (a rule beside the title column): its accent appears.
  await page.getByTestId('design-idea-1').click();
  await expect(objects.filter({ hasText: 'Design Accent' })).toHaveCount(1);
  await expect(objects).toHaveCount(before + 1);

  // One undo step takes it back; redo applies it again.
  await page.getByTestId('thumb-2').click();
  await page.keyboard.press('Control+z');
  await expect(objects.filter({ hasText: 'Design Accent' })).toHaveCount(0);
  await expect(objects).toHaveCount(before);
  await page.keyboard.press('Control+y');
  await expect(objects.filter({ hasText: 'Design Accent' })).toHaveCount(1);
  // The ideas are those of the slide as it is now (the accent is not content).
  await expect(pane.locator('[data-testid^="design-idea-"]').first()).toHaveAttribute(
    'data-idea',
    /^bullets\./,
  );

  // Saved and reopened: the layout is kept.
  const download = page.waitForEvent('download');
  await page.keyboard.press('Control+s');
  const saved = await (await download).path();
  await openDeck(page, saved);
  await page.getByTestId('thumb-2').click();
  await expect(page.getByTestId('slide-status')).toHaveText('Slide 2 / 3');
  await expect(objects.filter({ hasText: 'Design Accent' })).toHaveCount(1);
  expect(errors).toEqual([]);
});

async function goTo(page: Page, ref: string) {
  const box = page.getByLabel('Hộp tên');
  await box.click();
  await box.fill(ref);
  await box.press('Enter');
  await expect(page.getByLabel('Ô đang nhập')).toBeFocused();
}

async function formulaOf(page: Page, ref: string) {
  await goTo(page, ref);
  return page.getByLabel('Thanh công thức');
}

async function quick(page: Page, tab: string) {
  await goTo(page, 'A1:C4');
  await page.getByTestId('quick-analysis').click();
  await expect(page.getByTestId('quick-analysis-panel')).toBeVisible();
  await page.getByTestId(`qa-tab-${tab}`).click();
}

test('BlueSheet: Quick Analysis data bars, total row and recommended chart', async ({ page }) => {
  const errors: string[] = [];
  page.on('pageerror', (e) => errors.push(e.message));
  await page.goto('/sheet');
  await expect(page.getByTestId('sheet-canvas')).toHaveAttribute('data-rendered', /\d+/);
  const rows = [
    ['Tỉnh', 'Quý 1', 'Quý 2'],
    ['Hà Nội', '10', '12'],
    ['Huế', '5', '4'],
    ['Đà Nẵng', '7', '9'],
  ];
  for (const [i, row] of rows.entries()) {
    await goTo(page, `A${i + 1}`);
    for (const v of row) {
      await page.keyboard.type(v);
      await page.keyboard.press('Tab');
    }
  }
  // One cell: no button; several: the button at the corner of the selection.
  await goTo(page, 'B2');
  await expect(page.getByTestId('quick-analysis')).toHaveCount(0);

  // Formatting › Data bars.
  await quick(page, 'formatting');
  await page.getByTestId('qa-fmt-dataBar').click();
  await expect(page.getByTestId('quick-analysis-panel')).toHaveCount(0);

  // Totals › Sum row: label and formulas below the data, one undo step.
  await quick(page, 'totals');
  await page.getByTestId('qa-total-rows-sum').click();
  await expect(await formulaOf(page, 'B5')).toHaveValue('=SUM(B2:B4)');
  await expect(await formulaOf(page, 'C5')).toHaveValue('=SUM(C2:C4)');
  await expect(await formulaOf(page, 'A5')).toHaveValue('Tổng');
  await page.keyboard.press('Control+z');
  await expect(await formulaOf(page, 'B5')).toHaveValue('');
  await expect(await formulaOf(page, 'A5')).toHaveValue('');
  await page.keyboard.press('Control+y');
  await expect(await formulaOf(page, 'C5')).toHaveValue('=SUM(C2:C4)');

  // Charts: the recommended types with previews; columns first for a few categories.
  await quick(page, 'charts');
  const charts = page.getByTestId('recommended-charts');
  await expect(charts).toBeVisible();
  await expect(charts.getByRole('listitem').first()).toHaveAttribute(
    'data-testid',
    'recommended-chart-column',
  );
  await page.getByTestId('recommended-chart-column').click();
  await expect(page.getByTestId('chart-selection')).toBeVisible();

  // The data bars are a conditional format of B2:C4.
  await goTo(page, 'B2');
  await page.getByRole('tab', { name: 'Trang đầu' }).click();
  await cmd(page, 'format.conditionalManage').click();
  await expect(page.getByTestId('cf-rule-0')).toBeVisible();
  await page.keyboard.press('Escape');

  // Saved and reopened: totals, chart and data bars are kept.
  await page.getByLabel('Ô đang nhập').focus();
  const download = page.waitForEvent('download');
  await page.keyboard.press('Control+s');
  const path = await (await download).path();
  await page.reload();
  const root = page.locator('[data-app="sheet"]');
  await expect(root).toHaveAttribute('aria-busy', 'false');
  const seq = Number(await root.getAttribute('data-load-seq'));
  const chooser = page.waitForEvent('filechooser');
  await cmd(page, 'file.open').click();
  await (await chooser).setFiles(path);
  await expect
    .poll(async () => Number(await root.getAttribute('data-load-seq')))
    .toBeGreaterThan(seq);
  await expect(root).toHaveAttribute('aria-busy', 'false');
  await expect(await formulaOf(page, 'B5')).toHaveValue('=SUM(B2:B4)');
  await page.getByRole('tab', { name: 'Chèn', exact: true }).click();
  await expect(cmd(page, 'chart.edit').first()).toBeEnabled();
  await page.getByRole('tab', { name: 'Trang đầu' }).click();
  await cmd(page, 'format.conditionalManage').click();
  await expect(page.getByTestId('cf-rule-0')).toBeVisible();
  expect(errors).toEqual([]);
});
