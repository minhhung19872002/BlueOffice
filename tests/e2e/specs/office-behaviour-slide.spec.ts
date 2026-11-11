import { expect, test, type Page } from '@playwright/test';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

// Everyday behaviour of BlueSlide, action by action, the way presentation programs of the
// OOXML family behave (docs/parity/office-behaviour-slide.md). Every expected value below is
// the reference behaviour, not what the app happened to do: real keys and real buttons on the
// running web app.

const here = dirname(fileURLToPath(import.meta.url));
// 960 × 540 pt deck: slide 1 = title slide (title at 108,81 720×162 + subtitle), slides 2–3 =
// title + bulleted body.
const deck = join(here, '..', '..', 'corpus', 'slide', 'slide-text-bullets.pptx');

/** Default grid step of a nudge (1/12 inch) and the fine step, in points. */
const GRID = 6;
const FINE = 1;
/** The font size list of the reference (points). */
const SIZES = [
  8, 9, 10, 10.5, 11, 12, 14, 16, 18, 20, 24, 28, 32, 36, 40, 44, 48, 54, 60, 66, 72, 80, 88, 96,
];

async function openDeck(page: Page, path = deck) {
  await page.goto('/slide');
  const chooser = page.waitForEvent('filechooser');
  await page.getByRole('button', { name: 'Mở', exact: true }).click();
  await (await chooser).setFiles(path);
  await expect(page.getByTestId('slide-status')).toHaveText(/^Slide 1 \/ \d+$/);
  await expect(page.getByTestId('slide-overlay')).toBeVisible();
}

const thumbs = (page: Page) => page.locator('[data-testid^="thumb-"]');
const status = (page: Page) => page.getByTestId('slide-status');
const editor = (page: Page) => page.getByTestId('slide-text-editor');
const command = (page: Page, id: string) => page.locator(`[data-command="${id}"]`).first();

/** Selected objects: `[x, y, width, height]` in slide points, and whether each is rotated. */
async function selection(page: Page) {
  return page.getByTestId('selection-box').evaluateAll((els) =>
    els.map((e) => ({
      box: ['x', 'y', 'width', 'height'].map(
        (a) => Math.round(Number(e.getAttribute(a)) * 100) / 100,
      ),
      transform: e.getAttribute('transform') ?? '',
    })),
  );
}
const boxes = async (page: Page) => (await selection(page)).map((s) => s.box);

/** Title of slide `n` (1-based) as the slide panel names it. */
async function titleOf(page: Page, n: number) {
  const label = (await page.getByTestId(`thumb-${n}`).getAttribute('aria-label')) ?? '';
  return label.replace(/^[^:]*:\s*/, '');
}

/** Titles of the first `n` slides, once the slide panel has read them. */
async function titles(page: Page, n: number) {
  const all = () => Promise.all(Array.from({ length: n }, (_, i) => titleOf(page, i + 1)));
  await expect.poll(async () => (await all()).every((t) => !/^Slide \d+$/.test(t))).toBe(true);
  return all();
}

/** Names of the shapes of the shown slide. */
async function shapeNames(page: Page) {
  return page
    .locator('[data-testid="slide-a11y-mirror"] [data-shape]')
    .evaluateAll((els) => els.map((e) => e.getAttribute('aria-label') ?? ''));
}

async function clickSlide(page: Page, fx: number, fy: number, options = {}) {
  const o = (await page.getByTestId('slide-overlay').boundingBox())!;
  await page.mouse.click(o.x + o.width * fx, o.y + o.height * fy, options);
}

/** Click the title of slide 1 (its box is 108,81 720×162). */
async function selectTitle(page: Page) {
  await clickSlide(page, 0.48, 0.3);
  await expect.poll(() => boxes(page)).toEqual([[108, 81, 720, 162]]);
}

async function tab(page: Page, name: string) {
  await page.getByRole('tab', { name, exact: true }).click();
}

/** Text and selection of the text being edited. */
async function editState(page: Page) {
  return editor(page).evaluate((e) => {
    const t = e as HTMLTextAreaElement;
    return { text: t.value, start: t.selectionStart, end: t.selectionEnd };
  });
}

test.describe('Slide panel', () => {
  test('Enter and Ctrl+M add a slide after the current one, with the layout rule', async ({
    page,
  }) => {
    await openDeck(page);
    await page.getByTestId('thumb-1').click();
    // After a title slide the new slide is "title and content"…
    await page.keyboard.press('Enter');
    await expect(thumbs(page)).toHaveCount(4);
    await expect(status(page)).toHaveText('Slide 2 / 4');
    await expect.poll(() => shapeNames(page)).toHaveLength(2);
    await expect
      .poll(async () => (await shapeNames(page)).join('|'))
      .toContain('Content Placeholder');
    // …and after any other slide it has that slide's layout.
    await page.keyboard.press('Enter');
    await expect(thumbs(page)).toHaveCount(5);
    await expect(status(page)).toHaveText('Slide 3 / 5');
    await expect
      .poll(async () => (await shapeNames(page)).join('|'))
      .toContain('Content Placeholder');
    await page.keyboard.press('Control+m');
    await expect(thumbs(page)).toHaveCount(6);
    await expect(status(page)).toHaveText('Slide 4 / 6');
    // The keyboard stays on the new slide's thumbnail.
    await expect(page.getByTestId('thumb-4')).toBeFocused();
    // One undo step per new slide.
    await page.keyboard.press('Control+z');
    await expect(thumbs(page)).toHaveCount(5);
  });

  test('Ctrl+D / Ctrl+Shift+D duplicate, Delete / Backspace delete the slide', async ({ page }) => {
    await openDeck(page);
    const [, second] = await titles(page, 2);
    await page.getByTestId('thumb-2').click();
    await page.keyboard.press('Control+d');
    // The copy follows the original and becomes the current slide.
    await expect(thumbs(page)).toHaveCount(4);
    await expect(status(page)).toHaveText('Slide 3 / 4');
    await expect.poll(() => titleOf(page, 3)).toBe(second);
    await page.keyboard.press('Control+Shift+d');
    await expect(thumbs(page)).toHaveCount(5);
    await expect(status(page)).toHaveText('Slide 4 / 5');
    await page.keyboard.press('Backspace');
    await expect(thumbs(page)).toHaveCount(4);
    await page.keyboard.press('Delete');
    await expect(thumbs(page)).toHaveCount(3);
    // The slide after the deleted one is shown (the last one when there is none).
    await expect(status(page)).toHaveText('Slide 3 / 3');
  });

  test('Ctrl+↑ / ↓ move the slide by one, Ctrl+Shift+↑ / ↓ to the start / end', async ({
    page,
  }) => {
    await openDeck(page);
    const [a, b, c] = await titles(page, 3);
    await page.getByTestId('thumb-3').click();
    await page.keyboard.press('Control+ArrowUp');
    await expect(status(page)).toHaveText('Slide 2 / 3');
    await expect.poll(() => titleOf(page, 2)).toBe(c);
    await expect.poll(() => titleOf(page, 3)).toBe(b);
    await page.keyboard.press('Control+Shift+ArrowDown');
    await expect(status(page)).toHaveText('Slide 3 / 3');
    await expect.poll(() => titleOf(page, 3)).toBe(c);
    await page.keyboard.press('Control+Shift+ArrowUp');
    await expect(status(page)).toHaveText('Slide 1 / 3');
    await expect.poll(() => titleOf(page, 1)).toBe(c);
    expect([await titleOf(page, 2), await titleOf(page, 3)]).toEqual([a, b]);
    await page.keyboard.press('Control+ArrowDown');
    await expect(status(page)).toHaveText('Slide 2 / 3');
    await expect.poll(() => titleOf(page, 2)).toBe(c);
  });

  test('arrows, Page Up / Down, Home and End go through the slides', async ({ page }) => {
    await openDeck(page);
    await page.getByTestId('thumb-1').click();
    await page.keyboard.press('ArrowDown');
    await expect(status(page)).toHaveText('Slide 2 / 3');
    await page.keyboard.press('PageDown');
    await expect(status(page)).toHaveText('Slide 3 / 3');
    await page.keyboard.press('ArrowUp');
    await expect(status(page)).toHaveText('Slide 2 / 3');
    await page.keyboard.press('Home');
    await expect(status(page)).toHaveText('Slide 1 / 3');
    await page.keyboard.press('End');
    await expect(status(page)).toHaveText('Slide 3 / 3');
    await page.keyboard.press('PageUp');
    await expect(status(page)).toHaveText('Slide 2 / 3');
  });

  test('Hide slide: a toggle, marked in the panel, skipped by the show', async ({ page }) => {
    await openDeck(page);
    await page.getByTestId('thumb-2').click({ button: 'right' });
    const items = page.locator('[role="menu"] [role^="menuitem"]');
    await items.filter({ hasText: 'Ẩn slide' }).click();
    await expect(page.getByTestId('thumb-2')).toHaveAttribute('data-hidden', 'true');
    await expect(command(page, 'slide.hide')).toHaveAttribute('aria-pressed', 'true');
    // The slide stays in the deck, selected; only the show leaves it out.
    await expect(thumbs(page)).toHaveCount(3);
    await expect(status(page)).toHaveText('Slide 2 / 3');
    await page.keyboard.press('F5');
    const stage = page.getByTestId('show-stage');
    await expect(stage).toHaveAttribute('data-slide', '0');
    await page.keyboard.press('ArrowRight');
    await expect(stage).toHaveAttribute('data-slide', '2');
    await page.keyboard.press('Escape');
    await expect(stage).toHaveCount(0);
    // The same command shows it again; undo hides it once more.
    await page.getByTestId('thumb-2').click();
    await command(page, 'slide.hide').click();
    await expect(page.getByTestId('thumb-2')).not.toHaveAttribute('data-hidden', 'true');
    await page.keyboard.press('Control+z');
    await expect(page.getByTestId('thumb-2')).toHaveAttribute('data-hidden', 'true');
  });

  test('context menu of a thumbnail: new, duplicate, delete, cut / copy / paste', async ({
    page,
  }) => {
    await openDeck(page);
    await page.getByTestId('thumb-2').click({ button: 'right' });
    const items = page.locator('[role="menu"] [role^="menuitem"]');
    await expect(items.first()).toBeVisible();
    const text = (await items.allInnerTexts()).join('|');
    for (const name of [
      'Slide mới',
      'Nhân bản slide',
      'Xoá slide',
      'Cắt slide',
      'Sao chép slide',
    ]) {
      expect(text).toContain(name);
    }
    // The right click made slide 2 the current slide.
    await items.filter({ hasText: 'Nhân bản slide' }).click();
    await expect(thumbs(page)).toHaveCount(4);
    await expect(status(page)).toHaveText('Slide 3 / 4');
  });
});

