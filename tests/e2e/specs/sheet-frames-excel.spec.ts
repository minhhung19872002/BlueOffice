import { expect, test, type Page } from '@playwright/test';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

/**
 * Slicers, timelines and new charts behaving like a spreadsheet application (SHEET-SLICER-001,
 * SHEET-TIMELINE-001, CORE-CHART-002/004): Shift click and Multi-Select on a slicer, a drag over
 * timeline periods, Timeline Tools › Show, the workbook's own slicer / timeline styles, the look of
 * a chart inserted from Insert › Charts, and a slide chart turned into a chartex type.
 */
const here = dirname(fileURLToPath(import.meta.url));
const corpus = (dir: string, name: string) => join(here, '..', '..', 'corpus', dir, name);
const cmd = (page: Page, id: string) => page.locator(`[data-command="${id}"]`);

async function openFile(page: Page, path: string) {
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
}

async function goTo(page: Page, ref: string) {
  const box = page.getByLabel('Hộp tên');
  await box.click();
  await box.fill(ref);
  await box.press('Enter');
  await expect(page.getByLabel('Ô đang nhập')).toBeFocused();
}

/** Texts of the chosen items of a slicer (or periods of a timeline strip). */
const pressed = (frame: ReturnType<Page['getByTestId']>) =>
  frame
    .locator(
      '[data-testid="slicer-item"][aria-pressed="true"], button[aria-pressed="true"]:not([data-testid])',
    )
    .evaluateAll((b) => b.map((x) => x.textContent ?? ''));

test('BlueSheet: slicer Shift click and Multi-Select, timeline drag and Show options, custom styles', async ({
  page,
}) => {
  const errors: string[] = [];
  page.on('pageerror', (e) => errors.push(e.message));
  await page.goto('/sheet');
  await openFile(page, corpus('sheet', 'writer-frame-styles-1.xlsx'));
  await page.getByRole('tab', { name: 'Báo cáo' }).click();

  // The workbook's own slicer style: its frame fill and the fill of the items not chosen.
  const slicer = page.getByTestId('slicer');
  await expect(slicer).toBeVisible();
  await expect(slicer).toHaveAttribute('data-style', 'Kiểu bộ lọc Bluestar');
  expect(await slicer.evaluate((el) => el.style.getPropertyValue('--frame-bg'))).toBe('#F2F7FC');
  expect(await slicer.evaluate((el) => el.style.getPropertyValue('--frame-on'))).toBe('#2E75B6');

  // Every region is chosen; a click keeps one, Shift click takes the range up to another.
  const items = await slicer
    .getByTestId('slicer-item')
    .evaluateAll((b) => b.map((x) => x.textContent ?? ''));
  expect(items.length).toBeGreaterThanOrEqual(3);
  await slicer.getByRole('button', { name: items[0]!, exact: true }).click();
  await expect.poll(() => pressed(slicer)).toEqual([items[0]]);
  await slicer
    .getByRole('button', { name: items[2]!, exact: true })
    .click({ modifiers: ['Shift'] });
  await expect.poll(() => pressed(slicer)).toEqual(items.slice(0, 3));
  // Multi-Select: a plain click now takes an item out.
  await slicer.getByTestId('slicer-multi').click();
  await expect(slicer.getByTestId('slicer-multi')).toHaveAttribute('aria-pressed', 'true');
  await slicer.getByRole('button', { name: items[1]!, exact: true }).click();
  await expect.poll(() => pressed(slicer)).toEqual([items[0], items[2]]);
  // Alt+C clears the filter while the slicer has the focus.
  await slicer.focus();
  await page.keyboard.press('Alt+c');
  await expect.poll(() => pressed(slicer)).toEqual(items);

  // The timeline of this file hides its time level and its scroll bar.
  const tl = page.getByTestId('timeline');
  await expect(tl).toBeVisible();
  await expect(tl.getByTestId('timeline-level')).toHaveCount(0);
  await expect(tl.getByTestId('timeline-strip')).toHaveAttribute('data-scrollbar', 'hidden');
  // The file scrolls the strip to July 2023 (scrollPosition): it does not start at the first
  // month.
  const strip = tl.getByTestId('timeline-strip');
  await expect.poll(() => strip.evaluate((el) => el.scrollLeft)).toBeGreaterThan(0);
  // A drag over three months in view chooses them.
  const k = await strip.evaluate((el) => {
    const left = el.getBoundingClientRect().left;
    return Array.from(el.children).findIndex((b) => b.getBoundingClientRect().left >= left);
  });
  const months = strip.getByRole('button');
  const first = await months.nth(k).boundingBox();
  const third = await months.nth(k + 2).boundingBox();
  await page.mouse.move(first!.x + first!.width / 2, first!.y + first!.height / 2);
  await page.mouse.down();
  await page.mouse.move(third!.x + third!.width / 2, third!.y + third!.height / 2, { steps: 6 });
  await page.mouse.up();
  await expect.poll(async () => (await pressed(strip)).length).toBe(3);
  await expect(tl.getByTestId('timeline-clear')).toBeEnabled();

  // Timeline Tools › Show: the time level comes back.
  await tl.getByTestId('timeline-settings').click();
  const dlg = page.getByTestId('frame-options-dialog');
  await expect(dlg).toBeVisible();
  await expect(dlg.getByTestId('timeline-show-timeLevel')).not.toBeChecked();
  await dlg.getByTestId('timeline-show-timeLevel').click();
  await dlg.getByTestId('frame-options-ok').click();
  await expect(dlg).toBeHidden();
  await expect(tl.getByTestId('timeline-level')).toBeVisible();
  // Undo hides it again (one step).
  await page.getByRole('button', { name: 'Hoàn tác' }).first().click();
  await expect(tl.getByTestId('timeline-level')).toHaveCount(0);
  expect(errors).toEqual([]);
});

