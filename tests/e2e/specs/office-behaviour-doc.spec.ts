import { expect, test, type Page } from '@playwright/test';

// Everyday behaviour of BlueDoc, action by action, the way word processors of the OOXML family
// behave (docs/parity/office-behaviour-doc.md). Every expected value below is the reference
// behaviour, not what the app happened to do: real keys and real buttons on the running web app.
// (Typed text starts with a capital: AutoCorrect capitalises the first letter of a sentence,
// as the reference does by default.)

test.use({ permissions: ['clipboard-read', 'clipboard-write'] });

const cmd = (page: Page, id: string) => page.locator(`[data-command="${id}"]`).first();
const mirror = (page: Page) => page.getByTestId('doc-a11y-mirror');

async function newDocument(page: Page) {
  await page.goto('/doc');
  await page.getByTestId('new-document').click();
  await expect(page.getByTestId('caret')).toBeVisible();
}

/** Text of every paragraph, in document order (the screen-reader mirror of the pages). */
async function paras(page: Page): Promise<string[]> {
  return mirror(page)
    .locator('[data-p]')
    .evaluateAll((els) => els.map((e) => (e.textContent ?? '').replace(/\u00a0/g, ' ')));
}

async function expectParas(page: Page, expected: string[]) {
  await expect.poll(() => paras(page)).toEqual(expected);
}

/** Paragraph kinds: `h1`…, `li1`… (list level), `p`. */
async function kinds(page: Page): Promise<string[]> {
  return mirror(page)
    .locator('[data-p]')
    .evaluateAll((els) =>
      els.map((e) =>
        e.tagName === 'LI' ? `li${e.getAttribute('aria-level')}` : e.tagName.toLowerCase(),
      ),
    );
}

/** The selected text, as Ctrl+C copies it ('' when nothing is selected). */
async function selected(page: Page): Promise<string> {
  await page.evaluate(() => navigator.clipboard.writeText('␀'));
  await page.keyboard.press('Control+c');
  for (let i = 0; i < 40; i++) {
    const v = await page.evaluate(() => navigator.clipboard.readText());
    if (v !== '␀') return v.replace(/\r\n/g, '\n');
    await page.waitForTimeout(50);
  }
  return '';
}

/** Where the caret is: types a marker, reads the paragraphs, takes the marker out again. */
async function caretMark(page: Page): Promise<string[]> {
  await page.keyboard.insertText('|');
  await expect.poll(async () => (await paras(page)).join('\n')).toContain('|');
  const out = await paras(page);
  await page.keyboard.press('Backspace');
  await expect.poll(async () => (await paras(page)).join('\n')).not.toContain('|');
  return out;
}

async function typeParas(page: Page, lines: string[]) {
  for (const [i, l] of lines.entries()) {
    if (i) await page.keyboard.press('Enter');
    await page.keyboard.insertText(l);
  }
  await expectParas(page, lines);
}

async function caretBox(page: Page) {
  for (let i = 0; i < 40; i++) {
    const b = await page.getByTestId('caret').boundingBox();
    if (b) return b;
    await page.waitForTimeout(50);
  }
  throw new Error('no caret');
}

/** Screen point of the caret after moving it `right` characters from the document start. */
async function pointAt(page: Page, right: number) {
  await page.keyboard.press('Control+Home');
  for (let i = 0; i < right; i++) await page.keyboard.press('ArrowRight');
  const c = await caretBox(page);
  return { x: c.x + 1, y: c.y + c.height / 2 };
}

async function tab(page: Page, name: string) {
  await page.getByRole('tab', { name, exact: true }).click();
}

const PX_PER_PT = 96 / 72;

