import { expect, test, type Page } from '@playwright/test';

// Everyday moving, selecting, typing and editing in BlueSheet, driven through the real grid
// (keyboard and mouse), with the expected values written from the reference spreadsheet
// behaviour (docs/parity/office-behaviour-sheet-nav.md).

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
const mode = (page: Page) => page.getByTestId('sheet-status');

async function goTo(page: Page, ref: string) {
  const box = nameBox(page);
  await box.click();
  await box.fill(ref);
  await box.press('Enter');
  await expect(editor(page)).toBeFocused();
}

/** Types `rows` from `ref` on (Tab between the cells of a row; "" leaves a cell empty). */
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

/** The active cell: the Name Box of a single cell, else the a11y announcement's address. */
async function expectAt(page: Page, ref: string) {
  await expect(nameBox(page)).toHaveValue(ref);
}

test.describe('Home: moving with Enter / Tab', () => {
  test('Enter, Shift+Enter, Tab, Shift+Tab; Tab…Tab…Enter returns to the start column', async ({
    page,
  }) => {
    const errors = await openSheet(page);
    await goTo(page, 'B2');
    await page.keyboard.press('Enter');
    await expectAt(page, 'B3');
    await page.keyboard.press('Shift+Enter');
    await expectAt(page, 'B2');
    await page.keyboard.press('Tab');
    await expectAt(page, 'C2');
    await page.keyboard.press('Shift+Tab');
    await expectAt(page, 'B2');
    // A row of a list typed with Tab, closed with Enter: the next row starts under B.
    await page.keyboard.type('Tên');
    await page.keyboard.press('Tab');
    await page.keyboard.type('Tuổi');
    await page.keyboard.press('Tab');
    await page.keyboard.type('Lớp');
    await page.keyboard.press('Enter');
    await expectAt(page, 'B3');
    await page.keyboard.type('An');
    await page.keyboard.press('Tab');
    await page.keyboard.type('12');
    await page.keyboard.press('Enter');
    await expectAt(page, 'B4');
    // Moving with an arrow ends the Tab run: Enter goes straight down again.
    await page.keyboard.press('Tab');
    await page.keyboard.press('ArrowRight');
    await page.keyboard.press('Enter');
    await expectAt(page, 'D5');
    await expectInput(page, 'D2', 'Lớp');
    await expectInput(page, 'C3', '12');
    expect(errors).toEqual([]);
  });

  test('Enter and Tab cycle inside a selected range without collapsing it', async ({ page }) => {
    const errors = await openSheet(page);
    await goTo(page, 'B2:C3');
    // B2 → B3 → C2 → C3 → B2 (down the columns), the range stays selected.
    const seen: string[] = [];
    for (let i = 0; i < 4; i++) {
      await page.keyboard.type(String(i + 1));
      await page.keyboard.press('Enter');
      seen.push(await nameBox(page).inputValue());
    }
    expect(seen.every((s) => s === 'B2:C3')).toBe(true);
    await expectInput(page, 'B2', '1');
    await expectInput(page, 'B3', '2');
    await expectInput(page, 'C2', '3');
    await expectInput(page, 'C3', '4');
    expect(errors).toEqual([]);
  });
});

