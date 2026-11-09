import { expect, test } from '@playwright/test';

const cmd = (page: import('@playwright/test').Page, id: string) =>
  page.locator(`[data-command="${id}"]`);

test('inserts fields from the dialog: a formula in words and a formatted date', async ({
  page,
}) => {
  const errors: string[] = [];
  page.on('pageerror', (e) => errors.push(e.message));
  await page.goto('/doc');
  await page.getByTestId('new-document').click();
  await expect(page.getByTestId('caret')).toBeVisible();
  await page.getByRole('tab', { name: 'Chèn' }).click();
  await cmd(page, 'insert.field').click();
  await expect(page.getByTestId('field-dialog')).toBeVisible();
  await page.getByTestId('field-kind').selectOption('=');
  await page.getByTestId('field-args').fill('21*2');
  await page.getByTestId('field-format').selectOption('CardText');
  await expect(page.getByTestId('field-code')).toHaveText('= 21*2 \\* CardText');
  await page.getByTestId('field-ok').click();
  await expect(page.getByTestId('field-dialog')).toHaveCount(0);
  await expect(page.getByTestId('status-words')).toHaveText('3 từ');
  // A date with its own picture.
  await page.keyboard.insertText(' — ');
  await cmd(page, 'insert.field').click();
  await page.getByTestId('field-kind').selectOption('DATE');
  await page.getByTestId('field-picture').fill("'năm' yyyy");
  await expect(page.getByTestId('field-code')).toHaveText('DATE \\@ "\'năm\' yyyy"');
  await page.getByTestId('field-ok').click();
  // The date's result ("năm 2026") adds words to the count.
  await expect(page.getByTestId('field-dialog')).toHaveCount(0);
  await expect(page.getByTestId('status-words')).not.toHaveText('3 từ');
  expect(errors).toEqual([]);
});
