import { expect, test } from '@playwright/test';

// UI-FRAME-001: the shared window frame — 40px title bar on the app accent band, quick access,
// 24px status bar — in the three apps, each with its own accent.
const accents: Record<string, string> = {
  doc: 'rgb(79, 70, 229)', // #4F46E5
  sheet: 'rgb(11, 122, 107)', // #0B7A6B (strong teal, AA with white text)
  slide: 'rgb(194, 24, 91)', // #C2185B (strong coral)
};

for (const app of ['doc', 'sheet', 'slide'] as const) {
  test(`frame: ${app} has a 40px accent title bar and a 24px status bar`, async ({ page }) => {
    await page.goto(`/${app}`);
    const bar = page.getByTestId('title-bar');
    await expect(bar).toBeVisible();
    const box = await bar.boundingBox();
    expect(box?.height).toBe(40);
    expect(await bar.evaluate((el) => getComputedStyle(el).backgroundColor)).toBe(accents[app]);
    await expect(bar.locator(`[data-icon="app.${app}"]`)).toBeVisible();
    await expect(page.getByRole('toolbar', { name: 'Thanh truy cập nhanh' })).toBeVisible();
    await expect(page.locator('[data-command="qat:file.save"]')).toBeVisible();
    const status = page.getByTestId('status-bar');
    expect((await status.boundingBox())?.height).toBe(24);
  });
}

test('frame: quick-access undo reverts typing in BlueDoc', async ({ page }) => {
  await page.goto('/doc');
  await page.getByTestId('new-document').click();
  await expect(page.getByTestId('caret')).toBeVisible();
  await page.keyboard.insertText('Xin chào');
  await expect(page.getByTestId('status-words')).toHaveText('2 từ');
  await expect(page.getByTestId('save-state')).toHaveAttribute('data-state', 'unsaved');
  await page.locator('[data-command="qat:edit.undo"]').click();
  await expect(page.getByTestId('status-words')).toHaveText('0 từ');
});
