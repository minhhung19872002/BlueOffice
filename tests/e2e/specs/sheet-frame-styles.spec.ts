import { expect, test, type Locator, type Page } from '@playwright/test';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

// SHEET-SLICER-001 / SHEET-TIMELINE-001, as in the spreadsheet application they are compatible
// with: Timeline › the handles at the ends of the selection drag that end (the other stays);
// changing the time level keeps the chosen dates; New Slicer Style / New Timeline Style (a name,
// the elements, a format per element) adds a custom style to the workbook, offered with the
// built-in ones and applied once chosen (one undo step). Saving and reading back is covered by
// the engine tests (`sheet_pivot_frames.rs`): a workbook opened from a file is not saved as a
// download in the test browser.

const here = dirname(fileURLToPath(import.meta.url));
const corpus = join(here, '..', '..', 'corpus', 'sheet');

async function openFile(page: Page, path: string) {
  const root = page.locator('[data-app="sheet"]');
  await expect(root).toHaveAttribute('aria-busy', 'false');
  const seq = Number(await root.getAttribute('data-load-seq'));
  const chooser = page.waitForEvent('filechooser');
  await page.locator('[data-command="file.open"]').click();
  await (await chooser).setFiles(path);
  await expect
    .poll(async () => Number(await root.getAttribute('data-load-seq')))
    .toBeGreaterThan(seq);
  await expect(root).toHaveAttribute('aria-busy', 'false');
}

/** Drags from the middle of `from` to the middle of `to` with the mouse. */
async function drag(page: Page, from: Locator, to: Locator) {
  await from.scrollIntoViewIfNeeded();
  const a = await from.boundingBox();
  expect(a).not.toBeNull();
  if (!a) return;
  await page.mouse.move(a.x + a.width / 2, a.y + a.height / 2);
  await page.mouse.down();
  const b = await to.boundingBox();
  expect(b).not.toBeNull();
  if (!b) return;
  await page.mouse.move(b.x + b.width / 2, b.y + b.height / 2, { steps: 8 });
  await page.mouse.up();
}

/** New style… in the open options dialog: name, then `formats` element by element, OK. */
async function newStyle(
  page: Page,
  name: string,
  formats: { element: string; fill?: string; text?: string; bold?: boolean }[],
) {
  await page.getByTestId('frame-style-new').click();
  const dlg = page.getByTestId('frame-style-dialog');
  await expect(dlg).toBeVisible();
  await dlg.getByTestId('frame-style-name').fill(name);
  for (const f of formats) {
    await dlg.getByTestId(`frame-style-element-${f.element}`).check();
    if (f.fill) {
      await dlg.getByTestId('frame-style-use-fill').check();
      await dlg.getByTestId('frame-style-fill').fill(f.fill);
    }
    if (f.text) {
      await dlg.getByTestId('frame-style-use-text').check();
      await dlg.getByTestId('frame-style-text').fill(f.text);
    }
    if (f.bold) await dlg.getByTestId('frame-style-bold').check();
  }
  await dlg.getByTestId('frame-style-ok').click();
  await expect(dlg).toHaveCount(0);
  // The new style is the choice of the options dialog.
  await expect(page.getByTestId('frame-style')).toHaveValue(name);
}

test('BlueSheet: timeline selection handles, time level and a new timeline style', async ({
  page,
}) => {
  const errors: string[] = [];
  page.on('pageerror', (e) => errors.push(e.message));
  await page.goto('/sheet');
  // Rows dated every 23 days from 2023-03-15 (writer-timelines-1.xlsx).
  await openFile(page, join(corpus, 'writer-timelines-1.xlsx'));
  await page.getByRole('tab', { name: 'Báo cáo' }).click();
  const tl = page.getByTestId('timeline');
  await expect(tl).toBeVisible();
  const period = (name: string) => tl.getByRole('button', { name, exact: true });
  // Quarters; Q1 2024 … Q3 2024 by a click and a Shift click.
  await tl.getByTestId('timeline-level').selectOption('1');
  await expect(tl).toHaveAttribute('data-level', '1');
  await period('Quý 1 2024').click();
  await period('Quý 3 2024').click({ modifiers: ['Shift'] });
  await expect(tl.getByTestId('timeline-range')).toHaveText('Quý 1 2024 – Quý 3 2024');
  // The end handle sits on the last chosen period; dragged back to Q2 the range ends there.
  const end = tl.getByTestId('timeline-edge-end');
  await expect(period('Quý 3 2024').getByTestId('timeline-edge-end')).toHaveCount(1);
  await drag(page, end, period('Quý 2 2024'));
  await expect(tl.getByTestId('timeline-range')).toHaveText('Quý 1 2024 – Quý 2 2024');
  await expect(period('Quý 3 2024')).toHaveAttribute('aria-pressed', 'false');
  // The start handle dragged to Q4 2023: the range starts there, the end stays.
  await drag(page, tl.getByTestId('timeline-edge-start'), period('Quý 4 2023'));
  await expect(tl.getByTestId('timeline-range')).toHaveText('Quý 4 2023 – Quý 2 2024');
  await expect(period('Quý 4 2023')).toHaveAttribute('aria-pressed', 'true');
  // Months: the same dates stay chosen (October 2023 … June 2024).
  await tl.getByTestId('timeline-zoom-in').click();
  await expect(tl).toHaveAttribute('data-level', '2');
  await expect(tl.getByTestId('timeline-range')).toHaveText('10/2023 – 06/2024');
  await expect(period('09/2023')).toHaveAttribute('aria-pressed', 'false');
  await expect(period('07/2024')).toHaveAttribute('aria-pressed', 'false');

  // Timeline options › New style…: selected blocks in red with white bold text.
  await tl.getByTestId('timeline-settings').click();
  await expect(page.getByTestId('frame-options-dialog')).toBeVisible();
  await newStyle(page, 'Dòng thời gian đỏ', [
    { element: 'selected', fill: '#c00000', text: '#ffffff', bold: true },
    { element: 'header', text: '#1f4e79' },
  ]);
  await page.getByTestId('frame-options-ok').click();
  await expect(page.getByTestId('frame-options-dialog')).toHaveCount(0);
  await expect(tl).toHaveAttribute('data-style', 'Dòng thời gian đỏ');
  await expect
    .poll(() => period('01/2024').evaluate((b) => getComputedStyle(b).backgroundColor))
    .toBe('rgb(192, 0, 0)');

  expect(errors).toEqual([]);
});

