import { expect, test, type Page } from '@playwright/test';

/**
 * CORE-CHART-006: a country map chart ("Biểu đồ bản đồ") inserted from a small table through
 * Insert › Chart, recommended for country names, its map options changed, saved and reopened; a
 * province map says it needs the approved boundary dataset.
 */

async function goTo(page: Page, ref: string) {
  const box = page.getByLabel('Hộp tên');
  await box.click();
  await box.fill(ref);
  await box.press('Enter');
  await expect(page.getByLabel('Ô đang nhập')).toBeFocused();
}

const cmd = (page: Page, id: string) => page.locator(`[data-command="${id}"]`);

async function openChartDialog(page: Page) {
  await page.getByRole('tab', { name: 'Chèn' }).click();
  await cmd(page, 'chart.edit').click();
  const dlg = page.getByTestId('chart-dialog');
  await expect(dlg).toBeVisible();
  return dlg;
}

test('BlueSheet: insert a country map, set its options, save and reopen', async ({ page }) => {
  const errors: string[] = [];
  page.on('pageerror', (e) => errors.push(e.message));
  await page.goto('/sheet');
  await expect(page.getByTestId('sheet-canvas')).toHaveAttribute('data-rendered', /\d+/);
  const rows = [
    ['Quốc gia', 'Dân số'],
    ['Việt Nam', '98'],
    ['Nhật Bản', '124'],
    ['Brazil', '203'],
  ];
  for (const [i, row] of rows.entries()) {
    await goTo(page, `A${i + 1}`);
    for (const v of row) {
      await page.keyboard.type(v);
      await page.keyboard.press('Tab');
    }
  }

  // Recommended charts put the map first for a column of country names and numbers.
  await goTo(page, 'A1:B4');
  await page.getByRole('tab', { name: 'Chèn' }).click();
  await cmd(page, 'insert.chartRecommended').click();
  await expect(page.getByTestId('recommended-chart-regionMap')).toBeVisible();
  await page.keyboard.press('Escape');

  // Insert › Chart › Biểu đồ bản đồ.
  await goTo(page, 'A1:B4');
  await page.getByRole('tab', { name: 'Chèn' }).click();
  await cmd(page, 'insert.chart').click();
  const menu = page.getByTestId('menu-chart');
  await expect(menu.locator('[data-menu="regionMap"]')).toHaveText('Biểu đồ bản đồ');
  await menu.locator('[data-menu="regionMap"]').click();
  await expect(page.getByTestId('chart-selection')).toBeVisible();

  // The chart dialog shows the map options of the series.
  let dlg = await openChartDialog(page);
  await expect(dlg.getByTestId('map-options')).toBeVisible();
  await expect(dlg.getByTestId('map-level')).toHaveValue('countryRegion');
  await expect(dlg.getByTestId('map-needs-dataset')).toHaveCount(0);
  await dlg.getByTestId('map-projection').selectOption('robinson');
  await dlg.getByTestId('map-labels').selectOption('showAll');
  await dlg.getByTestId('map-scale').selectOption('diverging');
  await page.getByTestId('chart-ok').click();
  await expect(dlg).toBeHidden();

  // Save (Ctrl+S) and reopen: the options are those of the file.
  await page.getByLabel('Ô đang nhập').focus();
  const download = page.waitForEvent('download');
  await page.keyboard.press('Control+s');
  const path = await (await download).path();
  await page.reload();
  await expect(page.getByTestId('sheet-canvas')).toHaveAttribute('data-rendered', /\d+/);
  const chooser = page.waitForEvent('filechooser');
  await cmd(page, 'file.open').click();
  await (await chooser).setFiles(path);
  await expect(page.getByLabel('Thanh công thức')).toBeVisible();
  dlg = await openChartDialog(page);
  await expect(dlg.getByRole('combobox', { name: 'Loại', exact: true })).toHaveValue('regionMap');
  await expect(dlg.getByTestId('map-projection')).toHaveValue('robinson');
  await expect(dlg.getByTestId('map-labels')).toHaveValue('showAll');
  await expect(dlg.getByTestId('map-scale')).toHaveValue('diverging');

  // Provinces need the approved dataset: the dialog says so.
  await dlg.getByTestId('map-level').selectOption('state');
  await page.getByTestId('chart-ok').click();
  dlg = await openChartDialog(page);
  await expect(dlg.getByTestId('map-needs-dataset')).toHaveText(
    'Bản đồ cấp tỉnh cần bộ dữ liệu ranh giới tỉnh/thành được phê duyệt (assets/maps/vn-provinces).',
  );
  expect(errors).toEqual([]);
});
