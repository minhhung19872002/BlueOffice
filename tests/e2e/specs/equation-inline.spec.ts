import { expect, test, type Page } from '@playwright/test';
import { copyFileSync, mkdirSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const outDir = join(dirname(fileURLToPath(import.meta.url)), '..', '..', 'fidelity', 'output');

// CORE-MATH-010: edit an equation in place — click into a slot, type, Tab, autobuild, Backspace,
// undo, save and reopen.
const cmd = (page: Page, id: string) => page.locator(`[data-command="${id}"]`);
const page1 = (page: Page) => page.locator('.bo-page[data-page="1"]');

async function box(page: Page, testId: string) {
  await expect(page.getByTestId(testId).first()).toBeAttached();
  const b = await page.getByTestId(testId).first().boundingBox();
  if (!b) throw new Error(`${testId} has no box`);
  return b;
}

test('edits an equation in place', async ({ page }) => {
  const errors: string[] = [];
  page.on('pageerror', (e) => errors.push(e.message));
  await page.goto('/doc');
  await page.getByTestId('new-document').click();
  await expect(page.getByTestId('caret')).toBeVisible();
  await page.keyboard.insertText('Ta có ');

  // Insert a/b.
  await page.getByRole('tab', { name: 'Chèn' }).click();
  await cmd(page, 'insert.equation').click();
  const dialog = page.getByTestId('equation-dialog');
  await dialog.getByTestId('equation-input').fill('a/b');
  await dialog.getByTestId('equation-ok').click();
  await expect(dialog).toHaveCount(0);

  // Into the equation: its box is shown; Esc leaves it.
  await cmd(page, 'equation.editInPlace').click();
  await expect(page.getByTestId('bo-math-zone')).toBeVisible();
  const zone = await box(page, 'bo-math-zone');
  await page.keyboard.press('Escape');
  await expect(page.getByTestId('bo-math-zone')).toHaveCount(0);

  // Click into the denominator (below the fraction bar, in the middle).
  await page.mouse.click(zone.x + zone.width / 2, zone.y + zone.height * 0.78);
  await expect(page.getByTestId('bo-math-zone')).toBeVisible();
  const inDen = await box(page, 'caret');
  expect(inDen.y).toBeGreaterThan(zone.y + zone.height / 2 - 2);
  expect(inDen.height).toBeLessThan(zone.height * 0.8);
  await page.keyboard.type('c');

  // Tab: the next slot (after the fraction, full height).
  await page.keyboard.press('Tab');
  const after = await box(page, 'caret');
  expect(after.height).toBeGreaterThan(inDen.height);
  expect(after.x).toBeGreaterThan(inDen.x);

  // Typing x^2 and a space builds the superscript: one step left enters it (a small, raised caret).
  await page.keyboard.type('+x^2 ');
  await page.keyboard.press('ArrowLeft');
  const inSup = await box(page, 'caret');
  expect(inSup.height).toBeLessThan(after.height * 0.85);
  expect(inSup.y + inSup.height).toBeLessThan(after.y + after.height - 2);
  // Out of the superscript: just after the structure.
  await page.keyboard.press('ArrowRight');

  // Backspace selects the superscript structure, a second one removes it.
  await page.keyboard.press('Backspace');
  await expect(page.locator('.bo-sel')).toHaveCount(1);
  await page.keyboard.press('Backspace');
  await expect(page.locator('.bo-sel')).toHaveCount(0);
  await page.keyboard.press('ArrowLeft');
  const noSup = await box(page, 'caret');
  expect(noSup.height).toBeGreaterThan(inSup.height);

  // Undo brings it back (still editing in place).
  await page.keyboard.press('Control+z');
  await expect(page.getByTestId('bo-math-zone')).toBeVisible();
  await page.keyboard.press('End');
  await page.keyboard.press('ArrowLeft');
  const supAgain = await box(page, 'caret');
  expect(supAgain.height).toBeLessThan(after.height * 0.85);
  await page.keyboard.press('Escape');

  // The equation dialog reads the edited equation back.
  await cmd(page, 'insert.equation').click();
  await expect(dialog.getByTestId('equation-input')).toHaveValue(/^a\/\((cb|bc)\)\+x\^2$/);
  const linear = await dialog.getByTestId('equation-input').inputValue();
  await page.keyboard.press('Escape');
  await expect(dialog).toHaveCount(0);

  // Save, reload, reopen: the equation reads back the same.
  const download = page.waitForEvent('download');
  await page.keyboard.press('Control+s');
  const path = await (await download).path();
  // Kept for validation (bo-cli validate tests/fidelity/output/web-equation-inline.docx).
  mkdirSync(outDir, { recursive: true });
  copyFileSync(path, join(outDir, 'web-equation-inline.docx'));
  await page.reload();
  const chooser = page.waitForEvent('filechooser');
  await page.getByRole('button', { name: 'Mở tệp từ máy' }).click();
  await (await chooser).setFiles(path);
  await expect(page1(page)).toHaveAttribute('data-rendered', 'true');
  await page.keyboard.press('Control+End');
  await page.getByRole('tab', { name: 'Chèn' }).click();
  await cmd(page, 'insert.equation').click();
  await expect(page.getByTestId('equation-input')).toHaveValue(linear);
  expect(errors).toEqual([]);
});