test('BlueSheet: a new slicer style joins the workbook styles and comes back from the file', async ({
  page,
}) => {
  const errors: string[] = [];
  page.on('pageerror', (e) => errors.push(e.message));
  await page.goto('/sheet');
  // A pivot table slicer drawn with the file's own custom style "Kiểu bộ lọc Bluestar".
  await openFile(page, join(corpus, 'writer-frame-styles-1.xlsx'));
  await page.getByRole('tab', { name: 'Báo cáo' }).click();
  const slicer = page.getByTestId('slicer');
  await expect(slicer).toBeVisible();
  await expect(slicer).toHaveAttribute('data-style', 'Kiểu bộ lọc Bluestar');
  await slicer.getByTestId('slicer-header').click();
  await slicer.getByTestId('slicer-settings').click();
  await expect(page.getByTestId('frame-options-dialog')).toBeVisible();
  // The default name is the first free one; a name in use is refused with a message.
  await page.getByTestId('frame-style-new').click();
  const dlg = page.getByTestId('frame-style-dialog');
  await expect(dlg.getByTestId('frame-style-name')).toHaveValue('Kiểu slicer 1');
  await dlg.getByTestId('frame-style-name').fill('Kiểu bộ lọc Bluestar');
  await dlg.getByTestId('frame-style-ok').click();
  await expect(dlg.getByRole('alert')).toBeVisible();
  await dlg.getByRole('button', { name: 'Huỷ' }).click();
  await expect(dlg).toHaveCount(0);
  // Selected items green, the items not chosen pale yellow; the preview shows them.
  await newStyle(page, 'Bộ lọc xanh lá', [
    { element: 'selected', fill: '#548235', text: '#ffffff' },
    { element: 'unselected', fill: '#fff2cc' },
  ]);
  // Both custom styles are offered before the built-in ones.
  const options = page.getByTestId('frame-style').locator('optgroup option');
  await expect(options).toHaveText(['Kiểu bộ lọc Bluestar', 'Bộ lọc xanh lá']);
  await page.getByTestId('frame-options-ok').click();
  await expect(slicer).toHaveAttribute('data-style', 'Bộ lọc xanh lá');
  const on = slicer.locator('[aria-pressed="true"]').first();
  await expect.poll(() => on.evaluate((b) => getComputedStyle(b).backgroundColor)).toBe(
    'rgb(84, 130, 53)',
  );

  // One undo step takes the choice back, the next the new style.
  await page.getByLabel('Ô đang nhập').focus();
  await page.keyboard.press('Control+z');
  await expect(slicer).toHaveAttribute('data-style', 'Kiểu bộ lọc Bluestar');
  await page.keyboard.press('Control+y');
  await expect(slicer).toHaveAttribute('data-style', 'Bộ lọc xanh lá');

  // Offered again in the options with the file's own style.
  await slicer.getByTestId('slicer-header').click();
  await slicer.getByTestId('slicer-settings').click();
  await expect(page.getByTestId('frame-style').locator('optgroup option')).toHaveText([
    'Kiểu bộ lọc Bluestar',
    'Bộ lọc xanh lá',
  ]);
  await expect(page.getByTestId('frame-style')).toHaveValue('Bộ lọc xanh lá');
  expect(errors).toEqual([]);
});
