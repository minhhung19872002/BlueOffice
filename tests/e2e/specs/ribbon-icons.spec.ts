import { expect, test } from '@playwright/test';

// UI-ICONS-002: every command button of the Home tab shows an icon from the BlueOffice set
// (`<Icon>`, `svg.bo-icon`), in all three apps. Custom controls (font box, size box, colour
// pickers) carry their own rendering and are not ribbon buttons.

for (const app of ['doc', 'sheet', 'slide'] as const) {
  test(`Home tab of ${app}: no command button without an icon`, async ({ page }) => {
    await page.goto(`/${app}`);
    const home = page.getByRole('tab', { name: 'Trang đầu', exact: true });
    await expect(home).toBeVisible();
    await home.click();
    const panel = page.getByRole('tabpanel').first();
    const buttons = panel.locator('button[data-command]:not([data-launcher])');
    await expect(buttons.first()).toBeVisible();
    const missing = await buttons.evaluateAll((els) =>
      els
        .filter((el) => !el.querySelector('svg.bo-icon'))
        .map((el) => el.getAttribute('data-command') ?? '?'),
    );
    expect(missing, 'command ids without an icon').toEqual([]);
  });
}