test.describe('Objects on the slide', () => {
  test('typing on a selected shape edits its text; Esc: text → shape → nothing', async ({
    page,
  }) => {
    await openDeck(page);
    await selectTitle(page);
    // Typing goes into the shape, after the text it has.
    await page.keyboard.type('abc');
    await expect(editor(page)).toBeFocused();
    await expect(editor(page)).toHaveValue('Báo cáo kết quả kinh doanhabc');
    // Esc leaves the text with the shape still selected, a second Esc selects nothing.
    await page.keyboard.press('Escape');
    await expect(editor(page)).toHaveCount(0);
    await expect.poll(() => boxes(page)).toEqual([[108, 81, 720, 162]]);
    await page.keyboard.press('Escape');
    await expect(page.getByTestId('selection-box')).toHaveCount(0);
    // The typing is one undo step.
    await page.keyboard.press('Control+z');
    await selectTitle(page);
    await page.keyboard.press('F2');
    await expect(editor(page)).toHaveValue('Báo cáo kết quả kinh doanh');
  });

  test('Enter and F2 select all the text of the selected shape; F2 goes back', async ({ page }) => {
    await openDeck(page);
    await selectTitle(page);
    await page.keyboard.press('Enter');
    await expect(editor(page)).toBeFocused();
    expect(await editState(page)).toEqual({
      text: 'Báo cáo kết quả kinh doanh',
      start: 0,
      end: 26,
    });
    await page.keyboard.press('F2');
    await expect(editor(page)).toHaveCount(0);
    await expect.poll(() => boxes(page)).toEqual([[108, 81, 720, 162]]);
    await page.keyboard.press('F2');
    await expect(editor(page)).toBeFocused();
    expect(await editState(page)).toMatchObject({ start: 0, end: 26 });
    // Typing replaces the selected text.
    await page.keyboard.type('Mới');
    await expect(editor(page)).toHaveValue('Mới');
  });

  test('arrows nudge by the grid step, Ctrl is the fine step, one undo step each', async ({
    page,
  }) => {
    await openDeck(page);
    await selectTitle(page);
    await page.keyboard.press('ArrowRight');
    await expect.poll(() => boxes(page)).toEqual([[108 + GRID, 81, 720, 162]]);
    await page.keyboard.press('ArrowDown');
    await expect.poll(() => boxes(page)).toEqual([[108 + GRID, 81 + GRID, 720, 162]]);
    await page.keyboard.press('Control+ArrowLeft');
    await expect.poll(() => boxes(page)).toEqual([[108 + GRID - FINE, 81 + GRID, 720, 162]]);
    await page.keyboard.press('Control+ArrowUp');
    await expect.poll(() => boxes(page)).toEqual([[108 + GRID - FINE, 81 + GRID - FINE, 720, 162]]);
    await page.keyboard.press('Control+z');
    await expect.poll(() => boxes(page)).toEqual([[108 + GRID - FINE, 81 + GRID, 720, 162]]);
    for (let i = 0; i < 3; i++) await page.keyboard.press('Control+z');
    await expect.poll(() => boxes(page)).toEqual([[108, 81, 720, 162]]);
  });

  test('Shift+arrows resize, Alt+← / → rotate by 15°', async ({ page }) => {
    await openDeck(page);
    await selectTitle(page);
    // → wider, ← narrower, ↑ taller, ↓ shorter.
    await page.keyboard.press('Shift+ArrowRight');
    await expect.poll(() => boxes(page)).toEqual([[108, 81, 720 + GRID, 162]]);
    await page.keyboard.press('Shift+ArrowUp');
    await expect.poll(() => boxes(page)).toEqual([[108, 81, 720 + GRID, 162 + GRID]]);
    await page.keyboard.press('Shift+ArrowLeft');
    await page.keyboard.press('Shift+ArrowDown');
    await expect.poll(() => boxes(page)).toEqual([[108, 81, 720, 162]]);
    await page.keyboard.press('Control+Shift+ArrowRight');
    await expect.poll(() => boxes(page)).toEqual([[108, 81, 720 + FINE, 162]]);
    await page.keyboard.press('Control+z');
    // Alt+→: 15° clockwise around the centre (468, 162); Alt+← back.
    await page.keyboard.press('Alt+ArrowRight');
    await expect.poll(async () => (await selection(page))[0]?.transform).toBe('rotate(15 468 162)');
    await page.keyboard.press('Alt+ArrowRight');
    await expect.poll(async () => (await selection(page))[0]?.transform).toBe('rotate(30 468 162)');
    await page.keyboard.press('Alt+ArrowLeft');
    await page.keyboard.press('Alt+ArrowLeft');
    await expect.poll(async () => (await selection(page))[0]?.transform).toBe('');
  });

  test('Tab cycles the objects, Ctrl+A selects all, Delete removes, Ctrl+D duplicates', async ({
    page,
  }) => {
    await openDeck(page);
    await selectTitle(page);
    await page.keyboard.press('Tab');
    await expect.poll(() => boxes(page)).toEqual([[108, 252, 720, 108]]);
    await page.keyboard.press('Shift+Tab');
    await expect.poll(() => boxes(page)).toEqual([[108, 81, 720, 162]]);
    await page.keyboard.press('Control+a');
    await expect(page.getByTestId('selection-box')).toHaveCount(2);
    await page.keyboard.press('Escape');
    await expect(page.getByTestId('selection-box')).toHaveCount(0);

    // A new shape: Ctrl+D puts a copy a little down-right and selects the copy.
    await tab(page, 'Chèn');
    await page.getByRole('button', { name: 'Hình chữ nhật' }).click();
    // The button arms the pointer; a click on the slide puts the shape there.
    await clickSlide(page, 0.05, 0.85);
    await expect.poll(() => boxes(page)).toHaveLength(1);
    const [first] = await boxes(page);
    await page.keyboard.press('Control+d');
    await expect.poll(async () => (await boxes(page))[0]?.[0]).toBeGreaterThan(first![0]!);
    const [copy] = await boxes(page);
    expect(copy![1]!).toBeGreaterThan(first![1]!);
    expect(copy!.slice(2)).toEqual(first!.slice(2));
    await page.keyboard.press('Control+a');
    await expect(page.getByTestId('selection-box')).toHaveCount(4);
    await page.keyboard.press('Escape');

    // Delete removes the selected object; Ctrl+Z brings it back.
    await page.keyboard.press('Tab');
    await expect(page.getByTestId('selection-box')).toHaveCount(1);
    await page.keyboard.press('Delete');
    await expect(page.getByTestId('selection-box')).toHaveCount(0);
    await page.keyboard.press('Control+a');
    await expect(page.getByTestId('selection-box')).toHaveCount(3);
    await page.keyboard.press('Control+z');
    await page.keyboard.press('Control+a');
    await expect(page.getByTestId('selection-box')).toHaveCount(4);
  });

  test('Shift+drag keeps one axis, Ctrl+drag copies, Ctrl+click adds, Ctrl+G groups', async ({
    page,
  }) => {
    await openDeck(page);
    await page.getByTestId('thumb-2').click();
    await tab(page, 'Chèn');
    await page.getByRole('button', { name: 'Hình chữ nhật' }).click();
    await clickSlide(page, 0.3, 0.7);
    await expect.poll(() => boxes(page)).toHaveLength(1);
    const [start] = await boxes(page);
    const scale = (await page.getByTestId('slide-overlay').boundingBox())!.width / 960;

    // Shift held while dragging: the shape only moves horizontally.
    let b = (await page.getByTestId('selection-box').first().boundingBox())!;
    let cx = b.x + b.width / 2;
    let cy = b.y + b.height / 2;
    await page.mouse.move(cx, cy);
    await page.mouse.down();
    await page.keyboard.down('Shift');
    await page.mouse.move(cx + 40, cy + 9, { steps: 4 });
    await page.mouse.move(cx + 83, cy + 23, { steps: 4 });
    await page.mouse.up({});
    await page.keyboard.up('Shift');
    await expect.poll(async () => (await boxes(page))[0]?.[0]).toBeGreaterThan(start![0]! + 60);
    const [moved] = await boxes(page);
    expect(moved![1]).toBe(start![1]);

    // Ctrl+drag leaves the original and drops a copy; the copy is selected; one undo step.
    b = (await page.getByTestId('selection-box').first().boundingBox())!;
    cx = b.x + b.width / 2;
    cy = b.y + b.height / 2;
    await page.keyboard.down('Control');
    await page.mouse.move(cx, cy);
    await page.mouse.down();
    await page.mouse.move(cx + 10, cy + 40, { steps: 4 });
    await page.mouse.move(cx + 20, cy + 93, { steps: 4 });
    await page.mouse.up();
    await page.keyboard.up('Control');
    await expect.poll(async () => (await boxes(page))[0]?.[1]).toBeGreaterThan(moved![1]! + 60);
    const [copy] = await boxes(page);
    expect(copy!.slice(2)).toEqual(moved!.slice(2));
    // The drop point: the pointer offset (the copy may snap to a guide by a few pixels).
    expect(Math.abs(copy![1]! - (moved![1]! + 93 / scale))).toBeLessThan(8 / scale);
    await page.keyboard.press('Control+a');
    await expect(page.getByTestId('selection-box')).toHaveCount(4);
    await page.keyboard.press('Control+z');
    await page.keyboard.press('Control+a');
    await expect(page.getByTestId('selection-box')).toHaveCount(3);
    await page.keyboard.press('Control+y');
    await page.keyboard.press('Escape');

    // Click the copy, Ctrl+click the original: both are selected; Ctrl+click again takes it out.
    const at = async (box: number[]) => {
      const o = (await page.getByTestId('slide-overlay').boundingBox())!;
      return { x: o.x + (box[0]! + 6) * scale, y: o.y + (box[1]! + 6) * scale };
    };
    const pc = await at(copy!);
    const pm = await at(moved!);
    await page.mouse.click(pc.x, pc.y);
    await expect(page.getByTestId('selection-box')).toHaveCount(1);
    await page.keyboard.down('Control');
    await page.mouse.click(pm.x, pm.y);
    await expect(page.getByTestId('selection-box')).toHaveCount(2);
    await page.mouse.click(pm.x, pm.y);
    await expect(page.getByTestId('selection-box')).toHaveCount(1);
    await page.mouse.click(pm.x, pm.y);
    await page.keyboard.up('Control');
    await expect(page.getByTestId('selection-box')).toHaveCount(2);

    // Ctrl+G: one group; Ctrl+Shift+G: the two shapes again, both selected.
    await page.keyboard.press('Control+g');
    await expect(page.getByTestId('selection-box')).toHaveCount(1);
    await page.keyboard.press('Control+Shift+g');
    await expect(page.getByTestId('selection-box')).toHaveCount(2);
  });

  test('context menu of the empty slide and of a shape', async ({ page }) => {
    await openDeck(page);
    await clickSlide(page, 0.01, 0.02, { button: 'right' });
    const menu = page.locator('[role="menu"]');
    await expect(menu).toBeVisible();
    for (const name of ['Dán', 'Chọn tất cả', 'Slide mới']) await expect(menu).toContainText(name);
    await page.keyboard.press('Escape');
    // Right click on a shape selects it and offers cut / copy / paste, order and delete.
    await clickSlide(page, 0.48, 0.3, { button: 'right' });
    await expect(menu).toBeVisible();
    await expect.poll(() => boxes(page)).toEqual([[108, 81, 720, 162]]);
    for (const name of ['Cắt', 'Sao chép', 'Dán', 'Nhân bản', 'Xoá', 'Đưa lên trên cùng']) {
      await expect(menu).toContainText(name);
    }
  });
});