test.describe('typing and selection', () => {
  test('typing replaces the selection; Ctrl+Backspace / Ctrl+Delete delete a word', async ({
    page,
  }) => {
    await newDocument(page);
    await typeParas(page, ['Một hai ba bốn']);
    await page.keyboard.press('Shift+Home');
    await page.keyboard.insertText('X');
    await expectParas(page, ['X']);
    await page.keyboard.press('Backspace');
    await page.keyboard.insertText('Một hai ba bốn');
    // Ctrl+Backspace deletes the word before the caret.
    await page.keyboard.press('Control+Backspace');
    await expectParas(page, ['Một hai ba ']);
    await page.keyboard.press('Home');
    // Ctrl+Delete deletes the word after the caret with the space after it.
    await page.keyboard.press('Control+Delete');
    await expectParas(page, ['hai ba ']);
    // One undo step.
    await page.keyboard.press('Control+z');
    await expectParas(page, ['Một hai ba ']);
  });

  test('Ctrl+arrows go by word and by paragraph; Home / End; Ctrl+Home / Ctrl+End; Shift extends', async ({
    page,
  }) => {
    await newDocument(page);
    await typeParas(page, ['Một hai ba', 'Bốn năm', 'Sáu bảy']);
    await page.keyboard.press('Control+Home');
    await page.keyboard.press('Control+ArrowRight');
    expect(await caretMark(page)).toEqual(['Một |hai ba', 'Bốn năm', 'Sáu bảy']);
    await page.keyboard.press('Control+ArrowDown');
    expect(await caretMark(page)).toEqual(['Một hai ba', '|Bốn năm', 'Sáu bảy']);
    await page.keyboard.press('ArrowRight');
    // Inside a paragraph Ctrl+Up goes to its start, then to the previous paragraph's start.
    await page.keyboard.press('Control+ArrowUp');
    expect(await caretMark(page)).toEqual(['Một hai ba', '|Bốn năm', 'Sáu bảy']);
    await page.keyboard.press('Control+ArrowUp');
    expect(await caretMark(page)).toEqual(['|Một hai ba', 'Bốn năm', 'Sáu bảy']);
    await page.keyboard.press('End');
    expect(await caretMark(page)).toEqual(['Một hai ba|', 'Bốn năm', 'Sáu bảy']);
    await page.keyboard.press('Control+End');
    expect(await caretMark(page)).toEqual(['Một hai ba', 'Bốn năm', 'Sáu bảy|']);
    await page.keyboard.press('Control+Shift+ArrowLeft');
    expect(await selected(page)).toBe('bảy');
    await page.keyboard.press('Control+End');
    await page.keyboard.press('Control+Shift+ArrowUp');
    expect(await selected(page)).toBe('Sáu bảy');
    await page.keyboard.press('Control+End');
    await page.keyboard.press('Control+Shift+Home');
    expect(await selected(page)).toBe('Một hai ba\nBốn năm\nSáu bảy');
  });

  test('double click selects a word with its space, triple click the paragraph, Ctrl+click a sentence', async ({
    page,
  }) => {
    await newDocument(page);
    await typeParas(page, ['Câu một ở đây. Câu hai ở đây.', 'Đoạn sau']);
    const at = await pointAt(page, 5);
    await page.mouse.dblclick(at.x, at.y);
    expect(await selected(page)).toBe('một ');
    await page.mouse.click(at.x, at.y, { clickCount: 3 });
    expect((await selected(page)).trimEnd()).toBe('Câu một ở đây. Câu hai ở đây.');
    await page.keyboard.press('Control+Home');
    await page.keyboard.down('Control');
    await page.mouse.click(at.x, at.y);
    await page.keyboard.up('Control');
    expect(await selected(page)).toBe('Câu một ở đây. ');
  });

  test('a drag past the first word selects whole words', async ({ page }) => {
    await newDocument(page);
    await typeParas(page, ['Một hai ba bốn năm']);
    const from = await pointAt(page, 1);
    const to = await pointAt(page, 9);
    await page.mouse.move(from.x, from.y);
    await page.mouse.down();
    await page.mouse.move((from.x + to.x) / 2, to.y, { steps: 4 });
    await page.mouse.move(to.x, to.y, { steps: 4 });
    await page.mouse.up();
    // From the start of "Một" to the end of "ba" with its space.
    expect(await selected(page)).toBe('Một hai ba ');
  });

  test('Ctrl+Z undoes typing a run at a time; Ctrl+Y redoes', async ({ page }) => {
    await newDocument(page);
    await page.keyboard.type('một hai ba');
    await expectParas(page, ['Một hai ba']);
    await page.keyboard.press('Control+z');
    // Neither the whole paragraph at once nor a letter at a time.
    await expect
      .poll(async () => ((await paras(page))[0] ?? '').length)
      .toBeLessThan('Một hai ba'.length - 1);
    expect(((await paras(page))[0] ?? '').length).toBeGreaterThan(0);
    await page.keyboard.press('Control+y');
    await expectParas(page, ['Một hai ba']);
  });

  test('Insert key: overtype is off by default and toggles', async ({ page }) => {
    await newDocument(page);
    await typeParas(page, ['Abc']);
    await page.keyboard.press('Home');
    await page.keyboard.insertText('X');
    await expectParas(page, ['XAbc']);
    await page.keyboard.press('Insert');
    await page.keyboard.insertText('Y');
    await expectParas(page, ['XYbc']);
    await page.keyboard.press('Insert');
    await page.keyboard.insertText('Z');
    await expectParas(page, ['XYZbc']);
  });
});

