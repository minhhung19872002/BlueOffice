import { expect, test, type Page } from '@playwright/test';

// CORE-PRINT-001: the shared Print panel — BlueDoc selection and paper orientation,
// BlueSheet several selected sheets (Ctrl + click on their tabs).

async function newDocument(page: Page) {
  await page.goto('/doc');
  await page.getByTestId('new-document').click();
  await expect(page.getByTestId('caret')).toBeVisible();
}

const count = (page: Page) => page.getByTestId('print-preview-count');

/** Width / height of the preview canvas (CSS pixels). */
async function previewAspect(page: Page): Promise<number> {
  const box = await page.getByTestId('print-preview-canvas').boundingBox();
  return box ? box.width / box.height : 0;
}

test('BlueDoc: print panel previews the pages, the selection and a forced orientation', async ({
  page,
}) => {
  const errors: string[] = [];
  page.on('pageerror', (e) => errors.push(e.message));
  await newDocument(page);
  await page.keyboard.insertText('Trang một');
  await page.keyboard.press('Control+Enter');
  await page.keyboard.insertText('Trang hai');
  await expect(page.getByTestId('status-page')).toContainText('2');

  // Without a selection there is no "selection" choice.
  await page.keyboard.press('Control+p');
  const panel = page.getByTestId('print-panel');
  await expect(panel).toBeVisible();
  await expect(count(page)).toHaveText('Tờ 1/2 · 1 bản');
  await expect(page.getByTestId('print-pages').locator('option[value="selection"]')).toHaveCount(
    0,
  );
  // Portrait pages; landscape paper turns the sheet.
  await expect.poll(() => previewAspect(page)).toBeLessThan(1);
  await page.getByTestId('print-orientation').selectOption('landscape');
  await expect.poll(() => previewAspect(page)).toBeGreaterThan(1);
  await page.keyboard.press('Escape');
  await expect(panel).toHaveCount(0);

  // Select the second page's text: the selection prints on one sheet.
  await page.keyboard.press('Shift+Home');
  await page.keyboard.press('Control+p');
  await expect(panel).toBeVisible();
  await page.getByTestId('print-pages').selectOption('selection');
  await expect(count(page)).toHaveText('Tờ 1/1 · 1 bản');
  await expect(page.getByTestId('print-go')).toBeEnabled();
  await page.keyboard.press('Escape');
  expect(errors).toEqual([]);
});

test('BlueSheet: Ctrl + click on tabs prints the selected sheets together', async ({ page }) => {
  const errors: string[] = [];
  page.on('pageerror', (e) => errors.push(e.message));
  await page.goto('/sheet');
  await expect(page.getByTestId('sheet-canvas')).toHaveAttribute('data-rendered', /\d+/);
  await page.keyboard.type('một');
  await page.keyboard.press('Enter');
  await page.getByRole('button', { name: 'Thêm trang tính' }).click();
  await expect(page.getByTestId('sheet-tab-1')).toHaveAttribute('aria-selected', 'true');
  await page.keyboard.type('hai');
  await page.keyboard.press('Enter');

  // One sheet: one printed page.
  await page.keyboard.press('Control+p');
  const panel = page.getByTestId('print-panel');
  await expect(count(page)).toHaveText('Tờ 1/1 · 1 bản');
  await page.keyboard.press('Escape');
  await expect(panel).toHaveCount(0);

  // Ctrl + click adds the first sheet: both tabs are selected, both sheets print.
  await page.getByTestId('sheet-tab-0').click({ modifiers: ['Control'] });
  await expect(page.getByTestId('sheet-tab-0')).toHaveAttribute('aria-selected', 'true');
  await expect(page.getByTestId('sheet-tab-1')).toHaveAttribute('aria-selected', 'true');
  await page.keyboard.press('Control+p');
  await expect(count(page)).toHaveText('Tờ 1/2 · 1 bản');
  await page.keyboard.press('Escape');

  // A plain click ends the selection.
  await page.getByTestId('sheet-tab-1').click();
  await expect(page.getByTestId('sheet-tab-0')).toHaveAttribute('aria-selected', 'false');
  expect(errors).toEqual([]);
});