test.describe('Home: text', () => {
  test('Ctrl+B / I / U toggle, Ctrl+E / L / R / J align the paragraph', async ({ page }) => {
    await openDeck(page);
    await selectTitle(page);
    await page.keyboard.press('F2');
    await expect(editor(page)).toBeFocused();
    for (const [key, id] of [
      ['b', 'format.bold'],
      ['i', 'format.italic'],
      ['u', 'format.underline'],
    ] as const) {
      await expect(command(page, id)).toHaveAttribute('aria-pressed', 'false');
      await page.keyboard.press(`Control+${key}`);
      await expect(command(page, id)).toHaveAttribute('aria-pressed', 'true');
      await page.keyboard.press(`Control+${key}`);
      await expect(command(page, id)).toHaveAttribute('aria-pressed', 'false');
    }
    for (const [key, id] of [
      ['l', 'format.alignLeft'],
      ['r', 'format.alignRight'],
      ['j', 'format.justify'],
      ['e', 'format.alignCenter'],
    ] as const) {
      await page.keyboard.press(`Control+${key}`);
      await expect(command(page, id)).toHaveAttribute('aria-pressed', 'true');
    }
    await expect(command(page, 'format.justify')).toHaveAttribute('aria-pressed', 'false');
    await expect(editor(page)).toBeFocused();
  });

  test('font size: the size box, Ctrl+Shift+> / < and Ctrl+] / [ step through the list', async ({
    page,
  }) => {
    await openDeck(page);
    await selectTitle(page);
    await page.keyboard.press('F2');
    const size = page.getByTestId('slide-font-size');
    await size.selectOption('44');
    await expect(size).toHaveValue('44');
    await expect(editor(page)).toBeFocused();
    const next = (v: number) => SIZES[SIZES.indexOf(v) + 1]!;
    // 44 → 48 → 54 (the next size of the list, not +2 points).
    await page.keyboard.press('Control+Shift+Period');
    await expect(size).toHaveValue(String(next(44)));
    await page.keyboard.press('Control+]');
    await expect(size).toHaveValue(String(next(next(44))));
    await page.keyboard.press('Control+[');
    await expect(size).toHaveValue('48');
    await page.keyboard.press('Control+Shift+Comma');
    await expect(size).toHaveValue('44');
    await page.getByRole('button', { name: 'Tăng cỡ chữ' }).click();
    await expect(size).toHaveValue('48');
    await page.getByRole('button', { name: 'Giảm cỡ chữ' }).click();
    await page.getByRole('button', { name: 'Giảm cỡ chữ' }).click();
    await expect(size).toHaveValue('40');
  });

  test('Ctrl+Enter goes to the next placeholder, then to a new slide', async ({ page }) => {
    await openDeck(page);
    await selectTitle(page);
    // From the selected title: the subtitle, its text selected.
    await page.keyboard.press('Control+Enter');
    await expect(editor(page)).toHaveValue('Công ty giả định ABC – 2026');
    expect(await editState(page)).toMatchObject({ start: 0, end: 27 });
    // From the last placeholder: a new slide, the caret in its title.
    await page.keyboard.press('Control+Enter');
    await expect(thumbs(page)).toHaveCount(4);
    await expect(status(page)).toHaveText('Slide 2 / 4');
    await expect(editor(page)).toHaveValue('');
    await expect(editor(page)).toBeFocused();
    await page.keyboard.type('Tiêu đề mới');
    await page.keyboard.press('Control+Enter');
    await expect(editor(page)).toHaveValue('');
    // The body is a bulleted list: Enter starts the next bullet, Tab / Shift+Tab change the
    // level of the paragraph.
    await expect(command(page, 'format.bullets')).toHaveAttribute('aria-pressed', 'true');
    await page.keyboard.type('Một');
    await page.keyboard.press('Enter');
    await page.keyboard.press('Tab');
    await page.keyboard.type('Hai');
    await expect(editor(page)).toHaveValue('Một\nHai');
    await expect(command(page, 'format.outdent')).toBeEnabled();
    await page.keyboard.press('Shift+Tab');
    await page.keyboard.press('Escape');
    await expect.poll(() => titleOf(page, 2)).toBe('Tiêu đề mới');
  });

  test('double-click selects the word, triple-click the paragraph of the edited text', async ({
    page,
  }) => {
    await openDeck(page);
    await page.getByTestId('thumb-2').click();
    // The body of slide 2 (36,126 864×306): its first paragraph is "Mục tiêu năm 2026".
    await clickSlide(page, 0.5, 0.55);
    await expect.poll(() => boxes(page)).toEqual([[36, 126, 864, 306]]);
    await page.keyboard.press('F2');
    await expect(editor(page)).toBeFocused();
    const text = (await editState(page)).text;
    const first = text.split('\n')[0]!;
    expect(first).toBe('Mục tiêu năm 2026');
    // A point inside the first line, a little right of the bullet.
    const o = (await page.getByTestId('slide-overlay').boundingBox())!;
    const scale = o.width / 960;
    const x = o.x + (36 + 70) * scale;
    const y = o.y + (126 + 22) * scale;
    await page.mouse.dblclick(x, y);
    await expect
      .poll(async () => (await editState(page)).end - (await editState(page)).start)
      .toBeGreaterThan(0);
    const word = await editState(page);
    const picked = word.text.slice(word.start, word.end);
    expect(first.split(' ')).toContain(picked);
    await page.mouse.click(x, y, { clickCount: 3 });
    await expect.poll(() => editState(page)).toMatchObject({ start: 0, end: first.length });
    // Typing replaces the selected paragraph text.
    await page.keyboard.type('Mới');
    await expect.poll(async () => (await editState(page)).text.split('\n')[0]).toBe('Mới');
  });

  test('Backspace at the start of a bullet removes the bullet, then joins the paragraphs', async ({
    page,
  }) => {
    await openDeck(page);
    await page.getByTestId('thumb-2').click();
    await page.keyboard.press('Control+m');
    await expect(thumbs(page)).toHaveCount(4);
    // The body placeholder of the new slide (title and content).
    await clickSlide(page, 0.5, 0.6);
    await expect(page.getByTestId('selection-box')).toHaveCount(1);
    await page.keyboard.type('Một');
    await expect(editor(page)).toBeFocused();
    await page.keyboard.press('Enter');
    await page.keyboard.type('Hai');
    await expect(editor(page)).toHaveValue('Một\nHai');
    await expect(command(page, 'format.bullets')).toHaveAttribute('aria-pressed', 'true');
    await page.keyboard.press('Home');
    await page.keyboard.press('Backspace');
    // The text is untouched, the paragraph is no longer a bullet.
    await expect(command(page, 'format.bullets')).toHaveAttribute('aria-pressed', 'false');
    await expect(editor(page)).toHaveValue('Một\nHai');
    await page.keyboard.press('Backspace');
    await expect(editor(page)).toHaveValue('MộtHai');
    await expect(command(page, 'format.bullets')).toHaveAttribute('aria-pressed', 'true');
  });

  test('a new text box takes the typing at once and grows with its text', async ({ page }) => {
    await openDeck(page);
    await tab(page, 'Chèn');
    await page.getByRole('button', { name: 'Hộp văn bản' }).click();
    await clickSlide(page, 0.05, 0.85);
    await expect.poll(() => boxes(page)).toHaveLength(1);
    const [empty] = await boxes(page);
    await page.keyboard.type('Xin');
    await expect(editor(page)).toBeFocused();
    await expect(editor(page)).toHaveValue('Xin');
    await page.keyboard.press('Enter');
    await page.keyboard.type('chào');
    await page.keyboard.press('Enter');
    await page.keyboard.type('bạn');
    await page.keyboard.press('Escape');
    // The box resizes to fit its text: three lines are much taller than the one-line box.
    await expect.poll(async () => (await boxes(page))[0]?.[3]).toBeGreaterThan(empty![3]! * 1.6);
    expect((await boxes(page))[0]![1]).toBe(empty![1]);
  });
});

