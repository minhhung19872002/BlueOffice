import { expect, test, type Page } from '@playwright/test';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

// Keyboard-only walkthrough (WCAG 2.1.1 keyboard, 2.4.3 focus order, 2.4.7 focus visible,
// 2.1.2 no keyboard trap): in each app, from the document, reach the ribbon with Alt (KeyTips)
// and with F6, run a command from the keyboard, come back into the document, open a dialog
// from the keyboard and close it with Escape — focus must return where it was.

const deck = join(
  dirname(fileURLToPath(import.meta.url)),
  '..',
  '..',
  'corpus',
  'slide',
  'slide-text-bullets.pptx',
);

/** Identity of the focused element (tag, test id, label, role). */
async function focused(page: Page) {
  return page.evaluate(() => {
    const el = document.activeElement as HTMLElement | null;
    if (!el) return 'none';
    return [
      el.tagName,
      el.dataset.testid ?? '',
      el.getAttribute('aria-label') ?? '',
      el.getAttribute('role') ?? '',
    ].join('|');
  });
}

/** The focused element draws a visible focus indicator (outline or box-shadow). */
async function focusIsVisible(page: Page) {
  return page.evaluate(() => {
    const el = document.activeElement as HTMLElement | null;
    if (!el) return false;
    const s = getComputedStyle(el);
    const outline = s.outlineStyle !== 'none' && parseFloat(s.outlineWidth) >= 1;
    return outline || (s.boxShadow !== 'none' && s.boxShadow !== '');
  });
}

/** Alt → KeyTips on the tabs; the tab letter, then the command's KeyTip runs it. */
async function keyTip(page: Page, tab: string, command: string) {
  await page.keyboard.press('Alt');
  const tips = page.getByTestId('keytips');
  await expect(tips).toHaveAttribute('data-level', 'tabs');
  const tabKey = await tips.locator(`[data-keytip-for="tab:${tab}"]`).getAttribute('data-keytip');
  expect(tabKey, `KeyTip of tab ${tab}`).toBeTruthy();
  for (const ch of tabKey!.toLowerCase()) await page.keyboard.press(ch);
  await expect(tips).toHaveAttribute('data-level', 'items');
  const key = await tips.locator(`[data-keytip-for="${command}"]`).getAttribute('data-keytip');
  expect(key, `KeyTip of ${command}`).toBeTruthy();
  for (const ch of key!.toLowerCase()) await page.keyboard.press(ch);
  await expect(tips).toHaveCount(0);
}

/** F6 from the document until the ribbon's selected tab has focus (at most 8 presses). */
async function f6ToRibbon(page: Page) {
  for (let i = 0; i < 8; i++) {
    await page.keyboard.press('F6');
    const onTab = await page.evaluate(() => {
      const el = document.activeElement;
      return !!el?.closest('[data-f6-region=ribbon]') && el?.getAttribute('role') === 'tab';
    });
    if (onTab) return;
  }
  throw new Error('F6 never reached the ribbon');
}

/** F6 onwards until the keyboard is back in the app (`isHome` says when). */
async function f6Home(page: Page, isHome: () => Promise<boolean>) {
  for (let i = 0; i < 8; i++) {
    await page.keyboard.press('F6');
    if (await isHome()) return;
  }
  throw new Error('F6 never returned to the document');
}

test('BlueDoc keyboard walkthrough', async ({ page }) => {
  await page.goto('/doc');
  // The start screen is operable from the keyboard: Tab reaches "new document", Enter runs it.
  const start = page.getByTestId('new-document');
  await expect(start).toBeVisible();
  await start.focus();
  await page.keyboard.press('Enter');
  const input = page.getByTestId('ime-input');
  await expect(input).toBeFocused();
  await page.keyboard.type('Bàn phím');
  await page.keyboard.press('Shift+Home');

  // Alt: KeyTips, Home tab, Bold — the command runs on the selection, keyboard back in the text.
  await keyTip(page, 'home', 'format.bold');
  await expect(page.locator('[data-command="format.bold"]').first()).toHaveAttribute(
    'aria-pressed',
    'true',
  );
  await expect(input).toBeFocused();

  // F6 reaches the ribbon; arrows move between tabs, focus is visible; F6 goes on to the
  // status bar and back into the document.
  await f6ToRibbon(page);
  expect(await focusIsVisible(page)).toBe(true);
  await page.keyboard.press('ArrowRight');
  expect(await page.evaluate(() => document.activeElement?.getAttribute('role'))).toBe('tab');
  await f6Home(page, async () => input.evaluate((el) => el === document.activeElement));
  await page.keyboard.press('End');
  await page.keyboard.type(' tiếp');

  // A dialog from the keyboard (Find & Replace, Ctrl+H); Escape closes it, focus comes back.
  const before = await focused(page);
  await page.keyboard.press('Control+h');
  const dialog = page.getByRole('dialog');
  await expect(dialog).toBeVisible();
  // Focus moved into the dialog (no focus left behind it).
  expect(await page.evaluate(() => !!document.activeElement?.closest('[role=dialog]'))).toBe(true);
  await page.keyboard.press('Escape');
  await expect(dialog).toHaveCount(0);
  expect(await focused(page)).toBe(before);

  // The Font dialog through its KeyTip, closed with Escape, focus back in the document.
  await keyTip(page, 'home', 'format.fontDialog');
  await expect(page.getByRole('dialog')).toBeVisible();
  await page.keyboard.press('Escape');
  await expect(page.getByRole('dialog')).toHaveCount(0);
  await expect(input).toBeFocused();
});