test.describe('Home tab', () => {
  test('groups in the reference order: clipboard, font, paragraph, styles, editing', async ({
    page,
  }) => {
    await newDocument(page);
    await page.keyboard.insertText('Đoạn');
    const order = await page
      .locator('[role="tabpanel"][data-state="active"] > [data-group]')
      .evaluateAll((els) => els.map((e) => (e as HTMLElement).dataset.group));
    const main = order.filter((g) =>
      ['clipboard', 'font', 'paragraph', 'styles', 'editing'].includes(g ?? ''),
    );
    expect(main).toEqual(['clipboard', 'font', 'paragraph', 'styles', 'editing']);
    // The styles gallery: quick styles drawn as themselves, the caret's style pressed.
    const gallery = page.getByTestId('styles-gallery');
    await expect(gallery.locator('[data-style-id="Normal"]')).toHaveAttribute(
      'aria-pressed',
      'true',
    );
    await gallery.locator('[data-style-id="Heading1"]').click();
    await expect.poll(() => kinds(page)).toEqual(['h1']);
    await expect(gallery.locator('[data-style-id="Heading1"]')).toHaveAttribute(
      'aria-pressed',
      'true',
    );
    await page.getByTestId('styles-gallery-more').click();
    await expect(page.locator('[data-testid="style-item"]').first()).toBeVisible();
  });

  test('Ctrl+B / I / U toggle on a selection and at a collapsed caret', async ({ page }) => {
    await newDocument(page);
    await typeParas(page, ['Một hai']);
    await page.keyboard.press('Shift+Home');
    await page.keyboard.press('Control+b');
    await expect(cmd(page, 'format.bold')).toHaveAttribute('aria-pressed', 'true');
    await page.keyboard.press('Control+b');
    await expect(cmd(page, 'format.bold')).toHaveAttribute('aria-pressed', 'false');
    await page.keyboard.press('End');
    // At a collapsed caret the toggle applies to what is typed next.
    await page.keyboard.press('Control+i');
    await expect(cmd(page, 'format.italic')).toHaveAttribute('aria-pressed', 'true');
    await page.keyboard.insertText(' ba');
    await expect(cmd(page, 'format.italic')).toHaveAttribute('aria-pressed', 'true');
    await page.keyboard.press('Control+i');
    await page.keyboard.insertText(' bốn');
    await expect(cmd(page, 'format.italic')).toHaveAttribute('aria-pressed', 'false');
    await page.keyboard.press('Control+u');
    await expect(cmd(page, 'format.underline')).toHaveAttribute('aria-pressed', 'true');
  });

  test('Ctrl+Shift+= superscript, Ctrl+= subscript; Ctrl+Space removes character formatting', async ({
    page,
  }) => {
    await newDocument(page);
    await typeParas(page, ['Chữ']);
    await page.keyboard.press('Shift+Home');
    await page.keyboard.press('Control+Shift+Equal');
    await expect(cmd(page, 'format.superscript')).toHaveAttribute('aria-pressed', 'true');
    await page.keyboard.press('Control+Equal');
    await expect(cmd(page, 'format.subscript')).toHaveAttribute('aria-pressed', 'true');
    await page.keyboard.press('Control+b');
    await expect(cmd(page, 'format.bold')).toHaveAttribute('aria-pressed', 'true');
    await page.keyboard.press('Control+Space');
    await expect(cmd(page, 'format.bold')).toHaveAttribute('aria-pressed', 'false');
    await expect(cmd(page, 'format.subscript')).toHaveAttribute('aria-pressed', 'false');
  });

  test('font size: Ctrl+] / Ctrl+[ by one point, Ctrl+Shift+> / < along the size list', async ({
    page,
  }) => {
    await newDocument(page);
    await typeParas(page, ['Chữ']);
    await page.keyboard.press('Shift+Home');
    const size = page.getByTestId('font-size');
    const start = Number(await size.inputValue());
    await page.keyboard.press('Control+BracketRight');
    await expect(size).toHaveValue(String(start + 1));
    await page.keyboard.press('Control+BracketLeft');
    await expect(size).toHaveValue(String(start));
    // 14 → 16 → 18 → 16 (… 12 14 16 18 20 …), 72 → 80.
    await size.fill('14');
    await size.press('Enter');
    await page.keyboard.press('Control+Shift+Period');
    await expect(size).toHaveValue('16');
    await page.keyboard.press('Control+Shift+Period');
    await expect(size).toHaveValue('18');
    await page.keyboard.press('Control+Shift+Comma');
    await expect(size).toHaveValue('16');
    await size.fill('72');
    await size.press('Enter');
    await page.keyboard.press('Control+Shift+Period');
    await expect(size).toHaveValue('80');
  });

  test('Ctrl+D opens the font dialog; Ctrl+Shift+Space types a nonbreaking space', async ({
    page,
  }) => {
    await newDocument(page);
    await typeParas(page, ['Một']);
    await page.keyboard.press('Control+Shift+Space');
    await page.keyboard.insertText('hai');
    await expect
      .poll(() =>
        mirror(page)
          .locator('[data-p]')
          .first()
          .evaluate((e) => e.textContent),
      )
      .toBe('Một\u00a0hai');
    await page.keyboard.press('Control+d');
    await expect(page.getByTestId('font-effects-dialog')).toBeVisible();
  });

  test('Shift+F3 cycles the case of the selection', async ({ page }) => {
    await newDocument(page);
    await typeParas(page, ['Xin chào bạn']);
    await page.keyboard.press('Shift+Home');
    await page.keyboard.press('Shift+F3');
    await expectParas(page, ['XIN CHÀO BẠN']);
    await page.keyboard.press('Shift+F3');
    await expectParas(page, ['xin chào bạn']);
    await page.keyboard.press('Shift+F3');
    await expectParas(page, ['Xin Chào Bạn']);
    expect(await selected(page)).toBe('Xin Chào Bạn');
  });

  test('Ctrl+E / R / J / L align, Ctrl+Q removes the paragraph formatting', async ({ page }) => {
    await newDocument(page);
    await typeParas(page, ['Đoạn']);
    for (const [key, id] of [
      ['e', 'format.alignCenter'],
      ['r', 'format.alignRight'],
      ['j', 'format.justify'],
      ['l', 'format.alignLeft'],
      ['e', 'format.alignCenter'],
    ] as const) {
      await page.keyboard.press(`Control+${key}`);
      await expect(cmd(page, id)).toHaveAttribute('aria-pressed', 'true');
    }
    await page.keyboard.press('Control+q');
    await expect(cmd(page, 'format.alignLeft')).toHaveAttribute('aria-pressed', 'true');
    await expect(page.getByTestId('command-search')).toHaveCount(0);
  });

  test('line spacing Ctrl+2 / Ctrl+5 / Ctrl+1, 12 pt before with Ctrl+0, indents Ctrl+M / Ctrl+T', async ({
    page,
  }) => {
    await newDocument(page);
    await typeParas(page, ['Một', 'Hai']);
    const y = async () => (await caretBox(page)).y;
    const x = async () => (await caretBox(page)).x;
    await page.keyboard.press('Control+Home');
    const y1 = await y();
    await page.keyboard.press('ArrowDown');
    const single = (await y()) - y1;
    await page.keyboard.press('Control+a');
    await page.keyboard.press('Control+2');
    await page.keyboard.press('Control+Home');
    const top = await y();
    await page.keyboard.press('ArrowDown');
    // Double spacing: the second line is about twice as far down.
    await expect.poll(async () => ((await y()) - top) / single).toBeGreaterThan(1.6);
    await page.keyboard.press('Control+a');
    await page.keyboard.press('Control+1');
    await page.keyboard.press('Control+Home');
    await page.keyboard.press('ArrowDown');
    await expect.poll(async () => Math.round((await y()) - top)).toBe(Math.round(single));
    // Ctrl+0: 12 pt before the paragraph, Ctrl+0 again: none.
    await page.keyboard.press('Control+0');
    await expect.poll(async () => (await y()) - top - single).toBeGreaterThan(12 * PX_PER_PT - 2);
    await page.keyboard.press('Control+0');
    await expect.poll(async () => Math.round((await y()) - top)).toBe(Math.round(single));
    // Ctrl+M moves the paragraph in by half an inch, Ctrl+Shift+M back.
    await page.keyboard.press('Home');
    const x0 = await x();
    await page.keyboard.press('Control+m');
    await expect.poll(async () => Math.round((await x()) - x0)).toBe(Math.round(36 * PX_PER_PT));
    await page.keyboard.press('Control+Shift+m');
    await expect.poll(async () => Math.round(await x())).toBe(Math.round(x0));
    // Ctrl+T: hanging indent — the first line stays, the left indent moves in.
    await page.keyboard.press('Control+t');
    await expect.poll(async () => Math.round(await x())).toBe(Math.round(x0));
    await page.keyboard.press('Control+Shift+t');
    await expect.poll(async () => Math.round(await x())).toBe(Math.round(x0));
  });

  test('Tab at the start of a paragraph indents its first line; Backspace takes it away', async ({
    page,
  }) => {
    await newDocument(page);
    await typeParas(page, ['Đoạn văn']);
    await page.keyboard.press('Home');
    const x0 = (await caretBox(page)).x;
    await page.keyboard.press('Tab');
    // No tab character: the first line is indented by half an inch.
    await expectParas(page, ['Đoạn văn']);
    await expect
      .poll(async () => Math.round((await caretBox(page)).x - x0))
      .toBe(Math.round(36 * PX_PER_PT));
    await page.keyboard.press('Backspace');
    await expect.poll(async () => Math.round((await caretBox(page)).x)).toBe(Math.round(x0));
    await expectParas(page, ['Đoạn văn']);
    // Inside the text Tab is a tab character.
    await page.keyboard.press('End');
    await page.keyboard.press('Tab');
    await page.keyboard.insertText('x');
    await expect.poll(async () => (await paras(page))[0]).toMatch(/^Đoạn văn\s+x$/);
  });

  test('lists: "1. " numbers, Enter continues, Tab / Shift+Tab change level, empty Enter ends, Backspace', async ({
    page,
  }) => {
    await newDocument(page);
    await page.keyboard.type('1. một');
    await expect.poll(() => kinds(page)).toEqual(['li1']);
    await expectParas(page, ['một']);
    await page.keyboard.press('Enter');
    await page.keyboard.type('hai');
    await expect.poll(() => kinds(page)).toEqual(['li1', 'li1']);
    await page.keyboard.press('Enter');
    await page.keyboard.press('Tab');
    await expect.poll(() => kinds(page)).toEqual(['li1', 'li1', 'li2']);
    await page.keyboard.press('Shift+Tab');
    await expect.poll(() => kinds(page)).toEqual(['li1', 'li1', 'li1']);
    // Enter on an empty item ends the list.
    await page.keyboard.press('Enter');
    await expect.poll(() => kinds(page)).toEqual(['li1', 'li1', 'p']);
    await page.keyboard.type('sau');
    await expectParas(page, ['Một', 'Hai', 'sau']);
    // Backspace at the start of an item removes its number, the text stays.
    await page.keyboard.press('ArrowUp');
    await page.keyboard.press('Home');
    await page.keyboard.press('Backspace');
    await expect.poll(() => kinds(page)).toEqual(['li1', 'p', 'p']);
    await expectParas(page, ['Một', 'Hai', 'sau']);
  });

  test('bullets from "* " and "- ", and Ctrl+Shift+L', async ({ page }) => {
    await newDocument(page);
    await page.keyboard.type('* một');
    await expect.poll(() => kinds(page)).toEqual(['li1']);
    // (The first letter is capitalised once the word is finished.)
    await expectParas(page, ['một']);
    await page.keyboard.press('Enter');
    await page.keyboard.press('Enter');
    await page.keyboard.type('- hai');
    await expect.poll(() => kinds(page)).toEqual(['li1', 'li1']);
    await page.keyboard.press('Enter');
    await page.keyboard.press('Enter');
    await page.keyboard.insertText('Ba');
    await expect.poll(() => kinds(page)).toEqual(['li1', 'li1', 'p']);
    await page.keyboard.press('Control+Shift+l');
    await expect.poll(() => kinds(page)).toEqual(['li1', 'li1', 'li1']);
  });

  test('styles go to the paragraph when nothing is selected: Ctrl+Alt+1/2/3, Ctrl+Shift+N', async ({
    page,
  }) => {
    await newDocument(page);
    await typeParas(page, ['Tiêu đề']);
    await page.keyboard.press('Home');
    await page.keyboard.press('Control+Alt+1');
    await expect.poll(() => kinds(page)).toEqual(['h1']);
    await page.keyboard.press('Control+Alt+2');
    await expect.poll(() => kinds(page)).toEqual(['h2']);
    await page.keyboard.press('Control+Alt+3');
    await expect.poll(() => kinds(page)).toEqual(['h3']);
    await page.keyboard.press('Control+Shift+n');
    await expect.poll(() => kinds(page)).toEqual(['p']);
    // The gallery tile of Heading 1 (Normal, Title, Heading 1 are the first quick styles).
    await page.locator('[data-style-command="style.heading1"]').click();
    await expect.poll(() => kinds(page)).toEqual(['h1']);
  });

  test('Ctrl+H opens replace; F4 repeats the last formatting', async ({ page }) => {
    await newDocument(page);
    await typeParas(page, ['Một', 'Hai']);
    await page.keyboard.press('Control+Home');
    await page.keyboard.press('Control+e');
    await page.keyboard.press('ArrowDown');
    await page.keyboard.press('F4');
    await expect(cmd(page, 'format.alignCenter')).toHaveAttribute('aria-pressed', 'true');
    await page.keyboard.press('Control+h');
    await expect(page.getByTestId('find-replace')).toBeVisible();
  });

  test('paste keeps the source formatting and offers the paste options button', async ({
    page,
  }) => {
    await newDocument(page);
    await typeParas(page, ['Đậm']);
    await page.keyboard.press('Shift+Home');
    await page.keyboard.press('Control+b');
    expect(await selected(page)).toBe('Đậm');
    await page.keyboard.press('End');
    await page.keyboard.press('Enter');
    // Enter after bold text: what is typed next is bold too; Ctrl+B turns it off.
    await expect(cmd(page, 'format.bold')).toHaveAttribute('aria-pressed', 'true');
    await page.keyboard.press('Control+b');
    await expect(cmd(page, 'format.bold')).toHaveAttribute('aria-pressed', 'false');
    await page.keyboard.press('Control+v');
    await expectParas(page, ['Đậm', 'Đậm']);
    await expect(page.getByTestId('paste-options')).toBeVisible();
    await page.keyboard.press('Shift+Home');
    await expect(cmd(page, 'format.bold')).toHaveAttribute('aria-pressed', 'true');
  });
});

