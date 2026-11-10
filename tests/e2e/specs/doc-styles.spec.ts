import { expect, test, type Page } from '@playwright/test';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

// DOC-STYLE-001: modify Heading 1 from the Styles pane (every heading follows, kept after
// save and reopen), Reveal Formatting names the source, Organizer copies a style from a
// second file.

const corpus = join(dirname(fileURLToPath(import.meta.url)), '..', '..', 'corpus', 'doc');
const cmd = (page: Page, id: string) => page.locator(`[data-command="${id}"]`);
const page1 = (page: Page) => page.locator('.bo-page[data-page="1"]');

async function newDocument(page: Page) {
  await page.goto('/doc');
  await page.getByTestId('new-document').click();
  await expect(page.getByTestId('caret')).toBeVisible();
}

async function saveAndReopen(page: Page) {
  const download = page.waitForEvent('download');
  await page.keyboard.press('Control+s');
  const path = await (await download).path();
  await page.reload();
  const chooser = page.waitForEvent('filechooser');
  await page.getByRole('button', { name: 'Mở tệp từ máy' }).click();
  await (await chooser).setFiles(path);
  await expect(page1(page)).toHaveAttribute('data-rendered', 'true');
}

/** Caret to the start of body paragraph `n` (0-based) through the keyboard. */
async function toParagraph(page: Page, n: number) {
  // A click on the page gives the document the keyboard back.
  const box = (await page1(page).boundingBox())!;
  await page.mouse.click(box.x + box.width * 0.3, box.y + box.height * 0.08);
  await page.keyboard.press('Control+Home');
  for (let i = 0; i < n; i++) await page.keyboard.press('ArrowDown');
  await page.keyboard.press('End');
}

/** Value and source of a Reveal Formatting row. */
async function revealed(page: Page, key: string) {
  const row = page
    .getByTestId('reveal-formatting')
    .locator(`[data-testid="reveal-item"][data-key="${key}"]`)
    .first();
  if ((await row.count()) === 0) return { text: '', source: null };
  return { text: (await row.textContent()) ?? '', source: await row.getAttribute('data-source') };
}

async function openStylesPane(page: Page) {
  if (
    !(await page
      .getByTestId('styles-pane')
      .isVisible()
      .catch(() => false))
  ) {
    await page.keyboard.press('Control+Alt+Shift+S');
  }
  await expect(page.getByTestId('styles-pane')).toBeVisible();
  await page.getByTestId('styles-show').selectOption('all');
}

test('modify Heading 1: every heading changes, kept after save and reopen', async ({ page }) => {
  const errors: string[] = [];
  page.on('pageerror', (e) => errors.push(e.message));
  await newDocument(page);
  await page.keyboard.insertText('Chương một');
  await page.keyboard.press('Enter');
  await page.keyboard.insertText('Nội dung của chương');
  await page.keyboard.press('Enter');
  await page.keyboard.insertText('Chương hai');
  await page.keyboard.press('Control+Home');
  await cmd(page, 'style.heading1').click();
  await page.keyboard.press('Control+End');
  await cmd(page, 'style.heading1').click();

  await openStylesPane(page);
  await page.locator('[data-testid="style-menu"][data-style-id="Heading1"]').click();
  await page.getByTestId('style-modify').click();
  const dlg = page.getByTestId('style-dialog');
  await expect(dlg).toBeVisible();
  await expect(page.getByTestId('style-dlg-name')).toHaveValue('heading 1');
  await page.getByTestId('style-dlg-color').fill('#c00000');
  await page.getByTestId('style-dlg-ok').click();
  await expect(dlg).toBeHidden();

  await page.getByTestId('styles-reveal').click();
  await expect(page.getByTestId('reveal-formatting')).toBeVisible();
  for (const p of [0, 2]) {
    await toParagraph(page, p);
    await expect.poll(async () => (await revealed(page, 'color')).text).toContain('#C00000');
    expect((await revealed(page, 'color')).source).toBe('paragraphStyle');
  }
  await toParagraph(page, 1);
  await expect
    .poll(async () => (await revealed(page, 'style')).text, { timeout: 5000 })
    .toContain('Normal');
  await expect.poll(async () => (await revealed(page, 'color')).text).not.toContain('#C00000');

  await saveAndReopen(page);
  await openStylesPane(page);
  await page.getByTestId('styles-reveal').click();
  for (const p of [0, 2]) {
    await toParagraph(page, p);
    await expect.poll(async () => (await revealed(page, 'color')).text).toContain('#C00000');
  }
  expect(errors).toEqual([]);
});

test('organizer copies a style from a second file', async ({ page }) => {
  const errors: string[] = [];
  page.on('pageerror', (e) => errors.push(e.message));
  await newDocument(page);
  await page.keyboard.insertText('Về việc tổ chức hội nghị');
  await openStylesPane(page);
  await expect(page.locator('[data-testid="style-item"][data-style-id="TrichYeu"]')).toHaveCount(0);
  await page.getByTestId('styles-organizer').click();
  const org = page.getByTestId('style-organizer');
  await expect(org).toBeVisible();
  const chooser = page.waitForEvent('filechooser');
  await page.getByTestId('organizer-open').click();
  await (await chooser).setFiles(join(corpus, 'styles-02-based-on-chain.docx'));
  const file = page.getByTestId('organizer-file');
  await expect(file.locator('option[value="TrichYeu"]')).toHaveCount(1);
  await file.selectOption('TrichYeu');
  await page.getByTestId('organizer-copy-to-current').click();
  await expect(
    page.getByTestId('organizer-current').locator('option[value="TrichYeu"]'),
  ).toHaveCount(1);
  await page.getByTestId('organizer-close').click();
  await expect(org).toBeHidden();
  // The copied style is in the pane and applies (centred, bold).
  const item = page.locator('[data-testid="style-item"][data-style-id="TrichYeu"]');
  await expect(item).toHaveCount(1);
  await item.click();
  await page.getByTestId('styles-reveal').click();
  await toParagraph(page, 0);
  await expect.poll(async () => (await revealed(page, 'align')).text).toMatch(/center/);
  expect(errors).toEqual([]);
});
