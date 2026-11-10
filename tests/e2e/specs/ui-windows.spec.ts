import { expect, test, type Page } from '@playwright/test';

// CORE-UI-007: customizable status bar, high-contrast theme, split view, and several
// windows (New Window, View Side by Side with synchronous scrolling).

const cmd = (page: Page, id: string) => page.locator(`[data-command="${id}"]`);
const viewTab = (page: Page) => page.getByRole('tab', { name: 'Xem', exact: true });
const nav = (page: Page, name: string) =>
  page.getByRole('navigation', { name: 'Tệp' }).getByRole('button', { name, exact: true });

async function openSample(page: Page) {
  await page.goto('/doc');
  await page.getByTestId('open-sample').click();
  await expect(page.getByTestId('status-page')).toHaveText(/^Trang 1\/\d+$/);
  await expect(page.locator('.bo-page[data-page="1"]').first()).toHaveAttribute(
    'data-rendered',
    'true',
  );
}

async function openStatusMenu(page: Page) {
  await page.getByTestId('status-bar').click({ button: 'right', position: { x: 500, y: 12 } });
  const menu = page.getByTestId('status-bar-menu');
  await expect(menu).toBeVisible();
  return menu;
}

test('BlueDoc: the status bar checklist hides and shows items and is remembered', async ({
  page,
}) => {
  const errors: string[] = [];
  page.on('pageerror', (e) => errors.push(e.message));
  await openSample(page);
  await expect(page.getByTestId('status-words')).toBeVisible();
  await expect(page.getByTestId('status-overtype')).toHaveCount(0);

  const menu = await openStatusMenu(page);
  await expect(menu).toHaveAttribute('role', 'menu');
  const words = page.getByTestId('status-item-words');
  const overtype = page.getByTestId('status-item-overtype');
  await expect(words).toHaveAttribute('aria-checked', 'true');
  await expect(overtype).toHaveAttribute('aria-checked', 'false');
  // The checklist stays open while items are toggled.
  await words.click();
  await expect(words).toHaveAttribute('aria-checked', 'false');
  await expect(page.getByTestId('status-words')).toHaveCount(0);
  await overtype.click();
  await expect(overtype).toHaveAttribute('aria-checked', 'true');
  await expect(page.getByTestId('status-overtype')).toBeVisible();
  // Keyboard: End goes to "reset", Home back to the first item; Escape closes.
  await page.keyboard.press('Home');
  await expect(page.getByTestId('status-item-page')).toHaveAttribute('data-active', 'true');
  await page.keyboard.press('Escape');
  await expect(menu).toHaveCount(0);

  // Remembered across reloads.
  await openSample(page);
  await expect(page.getByTestId('status-overtype')).toBeVisible();
  await expect(page.getByTestId('status-words')).toHaveCount(0);

  // Reset brings the defaults back.
  await openStatusMenu(page);
  await page.getByTestId('status-item-reset').click();
  await expect(page.getByTestId('status-words')).toBeVisible();
  await expect(page.getByTestId('status-overtype')).toHaveCount(0);
  await page.keyboard.press('Escape');
  expect(errors).toEqual([]);
});

test('BlueSheet: status bar statistics follow the checklist', async ({ page }) => {
  await page.goto('/sheet');
  await expect(page.getByTestId('sheet-canvas')).toHaveAttribute('data-rendered', /\d+/);
  const box = page.getByLabel('Hộp tên');
  await box.click();
  await box.fill('A1');
  await box.press('Enter');
  for (const v of ['4', '10', '1']) {
    await page.keyboard.type(v);
    await page.keyboard.press('Enter');
  }
  await box.click();
  await box.fill('A1:A3');
  await box.press('Enter');
  await expect(page.getByTestId('status-sum')).toContainText('15');
  await expect(page.getByTestId('status-max')).toHaveCount(0);

  await openStatusMenu(page);
  await page.getByTestId('status-item-max').click();
  await page.getByTestId('status-item-sum').click();
  await page.keyboard.press('Escape');
  await expect(page.getByTestId('status-max')).toContainText('10');
  await expect(page.getByTestId('status-sum')).toHaveCount(0);
});