test.describe('Find and replace', () => {
  test('Ctrl+F finds through the slides, Ctrl+H replaces one and all', async ({ page }) => {
    await openDeck(page);
    await page.getByTestId('thumb-1').click();
    await page.keyboard.press('Control+f');
    const dialog = page.getByTestId('slide-find-dialog');
    await expect(dialog).toBeVisible();
    await page.getByTestId('slide-find-what').fill('doanh');
    await page.keyboard.press('Enter');
    // "doanh" is in the title of slide 1 ("…kinh doanh"): its shape gets selected.
    await expect(page.getByTestId('slide-find-status')).toHaveText('Tìm thấy ở slide 1.');
    await expect.poll(() => boxes(page)).toEqual([[108, 81, 720, 162]]);
    // The next one is on slide 2 ("Tăng doanh thu 15%").
    await page.getByTestId('slide-find-next').click();
    await expect(page.getByTestId('slide-find-status')).toHaveText('Tìm thấy ở slide 2.');
    await expect(status(page)).toHaveText('Slide 2 / 3');
    await page.getByTestId('slide-find-what').fill('không có chuỗi này');
    await page.getByTestId('slide-find-next').click();
    await expect(page.getByTestId('slide-find-status')).toHaveText('Không tìm thấy.');
    await page.keyboard.press('Escape');
    await expect(dialog).toHaveCount(0);

    // Replace all: every match of the deck, one undo step, a count.
    await page.keyboard.press('Control+h');
    await page.getByTestId('slide-find-what').fill('doanh');
    await page.getByTestId('slide-find-with').fill('DOANH');
    await page.getByTestId('slide-find-case').check();
    await page.getByTestId('slide-find-replace-all').click();
    await expect(page.getByTestId('slide-find-status')).toHaveText(/^Đã thay \d+ chỗ\.$/);
    const count = Number(
      (await page.getByTestId('slide-find-status').innerText()).replace(/\D/g, ''),
    );
    expect(count).toBeGreaterThanOrEqual(2);
    // Nothing is left to find (the search is case sensitive now).
    await page.getByTestId('slide-find-next').click();
    await expect(page.getByTestId('slide-find-status')).toHaveText('Không tìm thấy.');
    await page.keyboard.press('Escape');
    await expect.poll(() => titleOf(page, 1)).toBe('Báo cáo kết quả kinh DOANH');
    await page.keyboard.press('Control+z');
    await expect.poll(() => titleOf(page, 1)).toBe('Báo cáo kết quả kinh doanh');
  });
});

