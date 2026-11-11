import { expect, test, type Page } from '@playwright/test';

// Copy / cut / paste on the BlueSheet grid with the reference spreadsheet's behaviour
// (docs/parity/office-behaviour-sheet-nav.md): the moving border, Enter pasting once, a paste
// repeating over a larger selection, cut moving cells and the formulas that point at them.
// The name ends with "paste": it runs in the clipboard project (one spec at a time).

test.use({ permissions: ['clipboard-read', 'clipboard-write'] });

async function openSheet(page: Page) {
  const errors: string[] = [];
  page.on('pageerror', (e) => errors.push(e.message));
  await page.goto('/sheet');
  await expect(page.getByTestId('sheet-canvas')).toHaveAttribute('data-rendered', /\d+/);
  return errors;
}

const nameBox = (page: Page) => page.getByLabel('Hộp tên');
const bar = (page: Page) => page.getByLabel('Thanh công thức');
const editor = (page: Page) => page.getByLabel('Ô đang nhập');

async function goTo(page: Page, ref: string) {
  const box = nameBox(page);
  await box.click();
  await box.fill(ref);
  await box.press('Enter');
  await expect(editor(page)).toBeFocused();
}

async function typeRows(page: Page, ref: string, rows: string[][]) {
  const m = /^([A-Z]+)(\d+)$/.exec(ref)!;
  for (const [i, row] of rows.entries()) {
    await goTo(page, `${m[1]}${Number(m[2]) + i}`);
    for (const v of row) {
      if (v) await page.keyboard.type(v);
      await page.keyboard.press('Tab');
    }
  }
}

async function expectInput(page: Page, ref: string, input: string) {
  await goTo(page, ref);
  await expect(bar(page)).toHaveValue(input);
}

test('copy marks the cells until Esc; Enter pastes once and ends the mark', async ({ page }) => {
  const errors = await openSheet(page);
  await typeRows(page, 'A1', [['1'], ['2']]);
  await goTo(page, 'A1:A2');
  await page.keyboard.press('Control+c');
  const mark = page.getByTestId('copy-mark');
  await expect(mark).toHaveCount(1);
  // Moving keeps the mark; Esc removes it.
  await page.keyboard.press('ArrowRight');
  await expect(mark).toHaveCount(1);
  await page.keyboard.press('Escape');
  await expect(mark).toHaveCount(0);
  // Copy again, then Enter on C1: pasted, the mark ends, the pasted cells are selected.
  await goTo(page, 'A1:A2');
  await page.keyboard.press('Control+c');
  await goTo(page, 'C1');
  await page.keyboard.press('Enter');
  await expect(mark).toHaveCount(0);
  await expect(nameBox(page)).toHaveValue('C1:C2');
  await expectInput(page, 'C2', '2');
  // A plain Enter afterwards just moves.
  await goTo(page, 'E1');
  await page.keyboard.press('Enter');
  await expect(nameBox(page)).toHaveValue('E2');
  await expectInput(page, 'E1', '');
  // Ctrl+V keeps the mark (several pastes); typing an entry ends it.
  await goTo(page, 'A1');
  await page.keyboard.press('Control+c');
  await goTo(page, 'F1');
  await page.keyboard.press('Control+v');
  await expect(mark).toHaveCount(1);
  await goTo(page, 'G1');
  await page.keyboard.type('x');
  await page.keyboard.press('Enter');
  await expect(mark).toHaveCount(0);
  expect(errors).toEqual([]);
});

test('a paste repeats over a selection that is a multiple of the copied size', async ({ page }) => {
  const errors = await openSheet(page);
  await typeRows(page, 'A1', [['a', 'b']]);
  await goTo(page, 'A1:B1');
  await page.keyboard.press('Control+c');
  await goTo(page, 'D1:G3');
  await page.keyboard.press('Control+v');
  await expect(nameBox(page)).toHaveValue('D1:G3');
  for (const [ref, v] of [
    ['D1', 'a'],
    ['E1', 'b'],
    ['F1', 'a'],
    ['G3', 'b'],
    ['D3', 'a'],
  ] as const)
    await expectInput(page, ref, v);
  // One undo step takes the whole paste back.
  await page.keyboard.press('Control+z');
  await expectInput(page, 'G3', '');
  await expectInput(page, 'D1', '');
  expect(errors).toEqual([]);
});

test('cut and paste moves the cells and the formulas that point at them follow', async ({
  page,
}) => {
  const errors = await openSheet(page);
  await typeRows(page, 'A1', [['5'], ['=A1*2']]);
  await goTo(page, 'A1');
  await page.keyboard.press('Control+x');
  await goTo(page, 'C5');
  await page.keyboard.press('Control+v');
  await expect(page.getByTestId('copy-mark')).toHaveCount(0);
  await expectInput(page, 'A1', '');
  await expectInput(page, 'C5', '5');
  await expectInput(page, 'A2', '=C5*2');
  expect(errors).toEqual([]);
});
