import { expect, test } from '@playwright/test';

// Compact groups must not truncate their names as the ribbon replans on resize.
test('ribbon group names fit at desktop widths in all apps and tabs', async ({ page }) => {
  test.setTimeout(120_000);
  for (const width of [720, 900, 1164, 1366]) {
    await page.setViewportSize({ width, height: 782 });
    for (const app of ['doc', 'sheet', 'slide']) {
      await page.goto(`/${app}`);
      if (app === 'doc') await page.getByTestId('new-document').click();
      if (app === 'slide') await page.getByTestId('new-presentation').click();
      const tabs = page.locator('[data-testid=ribbon] [role=tab]');
      await expect(tabs.first()).toBeVisible();
      const names = (await tabs.allTextContents()).map((name) => name.trim()).filter(Boolean);
      for (const name of names) {
        await tabs.filter({ hasText: name }).first().click();
        await expect
          .poll(
            () =>
              page.evaluate(() =>
                [...document.querySelectorAll<HTMLElement>('[data-group] [class*="groupLabel"]')]
                  .filter(
                    (el) => el.getClientRects().length > 0 && el.scrollWidth > el.clientWidth + 1,
                  )
                  .map((el) => el.textContent?.trim()),
              ),
            { message: `${app}, ${width}px, ${name}` },
          )
          .toEqual([]);
      }
    }
  }
});
