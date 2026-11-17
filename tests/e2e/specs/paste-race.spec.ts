import { expect, test } from '@playwright/test';

test('paste options shortcut reopens the selected mode', async ({ page }) => {
  await page.goto('/doc');
  await page.getByTestId('new-document').click();
  await expect(page.getByTestId('caret')).toBeVisible();
  const input = page.getByTestId('ime-input');
  await expect(input).toBeAttached();
  await input.focus();
  // Synthetic clipboard content avoids modifying the machine clipboard.
  await input.evaluate((el) => {
    const data = new DataTransfer();
    data.setData('text/plain', 'Một hai');
    data.setData('text/html', '<b>Một hai</b>');
    el.dispatchEvent(
      new ClipboardEvent('paste', { bubbles: true, cancelable: true, clipboardData: data }),
    );
  });
  await expect(page.getByTestId('paste-options')).toBeVisible();
  await page.getByTestId('paste-options').click();
  await page.locator('[data-paste-option="text"]').click();
  await page.keyboard.press('Alt+Shift+V');
  await expect(page.getByTestId('paste-options-menu')).toBeVisible();
  await expect(page.locator('[data-paste-option="text"]')).toHaveAttribute('aria-checked', 'true');
});