test.describe('Insert tab', () => {
  test('Ctrl+Enter breaks the page; Tab in the last table cell adds a row', async ({ page }) => {
    await newDocument(page);
    await typeParas(page, ['Trang một']);
    await page.keyboard.press('Control+Enter');
    await page.keyboard.insertText('Trang hai');
    await expect(page.getByTestId('status-page')).toHaveText(/\/2$/);
    await tab(page, 'Chèn');
    await cmd(page, 'insert.table').click();
    await page.keyboard.insertText('A');
    for (let i = 0; i < 9; i++) await page.keyboard.press('Tab');
    await page.keyboard.insertText('B');
    await expect.poll(async () => mirror(page).locator('table tr').count()).toBe(4);
  });

  test('Ctrl+K links the selection; the menu then opens or removes the link', async ({ page }) => {
    await newDocument(page);
    await typeParas(page, ['Xem trang web']);
    await page.keyboard.press('End');
    await page.keyboard.press('Control+Shift+ArrowLeft');
    await page.keyboard.press('Control+Shift+ArrowLeft');
    await page.keyboard.press('Control+k');
    const dialog = page.getByTestId('doc-link-dialog');
    await expect(dialog).toBeVisible();
    await page.getByTestId('doc-link-address').fill('bluestar.vn');
    await page.getByTestId('doc-link-address').press('Enter');
    await expect(dialog).toHaveCount(0);
    await expect(mirror(page).locator('a')).toHaveAttribute('href', 'https://bluestar.vn');
    await expect(mirror(page).locator('a')).toHaveText('trang web');
    await expectParas(page, ['Xem trang web']);
    // Right click on the link: open / remove are offered; "remove" unlinks the text.
    const at = await pointAt(page, 6);
    await page.mouse.click(at.x, at.y, { button: 'right' });
    const menu = page.getByTestId('context-menu-text');
    await expect(menu.locator('[data-menu-command="insert.openLink"]')).toBeVisible();
    await menu.locator('[data-menu-command="insert.removeLink"]').click();
    await expect(mirror(page).locator('a')).toHaveCount(0);
    await expectParas(page, ['Xem trang web']);
  });

  test('Alt+Shift+D inserts the date as a field', async ({ page }) => {
    await newDocument(page);
    await page.keyboard.press('Alt+Shift+d');
    // A DATE field showing today's date (dd/MM/yyyy in vi-VN).
    await expect(mirror(page).locator('[role="img"]')).toHaveCount(1);
    await page.keyboard.press('Shift+Home');
    expect(await selected(page)).toMatch(/^\d{1,2}\/\d{1,2}\/\d{4}$/);
  });
});