test.describe('Slide show', () => {
  const shown = (page: Page) => page.getByTestId('show-stage');

  test('F5 from the start, Shift+F5 from the current slide, keys of the show', async ({ page }) => {
    await openDeck(page);
    await page.getByTestId('thumb-2').click();
    await page.keyboard.press('F5');
    await expect(shown(page)).toHaveAttribute('data-slide', '0');
    // N, Space, →, ↓, Enter, Page Down: next; P, Backspace, ←, ↑, Page Up: previous.
    await page.keyboard.press('n');
    await expect(shown(page)).toHaveAttribute('data-slide', '1');
    await page.keyboard.press('p');
    await expect(shown(page)).toHaveAttribute('data-slide', '0');
    await page.keyboard.press('Space');
    await expect(shown(page)).toHaveAttribute('data-slide', '1');
    await page.keyboard.press('Backspace');
    await expect(shown(page)).toHaveAttribute('data-slide', '0');
    await page.keyboard.press('ArrowRight');
    await page.keyboard.press('ArrowDown');
    await expect(shown(page)).toHaveAttribute('data-slide', '2');
    await page.keyboard.press('ArrowLeft');
    await page.keyboard.press('ArrowUp');
    await expect(shown(page)).toHaveAttribute('data-slide', '0');
    await page.keyboard.press('Enter');
    await page.keyboard.press('PageDown');
    await expect(shown(page)).toHaveAttribute('data-slide', '2');
    await page.keyboard.press('PageUp');
    await expect(shown(page)).toHaveAttribute('data-slide', '1');
    // A slide number then Enter goes to that slide.
    await page.keyboard.press('3');
    await page.keyboard.press('Enter');
    await expect(shown(page)).toHaveAttribute('data-slide', '2');
    await page.keyboard.press('1');
    await page.keyboard.press('Enter');
    await expect(shown(page)).toHaveAttribute('data-slide', '0');
    await page.keyboard.press('End');
    await expect(shown(page)).toHaveAttribute('data-slide', '2');
    await page.keyboard.press('Home');
    await expect(shown(page)).toHaveAttribute('data-slide', '0');
    // B / W: black / white screen, the same key brings the slide back.
    const surface = page.getByTestId('show-surface');
    await page.keyboard.press('b');
    await expect(surface).toHaveAttribute('data-cover', 'black');
    await page.keyboard.press('b');
    await expect(surface).toHaveAttribute('data-cover', 'none');
    await page.keyboard.press('w');
    await expect(surface).toHaveAttribute('data-cover', 'white');
    await page.keyboard.press('w');
    await expect(surface).toHaveAttribute('data-cover', 'none');
    // Ctrl+P: pen; Esc gives the arrow back, the next Esc ends the show.
    await page.keyboard.press('Control+p');
    await expect(surface).toHaveAttribute('data-tool', 'pen');
    await page.keyboard.press('Escape');
    await expect(surface).toHaveAttribute('data-tool', 'pointer');
    await expect(shown(page)).toBeVisible();
    await page.keyboard.press('ArrowRight');
    await page.keyboard.press('Escape');
    await expect(shown(page)).toHaveCount(0);
    // Back in the editor on the slide that was shown last.
    await expect(status(page)).toHaveText('Slide 2 / 3');

    await page.getByTestId('thumb-3').click();
    await page.keyboard.press('Shift+F5');
    await expect(shown(page)).toHaveAttribute('data-slide', '2');
    // After the last slide: the end screen, one more step leaves the show.
    await page.keyboard.press('ArrowRight');
    await page.keyboard.press('ArrowRight');
    await expect(shown(page)).toHaveCount(0);
    await expect(status(page)).toHaveText('Slide 3 / 3');
  });
});

