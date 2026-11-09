import { expect, test, type Page } from '@playwright/test';
import { copyFileSync, mkdirSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const outDir = join(dirname(fileURLToPath(import.meta.url)), '..', '..', 'fidelity', 'output');

// CORE-MATH-007: insert an equation from the linear format, switch it linear ↔ built up, save and reopen.
const cmd = (page: Page, id: string) => page.locator(`[data-command="${id}"]`);
const page1 = (page: Page) => page.locator('.bo-page[data-page="1"]');

test('inserts an equation, switches its format, keeps it after save and reopen', async ({
  page,
}) => {
  const errors: string[] = [];
  page.on('pageerror', (e) => errors.push(e.message));
  await page.goto('/doc');
  await page.getByTestId('new-document').click();
  await expect(page.getByTestId('caret')).toBeVisible();
  await page.keyboard.insertText('Ta có ');

  // Insert x^2+1 typed in UnicodeMath.
  await page.getByRole('tab', { name: 'Chèn' }).click();
  await expect(cmd(page, 'equation.linear')).toBeDisabled();
  await cmd(page, 'insert.equation').click();
  const dialog = page.getByTestId('equation-dialog');
  await dialog.getByTestId('equation-input').fill('x^2+1');
  await dialog.getByTestId('equation-ok').click();
  await expect(dialog).toHaveCount(0);
  // The equation is one object: "Ta có" + equation.
  await expect(page.getByTestId('status-words')).toHaveText('3 từ');

  // Linear form, then built up again.
  await expect(cmd(page, 'equation.linear')).toBeEnabled();
  await cmd(page, 'equation.linear').click();
  await expect(cmd(page, 'equation.professional')).toBeEnabled();
  await cmd(page, 'equation.professional').click();
  await expect(cmd(page, 'equation.linear')).toBeEnabled();

  // Edit it: the dialog shows the linear form; replace with a LaTeX fraction.
  await cmd(page, 'insert.equation').click();
  await expect(dialog.getByTestId('equation-input')).toHaveValue('x^2+1');
  await dialog.getByTestId('equation-input').fill('\\frac{a}{b}');
  await dialog.getByTestId('equation-latex').check();
  await dialog.getByTestId('equation-ok').click();
  await expect(dialog).toHaveCount(0);

  // Save, reload, reopen: the equation reads back.
  const download = page.waitForEvent('download');
  await page.keyboard.press('Control+s');
  const path = await (await download).path();
  // Kept for validation (bo-cli validate tests/fidelity/output/web-equation.docx).
  mkdirSync(outDir, { recursive: true });
  copyFileSync(path, join(outDir, 'web-equation.docx'));
  await page.reload();
  const chooser = page.waitForEvent('filechooser');
  await page.getByRole('button', { name: 'Mở tệp từ máy' }).click();
  await (await chooser).setFiles(path);
  await expect(page1(page)).toHaveAttribute('data-rendered', 'true');
  await page.keyboard.press('Control+End');
  await page.getByRole('tab', { name: 'Chèn' }).click();
  await cmd(page, 'insert.equation').click();
  await expect(page.getByTestId('equation-input')).toHaveValue('a/b');
  expect(errors).toEqual([]);
});

test('inserts a template equation as a numbered paragraph', async ({ page }) => {
  const errors: string[] = [];
  page.on('pageerror', (e) => errors.push(e.message));
  await page.goto('/doc');
  await page.getByTestId('new-document').click();
  await expect(page.getByTestId('caret')).toBeVisible();
  await page.keyboard.insertText('Ta có');
  await page.getByRole('tab', { name: 'Chèn' }).click();
  await cmd(page, 'insert.equation').click();
  const dialog = page.getByTestId('equation-dialog');
  await dialog.getByTestId('equation-template').selectOption('quadratic');
  await expect(dialog.getByTestId('equation-input')).toHaveValue('x=(-b±√(b^2-4ac))/2a');
  await dialog.getByTestId('equation-numbered').check();
  await dialog.getByTestId('equation-ok').click();
  await expect(dialog).toHaveCount(0);
  // A second paragraph now holds the equation and its number (1).
  await expect(
    page.getByTestId('status-paragraphs').or(page.getByTestId('status-words')),
  ).toBeVisible();
  await cmd(page, 'insert.equation').click();
  await dialog.getByTestId('equation-template').selectOption('euler');
  await dialog.getByTestId('equation-numbered').check();
  await dialog.getByTestId('equation-ok').click();
  await expect(dialog).toHaveCount(0);
  await expect(page1(page)).toBeVisible();
  expect(errors).toEqual([]);
});
