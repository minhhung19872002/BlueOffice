import { expect, test, type Page } from '@playwright/test';

async function goTo(page: Page, ref: string) {
  const box = page.getByLabel('Hộp tên');
  await box.click();
  await box.fill(ref);
  await box.press('Enter');
  await expect(page.getByLabel('Ô đang nhập')).toBeFocused();
}

test('BlueSheet: Goal Seek finds an input and applies it as one undo step', async ({ page }) => {
  const errors: string[] = [];
  page.on('pageerror', (e) => errors.push(e.message));
  await page.goto('/sheet');
  await expect(page.getByTestId('sheet-canvas')).toHaveAttribute('data-rendered', /\d+/);
  // A1 = 1, B1 = 2*A1+3.
  await page.keyboard.type('1');
  await page.keyboard.press('Tab');
  await page.keyboard.type('=2*A1+3');
  await page.keyboard.press('Enter');
  await goTo(page, 'B1');
  await page.getByRole('tab', { name: 'Dữ liệu' }).click();
  await page.locator('[data-command="data.goalSeek"]').click();
  await expect(page.getByTestId('goalseek-dialog')).toBeVisible();
  await expect(page.getByTestId('goalseek-target')).toHaveValue('B1');
  await page.getByTestId('goalseek-goal').fill('11');
  await page.getByTestId('goalseek-changing').fill('A1');
  await page.getByTestId('goalseek-find').click();
  await expect(page.getByTestId('goalseek-result')).toContainText('Đã tìm thấy');
  await page.getByTestId('goalseek-apply').click();
  await expect(page.getByTestId('goalseek-dialog')).toHaveCount(0);
  await goTo(page, 'A1');
  await expect(page.getByLabel('Thanh công thức')).toHaveValue('4');
  // Undo restores the input.
  await page.keyboard.press('Control+z');
  await goTo(page, 'A1');
  await expect(page.getByLabel('Thanh công thức')).toHaveValue('1');
  expect(errors).toEqual([]);
});

test('BlueSheet: a one-variable data table fills its results', async ({ page }) => {
  const errors: string[] = [];
  page.on('pageerror', (e) => errors.push(e.message));
  await page.goto('/sheet');
  await expect(page.getByTestId('sheet-canvas')).toHaveAttribute('data-rendered', /\d+/);
  // A1 = 1 (input), B2 = A1*100, rates 2 / 3 / 5 in A3:A5.
  await page.keyboard.type('1');
  await page.keyboard.press('Enter');
  await goTo(page, 'B2');
  await page.keyboard.type('=A1*100');
  await page.keyboard.press('Enter');
  await goTo(page, 'A3');
  for (const v of ['2', '3', '5']) {
    await page.keyboard.type(v);
    await page.keyboard.press('Enter');
  }
  await goTo(page, 'A2:B5');
  await page.getByRole('tab', { name: 'Dữ liệu' }).click();
  await page.locator('[data-command="data.dataTable"]').click();
  await page.getByTestId('datatable-col').fill('A1');
  await page.getByTestId('datatable-ok').click();
  await expect(page.getByTestId('datatable-dialog')).toHaveCount(0);
  await goTo(page, 'B3:B5');
  await expect(page.getByTestId('status-sum')).toHaveText('Tổng: 1.000');
  expect(errors).toEqual([]);
});