test.describe('File, View, status bar', () => {
  test('a new presentation starts with one title slide; Ctrl+S saves it as .pptx', async ({
    page,
  }) => {
    await page.goto('/slide');
    await page.getByTestId('new-presentation').click();
    await expect(status(page)).toHaveText('Slide 1 / 1');
    await expect(thumbs(page)).toHaveCount(1);
    // Title slide: two empty placeholders (title, subtitle); typing after a click goes in.
    await expect(
      page.locator('[data-testid="slide-overlay"] rect[class*="placeholder"]'),
    ).toHaveCount(2);
    await clickSlide(page, 0.5, 0.4);
    await expect(page.getByTestId('selection-box')).toHaveCount(1);
    await page.keyboard.type('Kế hoạch 2027');
    await expect(editor(page)).toHaveValue('Kế hoạch 2027');
    await page.keyboard.press('Escape');
    await expect(page.getByTestId('status-modified')).toBeVisible();
    const download = page.waitForEvent('download');
    await page.keyboard.press('Control+s');
    expect((await download).suggestedFilename()).toBe('Bản trình chiếu.pptx');
    await expect(page.getByTestId('status-modified')).toHaveCount(0);
    // Ctrl+M after the title slide: title and content.
    await page.keyboard.press('Control+m');
    await expect(thumbs(page)).toHaveCount(2);
    await expect(page.getByTestId('thumb-2')).toHaveAttribute('aria-current', 'true');
    await expect
      .poll(async () => (await shapeNames(page)).join('|'))
      .toContain('Content Placeholder');
  });

  test('views: slide sorter, reading view (Esc returns), notes, undo / redo buttons', async ({
    page,
  }) => {
    await openDeck(page);
    await tab(page, 'Xem');
    await page.getByRole('button', { name: 'Sắp xếp slide', exact: true }).click();
    await expect(page.getByTestId('slide-overlay')).toHaveCount(0);
    await page.getByRole('button', { name: 'Thường', exact: true }).click();
    await expect(page.getByTestId('slide-overlay')).toBeVisible();
    await page.getByRole('button', { name: 'Chế độ đọc', exact: true }).click();
    await expect(page.getByTestId('reading-view')).toBeVisible();
    await page.keyboard.press('ArrowRight');
    await page.keyboard.press('Escape');
    await expect(page.getByTestId('reading-view')).toHaveCount(0);
    // Reading view returns to the slide it showed last.
    await expect(status(page)).toHaveText('Slide 2 / 3');

    // Notes under the slide; the Home tab's undo / redo follow the history.
    const notes = page.getByTestId('notes-editor');
    await notes.click();
    await page.keyboard.insertText('Ghi chú của tôi');
    await page.getByTestId('thumb-1').click();
    await page.getByTestId('thumb-2').click();
    await expect(notes).toHaveValue('Ghi chú của tôi');
    await tab(page, 'Trang đầu');
    await expect(command(page, 'edit.undo')).toBeEnabled();
    await command(page, 'edit.undo').click();
    await expect(notes).toHaveValue('');
    await expect(command(page, 'edit.redo')).toBeEnabled();
    await command(page, 'edit.redo').click();
    await expect(notes).toHaveValue('Ghi chú của tôi');
  });

  test('F12 opens Save as, Ctrl+Alt+M the comments, the status bar zooms', async ({ page }) => {
    await openDeck(page);
    await page.getByTestId('thumb-1').click();
    await page.keyboard.press('F12');
    await expect(page.getByRole('dialog')).toBeVisible();
    await page.keyboard.press('Escape');
    await expect(page.getByRole('dialog')).toHaveCount(0);

    await tab(page, 'Chèn');
    await expect(command(page, 'review.comments')).toHaveAttribute('aria-pressed', 'false');
    await clickSlide(page, 0.01, 0.02);
    await page.keyboard.press('Control+Alt+m');
    await expect(command(page, 'review.comments')).toHaveAttribute('aria-pressed', 'true');

    const zoom = page.getByTestId('status-zoom');
    const percent = async () => Number((await zoom.innerText()).replace(/\D/g, ''));
    const before = await percent();
    const width = (await page.getByTestId('slide-overlay').boundingBox())!.width;
    await page.getByTestId('status-zoom-in').click();
    await expect.poll(percent).toBeGreaterThan(before);
    await expect
      .poll(async () => (await page.getByTestId('slide-overlay').boundingBox())!.width)
      .toBeGreaterThan(width);
    await page.getByTestId('status-zoom-out').click();
    await expect.poll(percent).toBe(before);
  });

  test('transitions and animations apply to the current slide / selected object', async ({
    page,
  }) => {
    await openDeck(page);
    await tab(page, 'Chuyển trang');
    await expect(command(page, 'transition.none')).toHaveAttribute('aria-pressed', 'true');
    await command(page, 'transition.fade').click();
    await expect(command(page, 'transition.fade')).toHaveAttribute('aria-pressed', 'true');
    // Only the current slide got it…
    await page.getByTestId('thumb-2').click();
    await expect(command(page, 'transition.none')).toHaveAttribute('aria-pressed', 'true');
    // …until "apply to all".
    await page.getByTestId('thumb-1').click();
    await command(page, 'transition.applyAll').click();
    await page.getByTestId('thumb-3').click();
    await expect(command(page, 'transition.fade')).toHaveAttribute('aria-pressed', 'true');

    // An animation needs a selected object.
    await tab(page, 'Hiệu ứng');
    await expect(command(page, 'anim.in.fade')).toBeDisabled();
    await clickSlide(page, 0.5, 0.12);
    await expect(page.getByTestId('selection-box')).toHaveCount(1);
    await expect(command(page, 'anim.in.fade')).toBeEnabled();
  });
});

// ---------------------------------------------------------------- pass 2

/** Text (or description) of each shape of the shown slide, as the screen-reader mirror has it. */
async function shapeTexts(page: Page) {
  return page
    .locator('[data-testid="slide-a11y-mirror"] [data-shape]')
    .evaluateAll((els) => els.map((e) => e.textContent || (e.getAttribute('aria-label') ?? '')));
}

/** Overlay point of slide point (x, y). */
async function slidePoint(page: Page, x: number, y: number) {
  const o = (await page.getByTestId('slide-overlay').boundingBox())!;
  const s = o.width / 960;
  return { x: o.x + x * s, y: o.y + y * s };
}

test.describe('Pass 2: inserting objects', () => {
  test('a shape: click = default size at the point, drag = dragged size, Shift = square', async ({
    page,
  }) => {
    await openDeck(page);
    await page.getByTestId('thumb-3').click();
    await tab(page, 'Chèn');
    const rect = page.getByRole('button', { name: 'Hình chữ nhật' });
    // The button arms the pointer (pressed); nothing is inserted yet.
    await rect.click();
    await expect(rect).toHaveAttribute('aria-pressed', 'true');
    await expect(page.getByTestId('slide-overlay')).toHaveAttribute('data-drawing', 'shape');
    // A click: 1 inch × 1 inch with its top-left corner at the point (on the 1/12 inch grid).
    let p = await slidePoint(page, 60, 300);
    await page.mouse.click(p.x, p.y);
    await expect.poll(() => boxes(page)).toEqual([[60, 300, 72, 72]]);
    await expect(page.getByTestId('slide-overlay')).not.toHaveAttribute('data-drawing', /./);
    // A drag: the dragged box, the rubber band shown while dragging.
    await rect.click();
    p = await slidePoint(page, 300, 300);
    const q = await slidePoint(page, 480, 396);
    await page.mouse.move(p.x, p.y);
    await page.mouse.down();
    await page.mouse.move((p.x + q.x) / 2, (p.y + q.y) / 2, { steps: 3 });
    await expect(page.getByTestId('draw-box')).toBeVisible();
    await page.mouse.move(q.x, q.y, { steps: 3 });
    await page.mouse.up();
    await expect.poll(() => boxes(page)).toEqual([[300, 300, 180, 96]]);
    // Shift while dragging: a square of the larger side.
    await page.getByRole('button', { name: 'Hình elip' }).click();
    p = await slidePoint(page, 600, 300);
    const r = await slidePoint(page, 720, 360);
    await page.mouse.move(p.x, p.y);
    await page.mouse.down();
    await page.keyboard.down('Shift');
    await page.mouse.move(r.x, r.y, { steps: 4 });
    await page.mouse.up();
    await page.keyboard.up('Shift');
    await expect.poll(() => boxes(page)).toEqual([[600, 300, 120, 120]]);
    // Esc leaves the drawing pointer without inserting; Enter inserts in the middle.
    await rect.click();
    await page.keyboard.press('Escape');
    await expect(page.getByTestId('slide-overlay')).not.toHaveAttribute('data-drawing', /./);
    await rect.click();
    await page.keyboard.press('Enter');
    await expect.poll(async () => (await boxes(page))[0]?.slice(2)).toEqual([240, 135]);
    // Each insertion is one undo step.
    await page.keyboard.press('Control+z');
    await page.keyboard.press('Control+a');
    await expect(page.getByTestId('selection-box')).toHaveCount(5);
  });

  test('a text box drawn by dragging takes the dragged width, typing goes in', async ({ page }) => {
    await openDeck(page);
    await page.getByTestId('thumb-3').click();
    await tab(page, 'Chèn');
    await page.getByRole('button', { name: 'Hộp văn bản' }).click();
    const p = await slidePoint(page, 120, 420);
    const q = await slidePoint(page, 420, 432);
    await page.mouse.move(p.x, p.y);
    await page.mouse.down();
    await page.mouse.move(q.x, q.y, { steps: 4 });
    await page.mouse.up();
    await expect.poll(async () => (await boxes(page))[0]?.slice(0, 3)).toEqual([120, 420, 300]);
    await page.keyboard.type('Ghi chú');
    await expect(editor(page)).toHaveValue('Ghi chú');
  });

  test('Insert › Table: the size grid shows columns × rows, a click inserts it', async ({
    page,
  }) => {
    await openDeck(page);
    await page.getByTestId('thumb-3').click();
    await tab(page, 'Chèn');
    await command(page, 'insert.table').click();
    const picker = page.getByTestId('table-size-picker');
    await expect(picker).toBeVisible();
    await picker.locator('[data-cell="4x3"]').hover();
    await expect(page.getByTestId('table-size-label')).toHaveText('Bảng 4 × 3');
    await expect(picker.locator('[data-on]')).toHaveCount(12);
    await picker.locator('[data-cell="4x3"]').click();
    await expect(picker).toHaveCount(0);
    await expect.poll(() => boxes(page)).toHaveLength(1);
    // Its cells: 4 columns × 3 rows (double-click edits a cell; Tab goes to the next cell).
    const b = (await page.getByTestId('selection-box').first().boundingBox())!;
    await page.mouse.dblclick(b.x + 10, b.y + 8);
    await expect(page.locator('[data-testid^="table-cell"]')).toHaveCount(12);
    await expect(editor(page)).toBeFocused();
    await page.keyboard.type('A');
    await expect(editor(page)).toHaveValue('A');
    await page.keyboard.press('Tab');
    await expect(editor(page)).toHaveValue('');
    await page.keyboard.type('B');
    await expect(editor(page)).toHaveValue('B');
    // Shift+Tab: back to the first cell, its text selected.
    await page.keyboard.press('Shift+Tab');
    await expect(editor(page)).toHaveValue('A');
    await page.keyboard.press('Escape');
    // The keyboard: arrows move in the grid, Esc closes it.
    await command(page, 'insert.table').click();
    await page.keyboard.press('ArrowRight');
    await page.keyboard.press('ArrowDown');
    await expect(page.getByTestId('table-size-label')).toHaveText('Bảng 2 × 2');
    await page.keyboard.press('Escape');
    await expect(picker).toHaveCount(0);
  });
});