test.describe('Home: typing and editing a cell', () => {
  test('typing replaces the cell, Esc cancels, Delete clears contents only', async ({ page }) => {
    const errors = await openSheet(page);
    await typeRows(page, 'A1', [['cũ']]);
    await goTo(page, 'A1');
    await page.keyboard.type('mới');
    await expect(mode(page)).toHaveAttribute('data-mode', 'enter');
    await page.keyboard.press('Escape');
    await expect(mode(page)).toHaveAttribute('data-mode', 'ready');
    await expect(bar(page)).toHaveValue('cũ');
    await page.keyboard.type('mới');
    await page.keyboard.press('Enter');
    await expectInput(page, 'A1', 'mới');
    // Bold, then Delete: the value goes, the bold stays (typing again is bold).
    await page.keyboard.press('Control+b');
    await page.keyboard.press('Delete');
    await expect(bar(page)).toHaveValue('');
    await page.keyboard.type('x');
    await page.keyboard.press('Enter');
    await goTo(page, 'A1');
    await expect(page.getByTestId('sheet-a11y-live')).toHaveText('A1: x, đậm');
    expect(errors).toEqual([]);
  });

  test('Backspace empties the cell into the editor; Alt+Enter is a new line in the cell', async ({
    page,
  }) => {
    const errors = await openSheet(page);
    await typeRows(page, 'A1', [['abc']]);
    await goTo(page, 'A1');
    await page.keyboard.press('Backspace');
    await expect(mode(page)).toHaveAttribute('data-mode', 'enter');
    await expect(editor(page)).toHaveValue('');
    await page.keyboard.type('dòng 1');
    await page.keyboard.press('Alt+Enter');
    await page.keyboard.type('dòng 2');
    await page.keyboard.press('Enter');
    await expectAt(page, 'A2');
    await expectInput(page, 'A1', 'dòng 1\ndòng 2');
    expect(errors).toEqual([]);
  });

  test('Enter mode: an arrow commits and moves; F2 switches to Edit mode where arrows move the caret', async ({
    page,
  }) => {
    const errors = await openSheet(page);
    await goTo(page, 'B2');
    await page.keyboard.type('12');
    await page.keyboard.press('ArrowRight');
    await expectAt(page, 'C2');
    await expectInput(page, 'B2', '12');
    // F2 on a filled cell: Edit mode, the caret at the end; Left moves inside the text.
    await goTo(page, 'B2');
    await page.keyboard.press('F2');
    await expect(mode(page)).toHaveAttribute('data-mode', 'edit');
    await page.keyboard.press('ArrowLeft');
    await page.keyboard.type('3');
    await expect(editor(page)).toHaveValue('132');
    // F2 again: back to Enter mode, where the arrow commits.
    await page.keyboard.press('F2');
    await expect(mode(page)).toHaveAttribute('data-mode', 'enter');
    await page.keyboard.press('ArrowDown');
    await expectAt(page, 'B3');
    await expectInput(page, 'B2', '132');
    expect(errors).toEqual([]);
  });

  test('Ctrl+Enter fills the selected cells with the entry and keeps the selection', async ({
    page,
  }) => {
    const errors = await openSheet(page);
    await goTo(page, 'B2:C4');
    await page.keyboard.type('7');
    await page.keyboard.press('Control+Enter');
    await expect(nameBox(page)).toHaveValue('B2:C4');
    for (const ref of ['B2', 'B3', 'B4', 'C2', 'C3', 'C4']) await expectInput(page, ref, '7');
    // One undo step removes them all.
    await page.keyboard.press('Control+z');
    for (const ref of ['B2', 'C4']) await expectInput(page, ref, '');
    expect(errors).toEqual([]);
  });
});

