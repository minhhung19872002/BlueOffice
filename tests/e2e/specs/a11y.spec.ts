import AxeBuilder from '@axe-core/playwright';
import { expect, test, type Page } from '@playwright/test';
import { appendFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const deck = join(
  dirname(fileURLToPath(import.meta.url)),
  '..',
  '..',
  'corpus',
  'slide',
  'slide-text-bullets.pptx',
);

// WCAG 2.2 AA audit of BlueOffice's own UI (BlueOffice-PARITY-PROMPT §9.3) with axe-core:
// the start screen, then BlueDoc, BlueSheet and BlueSlide with a document open — every
// ribbon tab, the status bar, the File screen (Info, Save As, Options), common dialogs and
// the main task panes — in the light, dark and high-contrast themes. Any violation fails the
// test; the only exclusions are listed in EXCLUDE with the reason.
//
// Method notes (docs/ui/accessibility-audit.md):
// * Tags: WCAG 2.0 / 2.1 / 2.2 level A and AA rules of axe-core.
// * The themes follow the system: `prefers-color-scheme: dark` selects the dark theme and
//   `prefers-contrast: more` the high-contrast one (packages/ui/src/theme.ts), so both are
//   emulated rather than toggled.

const TAGS = ['wcag2a', 'wcag2aa', 'wcag21a', 'wcag21aa', 'wcag22aa'];

/**
 * Regions axe cannot judge. Each one is exposed to assistive technology by BlueOffice's own
 * accessibility layer instead, which the keyboard walkthrough (a11y-keyboard.spec.ts) and the
 * manual screen-reader checks cover.
 */
const EXCLUDE: { selector: string; reason: string }[] = [
  {
    // The document pages / sheet grid / slide are pixels drawn by the engine on <canvas>;
    // axe sees no text in them (contrast of document content is the author's choice, not
    // UI). Their text is exposed through the visually hidden accessibility mirrors (BlueDoc
    // `doc-a11y-mirror`, BlueSheet `role=grid` `sheet-a11y-grid`, BlueSlide
    // `slide-a11y-mirror`), which stay IN the scan: `expectMirror` checks each one exists and
    // holds the document's text (a11y-mirror.spec.ts checks their structure in detail).
    selector: 'canvas',
    reason: 'engine-drawn document content (pixels)',
  },
];

const failures: string[] = [];

/** The accessibility mirror `testId` exists, is not `display: none` and holds `text`. */
async function expectMirror(page: Page, testId: string, text: string | RegExp) {
  const m = page.getByTestId(testId);
  await expect(m).toBeAttached();
  await expect(m).toContainText(text);
  expect(await m.evaluate((e) => getComputedStyle(e).display)).not.toBe('none');
}

async function scan(page: Page, label: string, include?: string) {
  // Let transitions and lazily rendered panels settle.
  await page.waitForTimeout(150);
  let b = new AxeBuilder({ page }).withTags(TAGS);
  for (const e of EXCLUDE) b = b.exclude(e.selector);
  if (include) b = b.include(include);
  const r = await b.analyze();
  for (const v of r.violations) {
    const nodes = v.nodes
      .slice(0, 5)
      .map((n) => `${n.target.join(' ')} — ${(n.failureSummary ?? '').replace(/\s+/g, ' ')}`)
      .join('\n      ');
    failures.push(`[${label}] ${v.id} (${v.impact}, ${v.nodes.length}): ${v.help}\n      ${nodes}`);
  }
}

function report(testName: string) {
  const out = process.env.A11Y_OUT;
  if (out) appendFileSync(out, `## ${testName}\n${failures.join('\n')}\n`);
  const list = failures.splice(0);
  expect(list, list.join('\n')).toEqual([]);
}

const cmd = (page: Page, id: string) => page.locator(`[data-command="${id}"]`).first();

async function tab(page: Page, name: string) {
  await page.getByRole('tab', { name, exact: true }).first().click();
}

/** Run a ribbon command found on `tabName`. */
async function run(page: Page, tabName: string, id: string) {
  await tab(page, tabName);
  await cmd(page, id).click();
}

/** Run a ribbon command from whichever tab (contextual tabs included) shows it. */
async function runAnywhere(page: Page, id: string) {
  const tabs = page.getByRole('tab');
  const n = await tabs.count();
  for (let i = 0; i < n; i++) {
    if (await cmd(page, id).isVisible()) break;
    await tabs.nth(i).click();
  }
  await cmd(page, id).click();
}

async function closeWithEscape(page: Page) {
  await page.keyboard.press('Escape');
  await page.waitForTimeout(100);
}

async function scanTabs(page: Page, app: string) {
  const tabs = page.locator(
    '[role=tablist][data-testid=ribbon-tabs] [role=tab], [data-testid=ribbon] [role=tab]',
  );
  const names = (await tabs.allInnerTexts()).map((s) => s.trim()).filter(Boolean);
  for (const n of [...new Set(names)]) {
    await tab(page, n);
    await scan(page, `${app} tab ${n}`);
  }
}

async function scanFileScreen(page: Page, app: string) {
  await page.getByTestId('file-tab').click();
  await expect(page.getByTestId('backstage')).toBeVisible();
  await scan(page, `${app} File › Info`);
  const nav = page.getByRole('navigation', { name: 'Tệp' });
  for (const [name, pageId] of [
    ['Lưu thành', 'saveAs'],
    ['Tùy chọn', 'options'],
  ] as const) {
    await nav.getByRole('button', { name, exact: true }).click();
    await expect(page.getByTestId('backstage')).toHaveAttribute('data-page', pageId);
    await scan(page, `${app} File › ${name}`);
  }
  await closeWithEscape(page);
  await expect(page.getByTestId('backstage')).toHaveCount(0);
}

async function scanDialog(page: Page, label: string, open: () => Promise<void>) {
  await open();
  // Modal dialogs and Popover panels are role=dialog; pickers are role=menu.
  await expect(page.locator('[role=dialog]:visible, [role=menu]:visible').last()).toBeVisible();
  await page.waitForTimeout(250);
  await scan(page, label);
  await closeWithEscape(page);
}

async function themes(page: Page, app: string, extra: () => Promise<void>) {
  for (const [name, media] of [
    ['dark', { colorScheme: 'dark' as const, contrast: 'no-preference' as const }],
    ['high contrast', { colorScheme: 'light' as const, contrast: 'more' as const }],
  ] as const) {
    await page.emulateMedia(media);
    await expect(page.locator('[data-app][data-theme]').first()).toHaveAttribute(
      'data-theme',
      name === 'dark' ? 'dark' : 'hc',
    );
    await page.waitForTimeout(200);
    await scan(page, `${app} ${name}`);
    await extra();
    await page.emulateMedia({ colorScheme: 'light', contrast: 'no-preference' });
  }
}

test('BlueDoc: start screen, ribbon, File screen, dialogs, task panes, themes', async ({
  page,
}) => {
  test.setTimeout(240_000);
  await page.goto('/doc');
  await expect(page.getByTestId('new-document')).toBeVisible();
  await scan(page, 'start screen');
  await page.getByTestId('new-document').click();
  await expect(page.getByTestId('caret')).toBeVisible();
  await page.keyboard.insertText('Báo cáo kiểm tra khả năng tiếp cận');
  await expect(page.locator('.bo-page[data-page="1"]')).toHaveAttribute('data-rendered', 'true');
  await expectMirror(page, 'doc-a11y-mirror', 'Báo cáo kiểm tra khả năng tiếp cận');
  await scan(page, 'doc document');
  await scanTabs(page, 'doc');
  await scanFileScreen(page, 'doc');
  await scanDialog(page, 'doc Font', () => run(page, 'Trang đầu', 'format.fontDialog'));
  await scanDialog(page, 'doc Paragraph spacing', () =>
    tab(page, 'Trang đầu').then(() => page.getByTestId('line-spacing').click()),
  );
  await scanDialog(page, 'doc Find & Replace', () => run(page, 'Trang đầu', 'edit.replace'));
  // Insert › Table inserts a 3 × 3 table at once: scan the table tools tabs, then the table
  // properties dialog.
  await run(page, 'Chèn', 'insert.table');
  await page.waitForTimeout(400);
  await scan(page, 'doc table inserted');
  await scanDialog(page, 'doc Table properties', () => runAnywhere(page, 'table.properties'));
  for (const [id, tabName, label] of [
    ['view.stylesPane', 'Xem', 'Styles'],
    ['view.navigationPane', 'Xem', 'Navigation'],
    ['review.comments', 'Xem lại', 'Comments'],
    ['script.editor', 'Xem', 'BlueScript'],
  ] as const) {
    await run(page, tabName, id);
    await page.waitForTimeout(300);
    await scan(page, `doc pane ${label}`);
    await run(page, tabName, id);
  }
  await themes(page, 'doc', async () => {
    await scanDialog(page, 'doc Font (theme)', () => run(page, 'Trang đầu', 'format.fontDialog'));
  });
  report('BlueDoc');
});

test('BlueSheet: ribbon, File screen, dialogs, task panes, themes', async ({ page }) => {
  test.setTimeout(240_000);
  await page.goto('/sheet');
  await expect(page.getByTestId('sheet-canvas')).toHaveAttribute('data-rendered', /\d+/);
  await page.getByLabel('Ô đang nhập').focus();
  await page.keyboard.type('Ô có chữ');
  await page.keyboard.press('Enter');
  await expectMirror(page, 'sheet-a11y-grid', 'Ô có chữ');
  await scan(page, 'sheet workbook');
  await scanTabs(page, 'sheet');
  await scanFileScreen(page, 'sheet');
  await scanDialog(page, 'sheet Format Cells', () => run(page, 'Trang đầu', 'format.cells'));
  await scanDialog(page, 'sheet Insert chart', () => run(page, 'Chèn', 'insert.chart'));
  await scanDialog(page, 'sheet Text to Columns', () => run(page, 'Dữ liệu', 'data.textToColumns'));
  // Step 2 of the fixed-width split: the ruler with its break lines.
  await scanDialog(page, 'sheet Text to Columns fixed width', async () => {
    await run(page, 'Dữ liệu', 'data.textToColumns');
    await page.getByTestId('split-fixed').check();
    await page.getByTestId('split-next').click();
    await expect(page.getByTestId('split-ruler')).toBeVisible();
  });
  // The outline bar beside the grid (rows 2–3 grouped), then the group is removed.
  await page.getByLabel('Ô đang nhập').focus();
  await page.keyboard.press('ArrowDown');
  await page.keyboard.press('Shift+ArrowDown');
  await page.keyboard.press('Alt+Shift+ArrowRight');
  await expect(page.getByTestId('outline-bar-rows')).toBeVisible();
  await scan(page, 'sheet outline bar');
  await page.keyboard.press('Control+z');
  await expect(page.getByTestId('outline-bar-rows')).toHaveCount(0);
  await scanDialog(page, 'sheet Find & Replace', async () => {
    // Find & Replace is not on the ribbon at 1366 px: its shortcut from the cell input.
    await page.getByLabel('Ô đang nhập').focus();
    await page.keyboard.press('Control+h');
  });
  for (const [id, tabName, label] of [
    ['review.comments', 'Chèn', 'Comments'],
    ['script.editor', 'Xem', 'BlueScript'],
  ] as const) {
    await run(page, tabName, id);
    await page.waitForTimeout(300);
    await scan(page, `sheet pane ${label}`);
    await closeWithEscape(page);
  }
  await themes(page, 'sheet', async () => {
    await scanDialog(page, 'sheet Format Cells (theme)', () =>
      run(page, 'Trang đầu', 'format.cells'),
    );
  });
  report('BlueSheet');
});

test('BlueSlide: ribbon, File screen, dialogs, task panes, themes', async ({ page }) => {
  test.setTimeout(240_000);
  await page.goto('/slide');
  const chooser = page.waitForEvent('filechooser');
  await page.getByRole('button', { name: 'Mở', exact: true }).click();
  await (await chooser).setFiles(deck);
  await expect(page.getByTestId('slide-overlay')).toBeVisible();
  await expectMirror(page, 'slide-a11y-mirror', /\S/);
  await scan(page, 'slide deck');
  await scanTabs(page, 'slide');
  await scanFileScreen(page, 'slide');
  // Insert › Table opens the size grid: scan it, then the slide with a 3 × 3 table selected.
  await run(page, 'Chèn', 'insert.table');
  await expect(page.getByTestId('table-size-picker')).toBeVisible();
  await scan(page, 'slide table size grid');
  await page.getByTestId('table-size-picker').locator('[data-cell="3x3"]').click();
  await page.waitForTimeout(400);
  await scan(page, 'slide table inserted');
  await scanDialog(page, 'slide Insert chart', () => run(page, 'Chèn', 'insert.chart'));
  for (const [id, tabName, label] of [
    ['view.selectionPane', 'Sắp xếp', 'Selection'],
    ['view.animationPane', 'Hiệu ứng', 'Animation'],
    ['review.comments', 'Chèn', 'Comments'],
    ['script.editor', 'Xem', 'BlueScript'],
  ] as const) {
    await run(page, tabName, id);
    await page.waitForTimeout(300);
    await scan(page, `slide pane ${label}`);
    await closeWithEscape(page);
  }
  await themes(page, 'slide', async () => {
    await scanDialog(page, 'slide Insert chart (theme)', () => run(page, 'Chèn', 'insert.chart'));
  });
  report('BlueSlide');
});