test.describe('Pass 2: slide panel selection', () => {
  test('Shift / Ctrl+click select several slides; delete, hide, duplicate act on all', async ({
    page,
  }) => {
    await openDeck(page);
    const [a, b, c] = await titles(page, 3);
    const sel = () =>
      page
        .locator('[data-testid^="thumb-"][data-selected="true"]')
        .evaluateAll((els) => els.map((e) => e.getAttribute('data-testid')));
    await page.getByTestId('thumb-1').click();
    await page.getByTestId('thumb-3').click({ modifiers: ['Shift'] });
    await expect.poll(sel).toEqual(['thumb-1', 'thumb-2', 'thumb-3']);
    await page.getByTestId('thumb-2').click({ modifiers: ['Control'] });
    await expect.poll(sel).toEqual(['thumb-1', 'thumb-3']);
    // Hide slide: both selected slides.
    await command(page, 'slide.hide').click();
    await expect(page.getByTestId('thumb-1')).toHaveAttribute('data-hidden', 'true');
    await expect(page.getByTestId('thumb-3')).toHaveAttribute('data-hidden', 'true');
    await expect(page.getByTestId('thumb-2')).not.toHaveAttribute('data-hidden', 'true');
    await page.keyboard.press('Control+z');
    // Ctrl+D: the copies follow the last selected slide, in order.
    await page.getByTestId('thumb-1').click();
    await page.getByTestId('thumb-3').click({ modifiers: ['Control'] });
    await page.keyboard.press('Control+d');
    await expect(thumbs(page)).toHaveCount(5);
    expect(await titles(page, 5)).toEqual([a, b, c, a, c]);
    // Delete: all the selected slides, one undo step brings them back.
    await page.getByTestId('thumb-4').click();
    await page.getByTestId('thumb-5').click({ modifiers: ['Shift'] });
    await page.keyboard.press('Delete');
    await expect(thumbs(page)).toHaveCount(3);
    await page.keyboard.press('Control+z');
    await expect(thumbs(page)).toHaveCount(5);
    // Ctrl+A in the slide panel: every slide.
    await page.getByTestId('thumb-1').click();
    await page.keyboard.press('Control+a');
    await expect.poll(async () => (await sel()).length).toBe(5);
    // A plain click: one slide again.
    await page.getByTestId('thumb-2').click();
    await expect.poll(sel).toEqual([]);
  });
});