test.describe('Home: jumping and selecting', () => {
  test('Ctrl+arrows jump to the edge of the data, Ctrl+Shift+arrows extend', async ({ page }) => {
    const errors = await openSheet(page);
    // A1:A5 filled, a gap, A9:A10 filled; B1:D1 filled.
    await typeRows(page, 'A1', [['1', '2', '3', '4'], ['2'], ['3'], ['4'], ['5']]);
    await typeRows(page, 'A9', [['9'], ['10']]);
    await goTo(page, 'A1');
    await page.keyboard.press('Control+ArrowDown');
    await expectAt(page, 'A5');
    await page.keyboard.press('Control+ArrowDown');
    await expectAt(page, 'A9');
    await page.keyboard.press('Control+ArrowDown');
    await expectAt(page, 'A10');
    await page.keyboard.press('Control+ArrowDown');
    await expectAt(page, 'A1048576');
    await page.keyboard.press('Control+ArrowUp');
    await expectAt(page, 'A10');
    await goTo(page, 'A1');
    await page.keyboard.press('Control+ArrowRight');
    await expectAt(page, 'D1');
    await page.keyboard.press('Control+ArrowRight');
    await expectAt(page, 'XFD1');
    await page.keyboard.press('Control+ArrowLeft');
    await expectAt(page, 'D1');
    // From an empty cell: to the next filled one.
    await goTo(page, 'A7');
    await page.keyboard.press('Control+ArrowUp');
    await expectAt(page, 'A5');
    await goTo(page, 'A1');
    await page.keyboard.press('Control+Shift+ArrowDown');
    await expect(nameBox(page)).toHaveValue('A1:A5');
    // The moving end (A5) jumps: nothing on its right in row 5, so to the last column.
    await page.keyboard.press('Control+Shift+ArrowRight');
    await expect(nameBox(page)).toHaveValue('A1:XFD5');
    await goTo(page, 'A1');
    await page.keyboard.press('Control+Shift+ArrowRight');
    await expect(nameBox(page)).toHaveValue('A1:D1');
    expect(errors).toEqual([]);
  });

  test('Home, Ctrl+Home, Ctrl+End', async ({ page }) => {
    const errors = await openSheet(page);
    await typeRows(page, 'B2', [['1', '2']]);
    await typeRows(page, 'A6', [['x']]);
    await goTo(page, 'E9');
    await page.keyboard.press('Home');
    await expectAt(page, 'A9');
    await page.keyboard.press('Control+End');
    // The last used row (6) and column (C).
    await expectAt(page, 'C6');
    await page.keyboard.press('Control+Home');
    await expectAt(page, 'A1');
    await goTo(page, 'B3');
    await page.keyboard.press('Control+Shift+End');
    await expect(nameBox(page)).toHaveValue('B3:C6');
    expect(errors).toEqual([]);
  });

  test('Ctrl+Space selects the column, Shift+Space the row, Ctrl+A the region then the sheet', async ({
    page,
  }) => {
    const errors = await openSheet(page);
    await typeRows(page, 'B2', [
      ['a', 'b'],
      ['1', '2'],
    ]);
    await goTo(page, 'C3');
    // Whole columns / rows are shown as a click on their header selects them: the used rows /
    // columns, at least 1000 rows and 26 columns (the reference names them C:C and 3:3).
    await page.keyboard.press('Control+Space');
    await expect(nameBox(page)).toHaveValue('C1:C1000');
    await goTo(page, 'C3');
    await page.keyboard.press('Shift+Space');
    await expect(nameBox(page)).toHaveValue('A3:Z3');
    await goTo(page, 'C3');
    await page.keyboard.press('Control+a');
    await expect(nameBox(page)).toHaveValue('B2:C3');
    await page.keyboard.press('Control+a');
    await expect(nameBox(page)).toHaveValue('A1:Z1000');
    // In an empty area Ctrl+A selects the whole sheet at once.
    await goTo(page, 'H20');
    await page.keyboard.press('Control+a');
    await expect(nameBox(page)).toHaveValue('A1:Z1000');
    expect(errors).toEqual([]);
  });

  test('PgDn / PgUp move by a screen, Alt+PgDn / Alt+PgUp by a screen sideways', async ({
    page,
  }) => {
    const errors = await openSheet(page);
    await goTo(page, 'B2');
    await page.keyboard.press('PageDown');
    const down = await nameBox(page).inputValue();
    expect(down).toMatch(/^B\d+$/);
    expect(Number(down.slice(1))).toBeGreaterThan(15);
    await page.keyboard.press('PageUp');
    await expectAt(page, 'B2');
    await page.keyboard.press('Alt+PageDown');
    const right = await nameBox(page).inputValue();
    expect(right).toMatch(/^[C-Z]2$/);
    expect(right.charCodeAt(0)).toBeGreaterThan('E'.charCodeAt(0));
    await page.keyboard.press('Alt+PageUp');
    await expectAt(page, 'B2');
    expect(errors).toEqual([]);
  });
});

