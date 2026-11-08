import { expect, test, type Page } from '@playwright/test';

// Responsive ribbon: at every common window width, on every tab of the three apps,
// ribbon controls never overlap, stay inside the panel and nothing scrolls
// horizontally (groups collapse instead — packages/ui/src/ribbonLayout.ts).
const widths = [1600, 1280, 1024, 800];
const apps = [
  {
    path: '/doc',
    ready: async (p: Page) => {
      await expect(p.getByTestId('status-engine')).toHaveText('Lõi: WASM');
      await p.getByTestId('open-sample').click();
      await expect(p.getByTestId('status-page')).toHaveText(/^Trang 1\/\d+$/);
    },
  },
  { path: '/sheet', ready: (p: Page) => expect(p.getByTestId('sheet-canvas')).toBeVisible() },
  { path: '/slide', ready: (p: Page) => expect(p.getByTestId('ribbon')).toBeVisible() },
];

interface Box {
  name: string;
  left: number;
  top: number;
  right: number;
  bottom: number;
}

/** Layout facts of the active ribbon panel, read in the page. */
async function measure(page: Page) {
  return page.getByTestId('ribbon').evaluate((ribbon) => {
    const panel = ribbon.querySelector<HTMLElement>('[role="tabpanel"]:not([hidden])');
    const tabStrip = ribbon.querySelector<HTMLElement>('[role="tablist"]');
    const rect = (el: Element, name: string): Box => {
      const r = el.getBoundingClientRect();
      return { name, left: r.left, top: r.top, right: r.right, bottom: r.bottom };
    };
    // Top-level controls of each group (buttons, custom controls, collapsed
    // triggers) and the overflow button.
    const controls = panel
      ? Array.from(
          panel.querySelectorAll(
            ':scope > [data-group] [data-command], :scope > [data-group] [data-control], :scope > [data-group] > button, :scope > [data-overflow] button',
          ),
        )
          .filter((el) => !el.closest('[data-control]') || el.hasAttribute('data-control'))
          .filter((el) => {
            const r = el.getBoundingClientRect();
            return r.width > 0 && r.height > 0;
          })
          .map((el) =>
            rect(
              el,
              el.getAttribute('data-command') ??
                el.getAttribute('data-control') ??
                el.getAttribute('aria-label') ??
                el.tagName,
            ),
          )
      : [];
    const groups = panel
      ? Array.from(panel.querySelectorAll(':scope > [data-group]')).map((g) =>
          rect(g, `${g.getAttribute('data-group')}@${g.getAttribute('data-level')}`),
        )
      : [];
    return {
      panel: panel ? rect(panel, 'panel') : null,
      panelOverflow: panel ? panel.scrollWidth - panel.clientWidth : 0,
      tabsOverflow: tabStrip ? tabStrip.scrollWidth - tabStrip.clientWidth : 0,
      pageOverflow: document.documentElement.scrollWidth - window.innerWidth,
      controls,
      groups,
    };
  });
}

function overlaps(a: Box, b: Box, tolerance = 0.5) {
  return (
    a.left < b.right - tolerance &&
    b.left < a.right - tolerance &&
    a.top < b.bottom - tolerance &&
    b.top < a.bottom - tolerance
  );
}

for (const app of apps) {
  for (const width of widths) {
    test(`ribbon fits without overlap: ${app.path} at ${width} px`, async ({ page }, info) => {
      await page.setViewportSize({ width, height: 800 });
      await page.goto(app.path);
      await app.ready(page);
      const ribbon = page.getByTestId('ribbon');
      const tabs = ribbon.getByRole('tab');
      const count = await tabs.count();
      expect(count).toBeGreaterThan(1);
      for (let t = 0; t < count; t += 1) {
        const tab = tabs.nth(t);
        const name = (await tab.textContent()) ?? `tab${t}`;
        await tab.click();
        // The collapse planner settles within a few frames after layout.
        await expect
          .poll(async () => (await measure(page)).panelOverflow, { timeout: 5_000 })
          .toBeLessThanOrEqual(0);
        const m = await measure(page);
        expect(m.pageOverflow, `${name}: page scrolls horizontally`).toBeLessThanOrEqual(0);
        expect(m.tabsOverflow, `${name}: tab strip overflows`).toBeLessThanOrEqual(0);
        const panel = m.panel;
        expect(panel, `${name}: no active panel`).not.toBeNull();
        if (!panel) continue;
        for (const c of m.controls) {
          expect(c.right, `${name}: ${c.name} spills out of the ribbon`).toBeLessThanOrEqual(
            panel.right + 0.5,
          );
          expect(c.left, `${name}: ${c.name} spills out of the ribbon`).toBeGreaterThanOrEqual(
            panel.left - 0.5,
          );
        }
        for (let i = 0; i < m.controls.length; i += 1) {
          for (let j = i + 1; j < m.controls.length; j += 1) {
            const a = m.controls[i];
            const b = m.controls[j];
            if (a && b) {
              expect(overlaps(a, b), `${name}: ${a.name} overlaps ${b.name}`).toBe(false);
            }
          }
        }
        for (let i = 1; i < m.groups.length; i += 1) {
          const a = m.groups[i - 1];
          const b = m.groups[i];
          if (a && b) expect(overlaps(a, b), `${name}: ${a.name} overlaps ${b.name}`).toBe(false);
        }
        await info.attach(`${app.path.slice(1)}-${width}-${t}-${name}.png`, {
          body: await ribbon.screenshot(),
          contentType: 'image/png',
        });
      }
    });
  }
}

test('a collapsed ribbon group still runs its commands', async ({ page }) => {
  await page.setViewportSize({ width: 800, height: 800 });
  await page.goto('/doc');
  await apps[0]?.ready(page);
  const ribbon = page.getByTestId('ribbon');
  // At 800 px the Home tab cannot show every group expanded.
  const collapsed = ribbon.locator('[data-group][data-level="collapsed"]');
  const more = ribbon.getByTestId('ribbon-more');
  await expect
    .poll(async () => (await collapsed.count()) + (await more.count()), { timeout: 5_000 })
    .toBeGreaterThan(0);
  const trigger = (await collapsed.count()) ? collapsed.first().getByRole('button').first() : more;
  await trigger.click();
  const panel = page.locator('[data-bo-popover]');
  await expect(panel).toBeVisible();
  await expect(panel.locator('[data-command], [data-control]').first()).toBeVisible();
  await page.keyboard.press('Escape');
  await expect(panel).toHaveCount(0);
});