test('BlueSheet keyboard walkthrough', async ({ page }) => {
  await page.goto('/sheet');
  await expect(page.getByTestId('sheet-canvas')).toHaveAttribute('data-rendered', /\d+/);
  const cell = page.getByLabel('Ô đang nhập');
  await cell.focus();
  await page.keyboard.type('Bàn phím');
  await page.keyboard.press('Enter');
  await page.keyboard.press('ArrowUp');

  await keyTip(page, 'home', 'format.bold');
  await expect(page.locator('[data-command="format.bold"]').first()).toHaveAttribute(
    'aria-pressed',
    'true',
  );
  await expect(cell).toBeFocused();

  // WCAG 2.5.7: sheet tabs reorder without dragging — Move sheet left.
  // (Commands of the collapsed Sheets group are run from command search: Alt+Q, name, Enter.)
  for (const name of ['Thêm trang tính', 'Chuyển trang tính sang trái']) {
    await page.keyboard.press('Alt+q');
    await page.getByTestId('command-search-input').fill(name);
    await page.keyboard.press('Enter');
    await expect(page.getByTestId('command-search')).toHaveCount(0);
    if (name === 'Thêm trang tính') {
      await expect(page.getByRole('tab', { name: 'Sheet2' })).toHaveAttribute(
        'aria-selected',
        'true',
      );
    }
  }
  await expect(page.getByTestId('sheet-tab-0')).toHaveText('Sheet2');
  await expect(page.getByTestId('sheet-tab-0')).toHaveAttribute('aria-selected', 'true');
  await cell.focus();

  await f6ToRibbon(page);
  expect(await focusIsVisible(page)).toBe(true);
  await f6Home(page, async () => cell.evaluate((el) => el === document.activeElement));

  const before = await focused(page);
  await page.keyboard.press('Control+h');
  const dialog = page.getByRole('dialog');
  await expect(dialog).toBeVisible();
  expect(await page.evaluate(() => !!document.activeElement?.closest('[role=dialog]'))).toBe(true);
  await page.keyboard.press('Escape');
  await expect(dialog).toHaveCount(0);
  expect(await focused(page)).toBe(before);
});

test('BlueSlide keyboard walkthrough', async ({ page }) => {
  await page.goto('/slide');
  const chooser = page.waitForEvent('filechooser');
  await page.getByRole('button', { name: 'Mở', exact: true }).click();
  await (await chooser).setFiles(deck);
  await expect(page.getByTestId('slide-overlay')).toBeVisible();
  const overlay = page.getByTestId('slide-overlay');
  await overlay.focus();

  // A command from the keyboard: Home › New slide adds a slide.
  const status = page.getByTestId('slide-status');
  const count = Number((await status.innerText()).split('/')[1]);
  await keyTip(page, 'home', 'slide.new');
  await expect(status).toHaveText(new RegExp(`/ ${count + 1}$`));
  // WCAG 2.5.7: slides reorder without dragging — Move slide up / down.
  const at = Number((await status.innerText()).match(/Slide (\d+)/)![1]);
  await keyTip(page, 'home', 'slide.moveUp');
  await expect(status).toHaveText(new RegExp(`^Slide ${at - 1} /`));
  await keyTip(page, 'home', 'slide.moveDown');
  await expect(status).toHaveText(new RegExp(`^Slide ${at} /`));

  await overlay.focus();
  await f6ToRibbon(page);
  expect(await focusIsVisible(page)).toBe(true);
  // Back in the app: focus is outside the ribbon, the status bar and the task panes.
  await f6Home(page, () =>
    page.evaluate(
      () =>
        !!document.activeElement &&
        document.activeElement !== document.body &&
        !document.activeElement.closest('[data-f6-region], [role=complementary]'),
    ),
  );

  // Design › Slide size opens its dialog; Escape closes it and focus returns.
  const before = await focused(page);
  await keyTip(page, 'design', 'design.slideSize');
  const dialog = page.getByRole('dialog');
  await expect(dialog).toBeVisible();
  expect(await page.evaluate(() => !!document.activeElement?.closest('[role=dialog]'))).toBe(true);
  await page.keyboard.press('Escape');
  await expect(dialog).toHaveCount(0);
  expect(await focused(page)).toBe(before);
});