test.describe('Formulas: typing a formula', () => {
  test('arrow keys pick a reference (point mode), coloured like its border on the grid', async ({
    page,
  }) => {
    const errors = await openSheet(page);
    await typeRows(page, 'A1', [['2'], ['3']]);
    await goTo(page, 'C1');
    await page.keyboard.type('=');
    // Left: the cell on the left goes into the formula, Up/Shift move and extend it.
    await page.keyboard.press('ArrowLeft');
    await page.keyboard.press('ArrowLeft');
    await expect(editor(page)).toHaveValue('=A1');
    await page.keyboard.press('Shift+ArrowDown');
    await expect(editor(page)).toHaveValue('=A1:A2');
    await expect(page.getByTestId('ref-box')).toHaveCount(1);
    // An operator, then an arrow picks a new reference.
    await page.keyboard.type('+');
    await page.keyboard.press('ArrowLeft');
    await expect(editor(page)).toHaveValue('=A1:A2+B1');
    const boxes = page.getByTestId('ref-box');
    await expect(boxes).toHaveCount(2);
    const colours = await boxes.evaluateAll((els) => els.map((e) => e.dataset.color));
    expect(new Set(colours).size).toBe(2);
    // The reference text has the colour of its border.
    const words = page.getByTestId('cell-editor-mirror').locator('[data-ref-color]');
    await expect(words).toHaveText(['A1:A2', 'B1']);
    expect(
      await words.evaluateAll((els) => els.map((e) => (e as HTMLElement).dataset.refColor)),
    ).toEqual(colours);
    // Typing ends the pick: an arrow then commits (Enter mode).
    await page.keyboard.press('Enter');
    await expectInput(page, 'C1', '=A1:A2+B1');
    await expect(page.getByTestId('ref-box')).toHaveCount(0);
    expect(errors).toEqual([]);
  });

  test('a click on a cell while typing a formula puts its reference in; a drag a range', async ({
    page,
  }) => {
    const errors = await openSheet(page);
    await typeRows(page, 'A1', [['4'], ['6']]);
    const centre = async (ref: string) => {
      await goTo(page, ref);
      const b = (await editor(page).boundingBox())!;
      return { x: b.x + b.width / 2, y: b.y + b.height / 2 };
    };
    const a1 = await centre('A1');
    const a2 = await centre('A2');
    await goTo(page, 'C3');
    await page.keyboard.type('=SUM(');
    await page.mouse.move(a1.x, a1.y);
    await page.mouse.down();
    await page.mouse.move(a2.x, a2.y, { steps: 4 });
    await page.mouse.up();
    await expect(editor(page)).toHaveValue('=SUM(A1:A2');
    await page.keyboard.type(')');
    await page.keyboard.press('Enter');
    await goTo(page, 'C3');
    await expect(page.getByTestId('sheet-a11y-live')).toHaveText('C3: 10, công thức =SUM(A1:A2)');
    expect(errors).toEqual([]);
  });

  test('F4 cycles $; function names complete with Tab; the argument hint follows the caret', async ({
    page,
  }) => {
    const errors = await openSheet(page);
    await goTo(page, 'B2');
    await page.keyboard.type('=A1');
    await page.keyboard.press('F4');
    await expect(editor(page)).toHaveValue('=$A$1');
    await page.keyboard.press('F4');
    await expect(editor(page)).toHaveValue('=A$1');
    await page.keyboard.press('Escape');
    await goTo(page, 'B2');
    await page.keyboard.type('=VLO');
    const list = page.getByTestId('function-list');
    await expect(list.getByRole('option')).toHaveText(['VLOOKUP']);
    await page.keyboard.press('Tab');
    await expect(editor(page)).toHaveValue('=VLOOKUP(');
    await expect(list).toHaveCount(0);
    // Still in the cell (Tab took the name); the hint shows the arguments, the first current.
    await expect(mode(page)).toHaveAttribute('data-mode', 'enter');
    const hint = page.getByTestId('argument-hint');
    await expect(hint).toContainText('VLOOKUP(lookup_value, table, column_index, [approximate])');
    await expect(page.getByTestId('argument-current')).toHaveText('lookup_value');
    await page.keyboard.type('1,');
    await expect(page.getByTestId('argument-current')).toHaveText('table');
    await page.keyboard.press('Escape');
    await expect(hint).toHaveCount(0);
    // Several matches: arrows choose, Tab takes the chosen one.
    await goTo(page, 'B3');
    await page.keyboard.type('=COUNTI');
    await expect(list.getByRole('option')).toHaveText(['COUNTIF', 'COUNTIFS']);
    await page.keyboard.press('ArrowDown');
    await page.keyboard.press('Tab');
    await expect(editor(page)).toHaveValue('=COUNTIFS(');
    await page.keyboard.press('Escape');
    expect(errors).toEqual([]);
  });

  test('a click on another sheet tab while typing puts that sheet in the reference', async ({
    page,
  }) => {
    const errors = await openSheet(page);
    await page.getByRole('button', { name: 'Thêm trang tính' }).click();
    const tabs = page.getByTestId(/^sheet-tab-\d+$/);
    await expect(tabs).toHaveCount(2);
    // Sheet 2: B2 = 21.
    await typeRows(page, 'B2', [['21']]);
    await tabs.nth(0).click();
    await goTo(page, 'A1');
    await page.keyboard.type('=');
    await tabs.nth(1).click();
    const second = (await tabs.nth(1).textContent())!.trim();
    await expect(bar(page)).toHaveValue(`=${second}!`);
    await page.keyboard.press('ArrowDown');
    await page.keyboard.press('ArrowRight');
    await expect(bar(page)).toHaveValue(`=${second}!B2`);
    await page.keyboard.type('*2');
    await page.keyboard.press('Enter');
    // Back on the first sheet, under the edited cell.
    await expect(tabs.nth(0)).toHaveAttribute('aria-selected', 'true');
    await expectAt(page, 'A2');
    await goTo(page, 'A1');
    await expect(page.getByTestId('sheet-a11y-live')).toHaveText(
      `A1: 42, công thức =${second}!B2*2`,
    );
    expect(errors).toEqual([]);
  });

  test('the formula bar edits like the cell and expands to several lines', async ({ page }) => {
    const errors = await openSheet(page);
    await goTo(page, 'A1');
    await bar(page).click();
    await page.keyboard.type('=1+2');
    await expect(editor(page)).toHaveValue('=1+2');
    await page.keyboard.press('Enter');
    await expectAt(page, 'A2');
    await goTo(page, 'A1');
    await expect(page.getByTestId('sheet-a11y-live')).toHaveText('A1: 3, công thức =1+2');
    // Alt+Enter in the bar is a line break too; the expand button shows the lines.
    await goTo(page, 'B1');
    await bar(page).click();
    await page.keyboard.type('một');
    await page.keyboard.press('Alt+Enter');
    await page.keyboard.type('hai');
    await page.keyboard.press('Enter');
    await expectInput(page, 'B1', 'một\nhai');
    const before = (await bar(page).boundingBox())!.height;
    await page.getByTestId('formula-bar-expand').click();
    await expect
      .poll(async () => (await bar(page).boundingBox())!.height)
      .toBeGreaterThan(before * 2);
    await page.keyboard.press('Control+Shift+U');
    await expect
      .poll(async () => (await bar(page).boundingBox())!.height)
      .toBeLessThan(before * 1.5);
    expect(errors).toEqual([]);
  });
});