test('high contrast: picked in File › Options, shared by the apps, follows forced colors', async ({
  page,
}) => {
  await openSample(page);
  const root = page.locator('[data-theme]').first();
  await expect(root).toHaveAttribute('data-theme', 'light');
  const paperBorder = () =>
    root.evaluate((el) => getComputedStyle(el).getPropertyValue('--bo-chrome-cursor').trim());
  const light = await paperBorder();

  await page.getByTestId('file-tab').click();
  await nav(page, 'Tùy chọn').click();
  await page.getByTestId('backstage-theme').selectOption('hc');
  await expect(root).toHaveAttribute('data-theme', 'hc');
  await page.keyboard.press('Escape');
  // The chrome tokens of the canvas change with the theme and text is at maximum contrast.
  expect(await paperBorder()).not.toBe(light);
  const contrast = await root.evaluate((el) => {
    const lum = (css: string) => {
      const [r, g, b] = (css.match(/[\d.]+/g) ?? []).slice(0, 3).map((n) => {
        const c = Number(n) / 255;
        return c <= 0.03928 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4;
      });
      return 0.2126 * (r ?? 0) + 0.7152 * (g ?? 0) + 0.0722 * (b ?? 0);
    };
    const bar = el.querySelector('[data-testid="status-bar"]') as HTMLElement;
    const cs = getComputedStyle(bar);
    const a = lum(cs.color);
    const b = lum(cs.backgroundColor);
    return (Math.max(a, b) + 0.05) / (Math.min(a, b) + 0.05);
  });
  // WCAG AAA for normal text.
  expect(contrast).toBeGreaterThanOrEqual(7);

  // Remembered, and the other apps use the same choice.
  await page.goto('/sheet');
  await expect(page.locator('[data-theme]').first()).toHaveAttribute('data-theme', 'hc');
  await page.goto('/slide');
  await expect(page.locator('[data-theme]').first()).toHaveAttribute('data-theme', 'hc');

  // "Follow the system": high contrast when the system forces colours.
  await page.evaluate(() => localStorage.removeItem('bo.theme'));
  await page.goto('/doc');
  await expect(page.locator('[data-theme]').first()).toHaveAttribute('data-theme', 'light');
  await page.emulateMedia({ forcedColors: 'active' });
  await expect(page.locator('[data-theme]').first()).toHaveAttribute('data-theme', 'hc');
});

test('BlueDoc: split view shows the document in two panes that scroll apart', async ({ page }) => {
  const errors: string[] = [];
  page.on('pageerror', (e) => errors.push(e.message));
  await openSample(page);
  await viewTab(page).click();
  await cmd(page, 'window.split').click();
  await expect(cmd(page, 'window.split')).toHaveAttribute('aria-pressed', 'true');
  const bar = page.getByTestId('split-bar');
  await expect(bar).toBeVisible();
  await expect(bar).toHaveAttribute('aria-valuenow', '50');
  const second = page.getByTestId('split-scroller');
  await expect(second.locator('.bo-page[data-page="1"]')).toHaveAttribute('data-rendered', 'true');

  // The second pane scrolls; the first stays where it is.
  const first = page.getByTestId('document-view').locator('.bo-doc-scroller');
  await second.evaluate((el) => el.scrollTo({ top: 600 }));
  await expect.poll(() => second.evaluate((el) => el.scrollTop)).toBeGreaterThan(300);
  expect(await first.evaluate((el) => el.scrollTop)).toBe(0);

  // The panes are stacked; the bar moves with the arrow keys.
  const before = await bar.boundingBox();
  await bar.focus();
  await page.keyboard.press('ArrowDown');
  await expect(bar).toHaveAttribute('aria-valuenow', '55');
  const after = await bar.boundingBox();
  expect(after!.y).toBeGreaterThan(before!.y);

  // Typing in the first pane shows in the second one too (one document).
  const words = async () => (await page.getByTestId('status-words').textContent()) ?? '';
  const count = await words();
  await first.locator('.bo-page[data-page="1"]').click({ position: { x: 200, y: 200 } });
  await page.keyboard.type('tách ngăn ');
  await expect.poll(words).not.toBe(count);
  await expect(second.locator('.bo-page[data-page="1"]')).toHaveAttribute('data-rendered', 'true');

  // Double-click on the bar removes the split.
  await bar.dblclick();
  await expect(bar).toHaveCount(0);
  await expect(page.getByTestId('document-view-split')).toHaveCount(0);
  await expect(cmd(page, 'window.split')).toHaveAttribute('aria-pressed', 'false');
  expect(errors).toEqual([]);
});

