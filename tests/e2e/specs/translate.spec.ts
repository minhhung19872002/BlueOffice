import { expect, test, type Page } from '@playwright/test';

// CORE-LANG-001: translation through the server's /api/translate (faked here by routing).
const cmd = (page: Page, id: string) => page.locator(`[data-command="${id}"]`);

/** A fake BlueOffice server translating `vi` → `en` by a small dictionary. */
async function fakeServer(page: Page, sent: { texts: string[]; from: string; to: string }[]) {
  const words: Record<string, string> = {
    'Xin chào các bạn': 'Hello to all of you',
    'Hôm nay trời đẹp quá': 'The weather is lovely today',
  };
  await page.route('**/api/translate/languages**', (route) =>
    route.fulfill({ json: { languages: ['vi', 'en'] } }),
  );
  await page.route('**/api/translate', async (route) => {
    const body = route.request().postDataJSON() as { texts: string[]; from: string; to: string };
    sent.push(body);
    await route.fulfill({ json: { texts: body.texts.map((t) => words[t] ?? `EN ${t}`) } });
  });
}

test('translates the selection and the whole document into a copy', async ({ page }) => {
  const errors: string[] = [];
  page.on('pageerror', (e) => errors.push(e.message));
  const sent: { texts: string[]; from: string; to: string }[] = [];
  await fakeServer(page, sent);
  await page.goto('/doc');
  await page.getByTestId('new-document').click();
  await expect(page.getByTestId('caret')).toBeVisible();
  await page.keyboard.insertText('Xin chào các bạn');
  await page.keyboard.press('Enter');
  await page.keyboard.insertText('Hôm nay trời đẹp quá');
  await expect(page.getByTestId('status-words')).toHaveText('9 từ');

  // Select the first paragraph and translate it.
  await page.keyboard.press('Control+Home');
  await page.keyboard.press('Shift+End');
  await page.getByRole('tab', { name: 'Xem lại' }).click();
  await cmd(page, 'review.translate').click();
  const pane = page.getByTestId('translate-pane');
  await expect(pane).toBeVisible();
  await expect(pane.getByTestId('translate-source')).toHaveText('Xin chào các bạn');
  await expect(pane.getByTestId('translate-result')).toHaveValue('Hello to all of you');
  expect(sent[0]).toEqual({ texts: ['Xin chào các bạn'], from: 'vi', to: 'en' });
  await pane.getByTestId('translate-replace').click();
  // "Hello to all of you" (5) + "Hôm nay trời đẹp quá" (5).
  await expect(page.getByTestId('status-words')).toHaveText('10 từ');

  // The whole document: a translated copy opens; only Vietnamese text is sent.
  await pane.getByTestId('translate-action-document').click();
  await expect(pane.getByTestId('translate-notice')).toContainText('(EN).docx');
  expect(sent.at(-1)).toEqual({ texts: ['Hôm nay trời đẹp quá'], from: 'vi', to: 'en' });
  // "Hello to all of you" (5) + "The weather is lovely today" (5): two paragraphs kept.
  await expect(page.getByTestId('status-words')).toHaveText('10 từ');
  await page.keyboard.press('Escape');
  expect(errors).toEqual([]);
});

test('says translation is unavailable when the server has none', async ({ page }) => {
  let posted = 0;
  await page.route('**/api/translate/languages**', (route) =>
    route.fulfill({ status: 404, json: { code: 'unavailable' } }),
  );
  await page.route('**/api/translate', (route) => {
    posted++;
    return route.fulfill({ status: 404, json: { code: 'unavailable' } });
  });
  await page.goto('/doc');
  await page.getByTestId('new-document').click();
  await expect(page.getByTestId('caret')).toBeVisible();
  await page.keyboard.insertText('Xin chào các bạn');
  await page.keyboard.press('Control+a');
  await page.getByRole('tab', { name: 'Xem lại' }).click();
  await cmd(page, 'review.translate').click();
  await expect(page.getByTestId('translate-unavailable')).toBeVisible();
  expect(posted).toBe(0);
});