test.describe('Home: AutoComplete and pick from the list', () => {
  test('typing the start of an entry of the column completes it; Alt+Down lists the entries', async ({
    page,
  }) => {
    const errors = await openSheet(page);
    await typeRows(page, 'A1', [['Hà Nội'], ['Huế'], ['Đà Nẵng']]);
    await goTo(page, 'A4');
    await page.keyboard.type('Hu');
    // The rest of the only matching entry is proposed, selected.
    await expect(editor(page)).toHaveValue('Huế');
    expect(
      await editor(page).evaluate((el: HTMLTextAreaElement) =>
        el.value.slice(el.selectionStart, el.selectionEnd),
      ),
    ).toBe('ế');
    await page.keyboard.press('Enter');
    await expectInput(page, 'A4', 'Huế');
    // Two entries start with "H": nothing is proposed until the text is unique.
    await goTo(page, 'A5');
    await page.keyboard.type('H');
    await expect(editor(page)).toHaveValue('H');
    // Backspace does not bring the proposal back.
    await page.keyboard.type('à');
    await expect(editor(page)).toHaveValue('Hà Nội');
    await page.keyboard.press('Delete');
    await expect(editor(page)).toHaveValue('Hà');
    await page.keyboard.press('Escape');
    // Alt+Down: the entries, sorted; arrows and Enter pick one.
    await goTo(page, 'A5');
    await page.keyboard.press('Alt+ArrowDown');
    const list = page.getByTestId('dv-list');
    await expect(list.getByRole('option')).toHaveText(['Đà Nẵng', 'Hà Nội', 'Huế']);
    await page.keyboard.press('ArrowDown');
    await page.keyboard.press('Enter');
    await expect(list).toHaveCount(0);
    await expectInput(page, 'A5', 'Hà Nội');
    expect(errors).toEqual([]);
  });
});