test('BlueSheet: Solver maximises a linear model and keeps the solution', async ({ page }) => {
  const errors: string[] = [];
  page.on('pageerror', (e) => errors.push(e.message));
  await page.goto('/sheet');
  await expect(page.getByTestId('sheet-canvas')).toHaveAttribute('data-rendered', /\d+/);
  // Maximise 3x + 5y with x ≤ 4, 2y ≤ 12, 3x + 2y ≤ 18 (x, y in A1:B1).
  await goTo(page, 'C1');
  for (const v of ['=3*A1+5*B1', '=2*B1', '=3*A1+2*B1']) {
    await page.keyboard.type(v);
    await page.keyboard.press('Enter');
  }
  await goTo(page, 'C1');
  await page.getByRole('tab', { name: 'Dữ liệu' }).click();
  await page.locator('[data-command="data.solver"]').click();
  await expect(page.getByTestId('solver-objective')).toHaveValue('C1');
  await page.getByTestId('solver-variables').fill('A1:B1');
  const limits: [string, string][] = [
    ['A1', '4'],
    ['C2', '12'],
    ['C3', '18'],
  ];
  for (const [cell, limit] of limits) {
    await page.getByTestId('solver-add').click();
    const row = page.getByTestId('solver-constraint').last();
    await row.getByLabel('Ô tham chiếu').fill(cell);
    await row.getByLabel('Ràng buộc', { exact: true }).fill(limit);
  }
  await page.getByTestId('solver-solve').click();
  await expect(page.getByTestId('solver-result')).toContainText('mục tiêu 36');
  await page.getByTestId('solver-keep').click();
  await expect(page.getByTestId('solver-dialog')).toHaveCount(0);
  await goTo(page, 'A1:B1');
  await expect(page.getByTestId('status-sum')).toHaveText('Tổng: 8');
  expect(errors).toEqual([]);
});

test('BlueSheet: Analysis histogram with chart output, patterned numbers', async ({ page }) => {
  const errors: string[] = [];
  page.on('pageerror', (e) => errors.push(e.message));
  await page.goto('/sheet');
  await expect(page.getByTestId('sheet-canvas')).toHaveAttribute('data-rendered', /\d+/);
  for (const v of ['1', '2', '2', '3', '3', '3', '4', '5']) {
    await page.keyboard.type(v);
    await page.keyboard.press('Enter');
  }
  await goTo(page, 'A1:A8');
  await page.getByRole('tab', { name: 'Dữ liệu' }).click();
  await page.locator('[data-command="data.analysis"]').click();
  const dialog = page.getByTestId('analysis-dialog');
  await expect(dialog).toBeVisible();
  await page.getByTestId('analysis-tool').selectOption('histogram');
  await page.getByTestId('analysis-cumulative').check();
  await page.getByTestId('analysis-chart').check();
  await page.getByTestId('analysis-ok').click();
  await expect(dialog).toHaveCount(0);
  // ⌈√8⌉ = 3 automatic bins from 1 to 5: 1 2 2 | 3 3 3 | 4 5.
  await goTo(page, 'B2');
  await expect(page.getByLabel('Thanh công thức')).toHaveValue('3');
  await goTo(page, 'B4');
  await expect(page.getByLabel('Thanh công thức')).toHaveValue('2');
  // The chart sits right of the table (A:C), from E1: clicking inside it selects it.
  await goTo(page, 'H6');
  const b = (await page.getByLabel('Ô đang nhập').boundingBox())!;
  await page.keyboard.press('Escape');
  await page.mouse.click(b.x + b.width / 2, b.y + b.height / 2);
  await expect(page.getByTestId('chart-selection')).toBeVisible();
  await page.keyboard.press('Escape');

  // Patterned "random" numbers: 1 to 3 by 1, each twice.
  await goTo(page, 'A20');
  await page.getByRole('tab', { name: 'Dữ liệu' }).click();
  await page.locator('[data-command="data.analysis"]').click();
  await page.getByTestId('analysis-tool').selectOption('random');
  await page.getByTestId('analysis-dist').selectOption('patterned');
  await expect(page.getByTestId('analysis-count')).toHaveCount(0);
  await page.getByTestId('analysis-p1').fill('1');
  await page.getByTestId('analysis-p2').fill('3');
  await page.getByTestId('analysis-p3').fill('1');
  await page.getByTestId('analysis-p4').fill('2');
  await page.getByTestId('analysis-p5').fill('1');
  await page.getByTestId('analysis-ok').click();
  await expect(page.getByTestId('analysis-dialog')).toHaveCount(0);
  await goTo(page, 'A1:A6');
  await expect(page.getByTestId('status-sum')).toHaveText('Tổng: 12');
  await goTo(page, 'A5');
  await expect(page.getByLabel('Thanh công thức')).toHaveValue('3');
  await goTo(page, 'A7');
  await expect(page.getByLabel('Thanh công thức')).toHaveValue('');
  expect(errors).toEqual([]);
});
