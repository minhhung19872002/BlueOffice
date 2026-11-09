import { expect, test, type Page } from '@playwright/test';

const tabTransition = (page: Page) =>
  page
    .getByRole('tab', { name: 'Chèn', exact: true })
    .evaluate((el) => getComputedStyle(el).transitionDuration);

test('motion uses the 120–200 ms tokens', async ({ page }) => {
  await page.emulateMedia({ reducedMotion: 'no-preference' });
  await page.goto('/doc');
  expect(await tabTransition(page)).toBe('0.12s, 0.12s');
});

test('prefers-reduced-motion turns UI motion off', async ({ page }) => {
  await page.emulateMedia({ reducedMotion: 'reduce' });
  await page.goto('/doc');
  expect(await tabTransition(page)).toBe('0s, 0s');
});