test.describe('Home: undo granularity', () => {
  test('one undo step per committed entry, per Ctrl+Enter and per fill', async ({ page }) => {
    const errors = await openSheet(page);
    await typeRows(page, 'A1', [['1', '2']]);
    await page.keyboard.press('Control+z');
    await expectInput(page, 'B1', '');
    await expectInput(page, 'A1', '1');
    await page.keyboard.press('Control+y');
    await expectInput(page, 'B1', '2');
    await goTo(page, 'A1:B3');
    await page.keyboard.press('Control+d');
    await expectInput(page, 'B3', '2');
    await page.keyboard.press('Control+z');
    await expectInput(page, 'B3', '');
    await expectInput(page, 'A2', '');
    await expectInput(page, 'B1', '2');
    expect(errors).toEqual([]);
  });
});

test.describe('Home: selection on the grid', () => {
  test('the headers of the selection are highlighted; no fill handle while typing', async ({
    page,
  }) => {
    const errors = await openSheet(page);
    await goTo(page, 'B2:C4');
    const cols = page.getByTestId('header-mark-cols');
    const rows = page.getByTestId('header-mark-rows');
    const handle = page.getByTestId('fill-handle');
    await expect(handle).toBeVisible();
    const sel = (await handle.locator('..').boundingBox())!;
    const c = (await cols.boundingBox())!;
    const r = (await rows.boundingBox())!;
    // The column headers B:C span the selection's width, the row headers 2:4 its height.
    expect(Math.abs(c.x - sel.x)).toBeLessThan(2);
    expect(Math.abs(c.width - sel.width)).toBeLessThan(2);
    expect(c.y + c.height).toBeLessThanOrEqual(sel.y + 1);
    expect(Math.abs(r.y - sel.y)).toBeLessThan(2);
    expect(Math.abs(r.height - sel.height)).toBeLessThan(2);
    expect(r.x + r.width).toBeLessThanOrEqual(sel.x + 1);
    await page.keyboard.type('1');
    await expect(handle).toHaveCount(0);
    await page.keyboard.press('Enter');
    await expect(handle).toBeVisible();
    expect(errors).toEqual([]);
  });
});

test.describe('Home: Go To', () => {
  test('F5 and Ctrl+G open Go To; Enter selects the reference and nothing leaks into the cell', async ({
    page,
  }) => {
    const errors = await openSheet(page);
    await goTo(page, 'A1');
    for (const key of ['F5', 'Control+g']) {
      await page.keyboard.press(key);
      const dialog = page.getByTestId('goto-dialog');
      await expect(dialog).toBeVisible();
      await page.getByTestId('goto-ref').fill(key === 'F5' ? 'C7' : 'B2:D3');
      await page.getByTestId('goto-ref').press('Enter');
      await expect(dialog).toHaveCount(0);
      await expect(nameBox(page)).toHaveValue(key === 'F5' ? 'C7' : 'B2:D3');
      await expect(mode(page)).toHaveAttribute('data-mode', 'ready');
      await expect(editor(page)).toHaveValue('');
    }
    expect(errors).toEqual([]);
  });
});