test('BlueSheet: Insert › Charts gives the default look (title, legend at the bottom)', async ({
  page,
}) => {
  const errors: string[] = [];
  page.on('pageerror', (e) => errors.push(e.message));
  await page.goto('/sheet');
  await expect(page.getByTestId('sheet-canvas')).toHaveAttribute('data-rendered', /\d+/);
  const rows = [
    ['Tỉnh', '2024', '2025'],
    ['HN', '10', '12'],
    ['HUE', '5', '4'],
  ];
  for (const [i, row] of rows.entries()) {
    await goTo(page, `A${i + 1}`);
    for (const v of row) {
      await page.keyboard.type(v);
      await page.keyboard.press('Tab');
    }
  }
  // Two series: placeholder title, legend at the bottom.
  await goTo(page, 'A1:C3');
  await page.getByRole('tab', { name: 'Chèn' }).click();
  await cmd(page, 'insert.chart').click();
  await page.getByTestId('menu-chart').locator('[data-menu="column"]').click();
  await expect(page.getByTestId('chart-selection')).toBeVisible();
  await page.getByRole('tab', { name: 'Chèn' }).click();
  await cmd(page, 'chart.edit').click();
  const dlg = page.getByTestId('chart-dialog');
  await expect(dlg.getByTestId('chart-title')).toHaveValue('Tiêu đề biểu đồ');
  await expect(dlg.getByTestId('chart-legend')).toHaveValue('b');
  await page.keyboard.press('Escape');
  await expect(dlg).toBeHidden();
  // One series: its name is the title and there is no legend.
  await page.keyboard.press('Escape');
  await goTo(page, 'A1:B3');
  await page.getByRole('tab', { name: 'Chèn' }).click();
  await cmd(page, 'insert.chart').click();
  await page.getByTestId('menu-chart').locator('[data-menu="line"]').click();
  await expect(page.getByTestId('chart-selection')).toBeVisible();
  await page.getByRole('tab', { name: 'Chèn' }).click();
  await cmd(page, 'chart.edit').click();
  await expect(dlg.getByTestId('chart-title')).toHaveValue('2024');
  await expect(dlg.getByTestId('chart-legend')).toHaveValue('none');
  expect(errors).toEqual([]);
});

test('BlueSlide: a column chart becomes a waterfall (chartex) on the same frame', async ({
  page,
}) => {
  const errors: string[] = [];
  page.on('pageerror', (e) => errors.push(e.message));
  await page.goto('/slide');
  const chooser = page.waitForEvent('filechooser');
  await page.getByRole('button', { name: 'Mở', exact: true }).click();
  await (await chooser).setFiles(corpus('slide', 'slide-text-bullets.pptx'));
  await expect(page.getByTestId('slide-status')).toHaveText(/^Slide 1 \/ \d+$/);
  await expect(page.getByTestId('slide-overlay')).toBeVisible();
  await page.getByRole('button', { name: '+ Thêm slide' }).click();
  await expect(page.getByTestId('slide-status')).toHaveText(/^Slide 2 \/ \d+$/);
  await page.getByRole('tab', { name: 'Chèn' }).click();
  await page.getByRole('button', { name: 'Biểu đồ', exact: true }).click();
  await expect(page.getByTestId('chart-dialog')).toBeVisible();
  await page.getByTestId('chart-title').fill('Dòng tiền');
  await page.getByTestId('chart-ok').click();
  await expect(page.getByTestId('chart-dialog')).toBeHidden();
  const box = await page.getByTestId('selection-box').boundingBox();

  await page.getByRole('tab', { name: 'Biểu đồ' }).click();
  await page.getByText('Sửa biểu đồ').click();
  await expect(page.getByTestId('chart-kind')).toHaveValue('column');
  await page.getByTestId('chart-kind').selectOption('waterfall');
  await page.getByTestId('chart-ok').click();
  await expect(page.getByTestId('chart-dialog')).toBeHidden();
  // Still one selected frame in the same place, now a waterfall with the same title.
  await expect(page.getByTestId('selection-box')).toHaveCount(1);
  expect(await page.getByTestId('selection-box').boundingBox()).toEqual(box);
  await page.getByRole('tab', { name: 'Biểu đồ' }).click();
  await page.getByText('Sửa biểu đồ').click();
  await expect(page.getByTestId('chart-kind')).toHaveValue('waterfall');
  await expect(page.getByTestId('chart-title')).toHaveValue('Dòng tiền');
  await page.getByTestId('chart-ok').click();
  expect(errors).toEqual([]);
});
