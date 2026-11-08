import { expect, test } from '@playwright/test';

// CORE-SPELL-004: red wavy underline under misspelt words, corrections on right click.
test('underlines misspelt Vietnamese and English words and corrects them from the menu', async ({
  page,
}) => {
  const errors: string[] = [];
  page.on('pageerror', (e) => errors.push(e.message));
  await page.goto('/doc');
  await page.getByTestId('new-document').click();
  await expect(page.getByTestId('caret')).toBeVisible();
  await page.keyboard.insertText('Xin chaò bạn, the documnet is ready');

  // Two marks: "chaò" (tone on the wrong vowel) and "documnet".
  const marks = page.getByTestId('spell-mark');
  await expect(marks).toHaveCount(2, { timeout: 15_000 });

  // Right click the first mark: "chào" is offered; picking it fixes the word.
  const first = (await marks.first().boundingBox())!;
  await page.mouse.click(first.x + first.width / 2, first.y - 6, { button: 'right' });
  const menu = page.getByTestId('spell-menu');
  await expect(menu).toBeVisible();
  await menu.getByRole('menuitem', { name: 'chào', exact: true }).click();
  await expect(marks).toHaveCount(1, { timeout: 15_000 });

  // "Thêm vào từ điển" on the English typo removes the last mark.
  const second = (await marks.first().boundingBox())!;
  await page.mouse.click(second.x + second.width / 2, second.y - 6, { button: 'right' });
  await page.getByTestId('spell-menu').getByRole('menuitem', { name: 'Thêm vào từ điển' }).click();
  await expect(marks).toHaveCount(0, { timeout: 15_000 });

  // The word count shows the corrected text ("chào" replaced "chaò").
  await expect(page.getByTestId('status-words')).toHaveText('7 từ');
  expect(errors).toEqual([]);
});