test.describe('Review, View and the status bar', () => {
  test('F7 opens the spelling check; Ctrl+Shift+E turns tracking on and off', async ({ page }) => {
    await newDocument(page);
    await typeParas(page, ['Văn bản']);
    const track = page.getByTestId('status-track-changes');
    await expect(track).toHaveAttribute('aria-pressed', 'false');
    await page.keyboard.press('Control+Shift+e');
    await expect(track).toHaveAttribute('aria-pressed', 'true');
    await page.keyboard.press('Control+Shift+e');
    await expect(track).toHaveAttribute('aria-pressed', 'false');
    await page.keyboard.press('F7');
    await expect(page.getByTestId('spellcheck-close')).toBeVisible();
  });

  test('status bar: page x / y, words, selected words, language, views and zoom at the right', async ({
    page,
  }) => {
    await newDocument(page);
    await typeParas(page, ['Một hai ba bốn']);
    await expect(page.getByTestId('status-page')).toHaveText('Trang 1/1');
    await expect(page.getByTestId('status-words')).toHaveText('4 từ');
    await page.keyboard.press('Control+Shift+ArrowLeft');
    await page.keyboard.press('Control+Shift+ArrowLeft');
    await expect(page.getByTestId('status-words')).toHaveText('2 / 4 từ');
    await expect(page.getByTestId('status-language')).toBeVisible();
    const views = (await page.getByTestId('status-views').boundingBox())!;
    const slider = (await page.getByTestId('status-zoom-slider').boundingBox())!;
    const words = (await page.getByTestId('status-words').boundingBox())!;
    // Words on the left; view buttons, then the zoom slider at the right end.
    expect(words.x).toBeLessThan(views.x);
    expect(views.x).toBeLessThan(slider.x);
    // Ctrl+wheel zooms.
    const zoom = page.getByTestId('status-zoom');
    const before = await zoom.textContent();
    const pageBox = (await page.locator('.bo-page[data-page="1"]').boundingBox())!;
    await page.mouse.move(pageBox.x + 100, pageBox.y + 100);
    await page.keyboard.down('Control');
    await page.mouse.wheel(0, -100);
    await page.keyboard.up('Control');
    await expect(zoom).not.toHaveText(before ?? '');
  });

  test('mini toolbar after a mouse selection, not after a keyboard one', async ({ page }) => {
    await newDocument(page);
    await typeParas(page, ['Một hai ba']);
    await page.keyboard.press('Shift+Home');
    await page.waitForTimeout(300);
    await expect(page.getByTestId('mini-toolbar')).toHaveCount(0);
    const at = await pointAt(page, 5);
    await page.mouse.dblclick(at.x, at.y);
    await expect(page.getByTestId('mini-toolbar')).toBeVisible();
  });

  test('right click on text: clipboard first, then font and lists, then link and comment', async ({
    page,
  }) => {
    await newDocument(page);
    await typeParas(page, ['Xin chào bạn']);
    const at = await pointAt(page, 2);
    await page.mouse.click(at.x, at.y, { button: 'right' });
    const menu = page.getByTestId('context-menu-text');
    await expect(menu).toBeVisible();
    const ids = await menu
      .locator('[data-menu-command]')
      .evaluateAll((els) => els.map((e) => e.getAttribute('data-menu-command')));
    const order = [
      'edit.cut',
      'edit.copy',
      'edit.paste',
      'format.fontDialog',
      'insert.link',
      'review.newComment',
    ];
    expect(ids.filter((i) => order.includes(i ?? ''))).toEqual(order);
    // Open / remove link only on a link.
    expect(ids).not.toContain('insert.removeLink');
  });
});
