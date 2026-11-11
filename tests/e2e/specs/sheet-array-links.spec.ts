import { expect, test, type Page } from '@playwright/test';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

// SHEET-FORMULA-001 through the real UI. Reference behaviour (public documentation of array
// formulas and of the Edit Links dialog in spreadsheet applications):
// * Ctrl+Shift+Enter enters one array formula over the selection; every cell shows it in
//   braces; a one-column result fills down and #N/A fills the rows beyond it; typing into one
//   cell of the array is refused with a message and the entry stays in the editor.
// * A workbook opened without its linked files calculates with the values saved for the
//   links; Data ▸ Edit links lists them; Update values reads a picked source file; Break link
//   turns the formulas using the link into values and renumbers the remaining link.
// Expected values are worked out by hand from tests/corpus/sheet/external-links*.xlsx
// (link 1: B2 = 100, B3 = 250 saved, 120 / 300 in the source; link 2: A1 = 7).

const here = dirname(fileURLToPath(import.meta.url));
const corpus = join(here, '..', '..', 'corpus', 'sheet');

async function goTo(page: Page, ref: string) {
  const box = page.getByLabel('Hộp tên');
  await box.click();
  await box.fill(ref);
  await box.press('Enter');
  await expect(page.getByLabel('Ô đang nhập')).toBeFocused();
}

async function expectValue(page: Page, ref: string, value: string) {
  await goTo(page, ref);
  await expect(page.getByTestId('sheet-a11y-live')).toHaveText(
    new RegExp(`^${ref}: ${value.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')}(,|$)`),
  );
}

async function expectBar(page: Page, ref: string, text: string) {
  await goTo(page, ref);
  await expect(page.getByLabel('Thanh công thức')).toHaveValue(text);
}

async function openFile(page: Page, file: string) {
  const chooser = page.waitForEvent('filechooser');
  await page.locator('[data-command="file.open"]').click();
  await (await chooser).setFiles(join(corpus, file));
  await expect(page.getByTestId('sheet-canvas')).toHaveAttribute('data-rendered', /\d+/);
}

test('BlueSheet: Ctrl+Shift+Enter enters a legacy array formula; part of it cannot change', async ({
  page,
}) => {
  const errors: string[] = [];
  page.on('pageerror', (e) => errors.push(e.message));
  await page.goto('/sheet');
  await expect(page.getByTestId('sheet-canvas')).toHaveAttribute('data-rendered', /\d+/);
  for (const [ref, v] of [
    ['F1', '1'],
    ['F2', '2'],
    ['F3', '3'],
  ]) {
    await goTo(page, ref!);
    await page.keyboard.type(v!);
    await page.keyboard.press('Enter');
  }
  // H1:H4 selected, the formula typed once, Ctrl+Shift+Enter.
  await goTo(page, 'H1:H4');
  await page.keyboard.type('=F1:F3*10');
  await page.keyboard.press('Control+Shift+Enter');
  await expectValue(page, 'H1', '10');
  await expectValue(page, 'H3', '30');
  await expectValue(page, 'H4', '#N/A');
  await expectBar(page, 'H2', '{=F1:F3*10}');
  // Typing into one cell of the array: refused with a message, the entry kept.
  await goTo(page, 'H2');
  await page.keyboard.type('5');
  await page.keyboard.press('Enter');
  await expect(page.getByTestId('message-dialog')).toContainText('Công thức mảng');
  await page.getByTestId('message-dialog').getByRole('button').last().click();
  await expect(page.getByLabel('Thanh công thức')).toHaveValue('5');
  await page.getByLabel('Ô đang nhập').focus();
  await page.keyboard.press('Escape');
  await expectValue(page, 'H2', '20');
  // Editing the array from one of its cells: the braces go, Ctrl+Shift+Enter re-enters it all.
  await goTo(page, 'H3');
  await page.keyboard.press('F2');
  await expect(page.getByLabel('Thanh công thức')).toHaveValue('=F1:F3*10');
  await page.keyboard.press('Backspace');
  await page.keyboard.press('Backspace');
  await page.keyboard.type('100');
  await page.keyboard.press('Control+Shift+Enter');
  await expectValue(page, 'H1', '100');
  await expectValue(page, 'H4', '#N/A');
  // The whole array selected and cleared with Delete.
  await goTo(page, 'H1:H4');
  await page.keyboard.press('Delete');
  await expectBar(page, 'H1', '');
  await expectBar(page, 'H4', '');
  // Typed with plain Enter the same formula spills (a dynamic array).
  await goTo(page, 'J1');
  await page.keyboard.type('=F1:F3*10');
  await page.keyboard.press('Enter');
  await expectValue(page, 'J3', '30');
  expect(errors).toEqual([]);
});

test('BlueSheet: saved link values, Edit links ▸ Update values and Break link', async ({
  page,
}) => {
  const errors: string[] = [];
  page.on('pageerror', (e) => errors.push(e.message));
  await page.goto('/sheet');
  await expect(page.getByTestId('sheet-canvas')).toHaveAttribute('data-rendered', /\d+/);
  await openFile(page, 'external-links.xlsx');
  // The values saved for the links: 100 × 2, 100 + 250, 7.
  await expectValue(page, 'A1', '200');
  await expectValue(page, 'A2', '350');
  await expectValue(page, 'A3', '7');
  // The formula bar names the linked file.
  await expectBar(page, 'A1', "='[Giá.xlsx]Bảng giá'!B2*2");
  await page.getByRole('tab', { name: 'Dữ liệu', exact: true }).click();
  await page.locator('[data-command="data.editLinks"]').click();
  const dlg = page.getByTestId('edit-links');
  await expect(dlg).toBeVisible();
  const list = page.getByTestId('edit-links-list');
  await expect(list.locator('tr[data-source="Giá.xlsx"]')).toBeVisible();
  await expect(list.locator('tr[data-source="Kho.xlsx"]')).toBeVisible();
  await expect(page.getByTestId('edit-links-location')).toContainText('Gi%C3%A1.xlsx');
  // Update values: the source file is picked; 120 × 2 and 120 + 300.
  const chooser = page.waitForEvent('filechooser');
  await page.getByTestId('edit-links-update').click();
  await (await chooser).setFiles(join(corpus, 'external-links-source.xlsx'));
  await expect(page.getByTestId('edit-links-status-0')).toHaveText('Đã cập nhật');
  await page.getByTestId('edit-links-close').click();
  await expectValue(page, 'A1', '240');
  await expectValue(page, 'A2', '420');
  // Break link 1: its formulas become values; link 2 is now [1].
  await page.locator('[data-command="data.editLinks"]').click();
  await list.locator('tr[data-source="Giá.xlsx"]').click();
  await page.getByTestId('edit-links-break').click();
  await page.getByTestId('break-link-ok').click();
  await expect(list.locator('tr[data-source="Giá.xlsx"]')).toHaveCount(0);
  await expect(list.locator('tr[data-source="Kho.xlsx"]')).toBeVisible();
  await page.getByTestId('edit-links-close').click();
  await expectBar(page, 'A1', '240');
  await expectBar(page, 'A3', "='[Kho.xlsx]S'!A1");
  await expectValue(page, 'A3', '7');
  // Undo brings the link and the formula back.
  await page.getByLabel('Ô đang nhập').focus();
  await page.keyboard.press('Control+Z');
  await expectBar(page, 'A1', "='[Giá.xlsx]Bảng giá'!B2*2");
  expect(errors).toEqual([]);
});