test.describe('Pass 2: formatting', () => {
  test('format painter: one use on a click, kept on a double-click until Esc', async ({ page }) => {
    await openDeck(page);
    await selectTitle(page);
    // The title made italic (it is not italic in the file).
    await page.keyboard.press('Control+i');
    await expect(command(page, 'format.italic')).toHaveAttribute('aria-pressed', 'true');
    await tab(page, 'Trang đầu');
    await command(page, 'format.painter').click();
    await expect(page.getByTestId('slide-overlay')).toHaveAttribute('data-painting', 'true');
    // A click on the subtitle paints it; the painter is put away.
    await clickSlide(page, 0.5, 0.55);
    await expect(page.getByTestId('slide-overlay')).not.toHaveAttribute('data-painting', /./);
    await expect(command(page, 'format.italic')).toHaveAttribute('aria-pressed', 'true');
    // One undo step takes the painted formatting away.
    await page.keyboard.press('Control+z');
    await clickSlide(page, 0.5, 0.55);
    await expect(command(page, 'format.italic')).toHaveAttribute('aria-pressed', 'false');
    // Double-click: kept for several shapes, Esc puts it away.
    await selectTitle(page);
    await command(page, 'format.painter').dblclick();
    await expect(command(page, 'format.painter')).toHaveAttribute('aria-pressed', 'true');
    await clickSlide(page, 0.5, 0.55);
    await expect(command(page, 'format.italic')).toHaveAttribute('aria-pressed', 'true');
    await expect(page.getByTestId('slide-overlay')).toHaveAttribute('data-painting', 'true');
    await page.keyboard.press('Escape');
    await expect(page.getByTestId('slide-overlay')).not.toHaveAttribute('data-painting', /./);
  });

  test('Ctrl+Shift+= superscript, Ctrl+= subscript, toggles; Ctrl+T the font dialog', async ({
    page,
  }) => {
    await openDeck(page);
    await selectTitle(page);
    await page.keyboard.press('Enter');
    await expect(editor(page)).toBeFocused();
    await page.keyboard.press('Control+Shift+Equal');
    await expect(command(page, 'format.superscript')).toHaveAttribute('aria-pressed', 'true');
    await page.keyboard.press('Control+Equal');
    await expect(command(page, 'format.subscript')).toHaveAttribute('aria-pressed', 'true');
    await expect(command(page, 'format.superscript')).toHaveAttribute('aria-pressed', 'false');
    await page.keyboard.press('Control+Equal');
    await expect(command(page, 'format.subscript')).toHaveAttribute('aria-pressed', 'false');
    // The font dialog: small capitals and 2 pt spacing, applied as one undo step.
    await page.keyboard.press('Control+t');
    const dialog = page.getByTestId('font-dialog');
    await expect(dialog).toBeVisible();
    // It opens with the selection's formatting (the size shown in the ribbon's size box).
    const size = await page.getByTestId('slide-font-size').inputValue();
    await expect(page.getByTestId('font-dialog-size')).toHaveValue(size);
    await page.getByTestId('font-dialog-small-caps').check();
    await page.getByTestId('font-dialog-spacing').fill('2');
    await page.getByTestId('font-dialog-ok').click();
    await expect(dialog).toHaveCount(0);
    await page.keyboard.press('Control+Shift+f');
    await expect(page.getByTestId('font-dialog-small-caps')).toBeChecked();
    await expect(page.getByTestId('font-dialog-spacing')).toHaveValue('2');
    await page.getByRole('button', { name: 'Huỷ' }).click();
    await expect(dialog).toHaveCount(0);
    await page.keyboard.press('Control+z');
    await page.keyboard.press('Control+t');
    await expect(page.getByTestId('font-dialog-small-caps')).not.toBeChecked();
    await expect(page.getByTestId('font-dialog-spacing')).toHaveValue('0');
  });

  test('Shift+F3 cycles the case of the selected text, Ctrl+Space clears its formatting', async ({
    page,
  }) => {
    await openDeck(page);
    await selectTitle(page);
    await page.keyboard.press('Enter');
    await expect(editor(page)).toBeFocused();
    // "Báo cáo kết quả kinh doanh": mixed case → UPPER → Each Word → lower.
    await page.keyboard.press('Shift+F3');
    await expect(editor(page)).toHaveValue('BÁO CÁO KẾT QUẢ KINH DOANH');
    await page.keyboard.press('Shift+F3');
    await expect(editor(page)).toHaveValue('Báo Cáo Kết Quả Kinh Doanh');
    await page.keyboard.press('Shift+F3');
    await expect(editor(page)).toHaveValue('báo cáo kết quả kinh doanh');
    // The selection stays on the text.
    expect(await editState(page)).toMatchObject({ start: 0, end: 26 });
    // Ctrl+Space: italic and superscript go, the placeholder's formatting comes back.
    await page.keyboard.press('Control+i');
    await page.keyboard.press('Control+Shift+Equal');
    await expect(command(page, 'format.italic')).toHaveAttribute('aria-pressed', 'true');
    await page.keyboard.press('Control+Space');
    await expect(command(page, 'format.italic')).toHaveAttribute('aria-pressed', 'false');
    await expect(command(page, 'format.superscript')).toHaveAttribute('aria-pressed', 'false');
  });

  test('Ctrl+Alt+V: paste special as shapes, picture or text only', async ({ page }) => {
    await openDeck(page);
    await selectTitle(page);
    await page.keyboard.press('Control+c');
    await page.keyboard.press('Control+Alt+v');
    const dialog = page.getByTestId('paste-special-dialog');
    await expect(dialog).toBeVisible();
    await expect(page.getByTestId('paste-special-theme')).toBeChecked();
    await expect(page.getByTestId('paste-special-keep')).toBeVisible();
    await expect(page.getByTestId('paste-special-text')).toBeVisible();
    await page.getByTestId('paste-special-text').check();
    await page.getByTestId('paste-special-ok').click();
    await expect(dialog).toHaveCount(0);
    // Text only: a new text box with the title's text.
    await expect.poll(async () => (await shapeNames(page)).length).toBe(3);
    await expect
      .poll(async () => (await shapeTexts(page)).filter((n) => n.includes('Báo cáo')).length)
      .toBe(2);
  });
});

test.describe('Pass 2: view aids, header and footer, the show', () => {
  test('Shift+F9 gridlines, Alt+F9 guides (draggable), View › Ruler', async ({ page }) => {
    await openDeck(page);
    await clickSlide(page, 0.02, 0.98);
    await page.keyboard.press('Shift+F9');
    await expect(page.getByTestId('slide-gridlines')).toBeVisible();
    await page.keyboard.press('Shift+F9');
    await expect(page.getByTestId('slide-gridlines')).toHaveCount(0);
    await page.keyboard.press('Alt+F9');
    const guides = page.getByTestId('static-guide');
    await expect(guides).toHaveCount(2);
    // The vertical guide through the centre, dragged 2 inches to the right.
    const v = await slidePoint(page, 480, 30);
    const to = await slidePoint(page, 624, 30);
    await page.mouse.move(v.x, v.y);
    await page.mouse.down();
    await page.mouse.move(to.x, to.y, { steps: 5 });
    await page.mouse.up();
    await expect
      .poll(async () =>
        Math.round(
          Number(
            await page.locator('[data-testid="static-guide"][data-axis="x"]').getAttribute('x1'),
          ),
        ),
      )
      .toBe(624);
    await page.keyboard.press('Alt+F9');
    await expect(guides).toHaveCount(0);
    await tab(page, 'Xem');
    await command(page, 'view.ruler').click();
    await expect(page.getByTestId('ruler-h')).toBeVisible();
    await expect(page.getByTestId('ruler-v')).toBeVisible();
    await command(page, 'view.ruler').click();
    await expect(page.getByTestId('ruler-h')).toHaveCount(0);
  });

  test('Insert › Header & Footer: slide number and footer on all but the title slide', async ({
    page,
  }) => {
    await openDeck(page);
    await tab(page, 'Chèn');
    await command(page, 'insert.headerFooter').click();
    const dialog = page.getByTestId('header-footer-dialog');
    await expect(dialog).toBeVisible();
    await page.getByTestId('hf-number').check();
    await page.getByTestId('hf-footer').check();
    await page.getByTestId('hf-footer-text').fill('Bluestar');
    await page.getByTestId('hf-not-title').check();
    await page.getByTestId('hf-apply-all').click();
    await expect(dialog).toHaveCount(0);
    // The title slide has none; the others show the footer text and their number.
    await expect.poll(async () => (await shapeNames(page)).length).toBe(2);
    await page.getByTestId('thumb-2').click();
    await expect.poll(async () => (await shapeNames(page)).length).toBe(4);
    await expect.poll(async () => (await shapeTexts(page)).join('|')).toContain('Bluestar');
    await expect.poll(async () => (await shapeTexts(page)).join('|')).toContain('2');
    // The dialog opens with what the slide has.
    await command(page, 'insert.headerFooter').click();
    await expect(page.getByTestId('hf-number')).toBeChecked();
    await expect(page.getByTestId('hf-footer-text')).toHaveValue('Bluestar');
    await expect(page.getByTestId('hf-not-title')).toBeChecked();
    await page.getByRole('button', { name: 'Huỷ' }).click();
    // One undo step for all the slides.
    await page.keyboard.press('Control+z');
    await expect.poll(async () => (await shapeNames(page)).length).toBe(2);
  });

  test('in the show: H shows the hidden slide that follows, Ctrl+H hides the pointer', async ({
    page,
  }) => {
    await openDeck(page);
    await page.getByTestId('thumb-2').click();
    await command(page, 'slide.hide').click();
    await expect(page.getByTestId('thumb-2')).toHaveAttribute('data-hidden', 'true');
    await page.keyboard.press('F5');
    const stage = page.getByTestId('show-stage');
    await expect(stage).toHaveAttribute('data-slide', '0');
    await page.keyboard.press('h');
    await expect(stage).toHaveAttribute('data-slide', '1');
    // H does nothing when no hidden slide follows; the show goes on after the hidden one.
    await page.keyboard.press('h');
    await expect(stage).toHaveAttribute('data-slide', '1');
    await page.keyboard.press('ArrowRight');
    await expect(stage).toHaveAttribute('data-slide', '2');
    await page.keyboard.press('Control+h');
    await expect(page.locator('[data-pointer-hidden="true"]')).toHaveCount(1);
    await page.keyboard.press('Control+a');
    await expect(page.locator('[data-pointer-hidden="true"]')).toHaveCount(0);
    await page.keyboard.press('Escape');
  });
});