test('BlueSheet: split at the active cell gives a second pane of the same sheet', async ({
  page,
}) => {
  const errors: string[] = [];
  page.on('pageerror', (e) => errors.push(e.message));
  await page.goto('/sheet');
  await expect(page.getByTestId('sheet-canvas')).toHaveAttribute('data-rendered', /\d+/);
  const box = page.getByLabel('Hộp tên');
  await box.click();
  await box.fill('C8');
  await box.press('Enter');
  await viewTab(page).click();
  await cmd(page, 'window.split').click();
  await expect(cmd(page, 'window.split')).toHaveAttribute('aria-pressed', 'true');
  const bar = page.getByTestId('sheet-split-bar');
  await expect(bar).toBeVisible();
  await expect(bar).toHaveAttribute('aria-orientation', 'horizontal');
  await expect(page.getByTestId('sheet-split-canvas')).toHaveAttribute('data-rendered', /\d+/);
  // The active cell is drawn in the second pane as well.
  await expect(page.getByTestId('sheet-split-selection')).toBeVisible();

  // The second pane scrolls on its own.
  const second = page.getByTestId('sheet-split-scroller');
  const mainTop = () =>
    page.getByTestId('sheet-canvas').evaluate((c) => c.closest('[class*="scroller"]')!.scrollTop);
  const top0 = await mainTop();
  await second.evaluate((el) => el.scrollTo({ top: el.scrollTop + 800 }));
  await expect.poll(() => second.evaluate((el) => el.scrollTop)).toBeGreaterThan(700);
  expect(await mainTop()).toBe(top0);

  // A click in the second pane selects that cell in the workbook.
  await page.getByTestId('sheet-split-canvas').click({ position: { x: 150, y: 90 } });
  await expect(box).not.toHaveValue('C8');

  // Split again removes it.
  await cmd(page, 'window.split').click();
  await expect(bar).toHaveCount(0);
  await expect(cmd(page, 'window.split')).toHaveAttribute('aria-pressed', 'false');
  expect(errors).toEqual([]);
});

test('two windows: view side by side scrolls both, synchronous scrolling can be turned off', async ({
  context,
}) => {
  const a = await context.newPage();
  const b = await context.newPage();
  await openSample(a);
  await openSample(b);
  for (const p of [a, b]) await p.setViewportSize({ width: 1366, height: 500 });

  await viewTab(a).click();
  await viewTab(b).click();
  await expect(cmd(a, 'window.syncScroll')).toBeDisabled();
  await cmd(a, 'window.sideBySide').click();
  // Both windows are paired (the only other window is taken without asking).
  await expect(a.getByTestId('status-side-by-side')).toContainText('phase1-sample.docx');
  await expect(b.getByTestId('status-side-by-side')).toContainText('phase1-sample.docx');
  await expect(cmd(b, 'window.sideBySide')).toHaveAttribute('aria-pressed', 'true');
  await expect(cmd(b, 'window.syncScroll')).toHaveAttribute('aria-pressed', 'true');

  const scroller = (p: Page) => p.getByTestId('document-view').locator('.bo-doc-scroller');
  const top = (p: Page) => scroller(p).evaluate((el) => el.scrollTop);
  await scroller(a).evaluate((el) => el.scrollTo({ top: 400 }));
  await expect.poll(() => top(b)).toBeGreaterThan(350);
  // …and back the other way.
  await scroller(b).evaluate((el) => el.scrollTo({ top: 100 }));
  await expect.poll(() => top(a)).toBeLessThan(150);

  // Synchronous scrolling off (in both windows): they scroll apart.
  await cmd(b, 'window.syncScroll').click();
  await expect(cmd(a, 'window.syncScroll')).toHaveAttribute('aria-pressed', 'false');
  await scroller(a).evaluate((el) => el.scrollTo({ top: 500 }));
  await a.waitForTimeout(400);
  expect(await top(b)).toBeLessThan(150);

  // Reset window position: both show the same place again.
  await cmd(a, 'window.resetPosition').click();
  await expect.poll(() => top(b)).toBeGreaterThan(450);

  // Side by side off ends it in both windows.
  await cmd(a, 'window.sideBySide').click();
  await expect(a.getByTestId('status-side-by-side')).toHaveCount(0);
  await expect(b.getByTestId('status-side-by-side')).toHaveCount(0);
});

test('New Window opens the same document in a second window; edits appear in both', async ({
  context,
}) => {
  const a = await context.newPage();
  await openSample(a);
  await viewTab(a).click();
  const opened = context.waitForEvent('page');
  await cmd(a, 'window.new').click();
  const b = await opened;
  await expect(b).toHaveURL(/[?&]window=/);
  await expect(a).toHaveURL(/[?&]window=/);
  for (const p of [a, b]) {
    await expect(p.getByTestId('status-shared-window')).toBeVisible();
    await expect(p.getByTestId('doc-name')).toHaveText('phase1-sample.docx');
    await expect(p.getByTestId('status-page')).toHaveText(/^Trang 1\/\d+$/);
  }

  const words = async (p: Page) =>
    Number(((await p.getByTestId('status-words').textContent()) ?? '').replace(/\D/g, ''));
  await expect.poll(() => words(b)).toBeGreaterThan(0);
  const n = await words(b);
  await a.bringToFront();
  await a
    .locator('.bo-page[data-page="1"]')
    .first()
    .click({ position: { x: 200, y: 200 } });
  await a.keyboard.type(' một hai ba ');
  await expect.poll(() => words(a)).toBe(n + 3);
  await expect.poll(() => words(b)).toBe(n + 3);
});
