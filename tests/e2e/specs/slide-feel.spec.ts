import { expect, test, type Page } from '@playwright/test';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

// Look and feel of the BlueSlide editing surface against the conventions of desktop
// presentation programs (black-box knowledge of the program the project owner compares with):
//  * a click on a shape selects it on mouse-DOWN: the selection frame is drawn in the very next
//    frame, before the engine has answered;
//  * the frame of a selected shape is solid, with round sizing handles and a rotation handle
//    above the top edge; while its text is edited the frame is dashed and has no handles;
//  * dragging shows a live outline of the shape under the pointer (not a lagging copy);
//  * the slide sits centred on a grey canvas with a shadow;
//  * the thumbnail of the current slide is highlighted, a dragged thumbnail shows a line where
//    it will land;
//  * the status bar holds "Slide n of m", notes, the views, a zoom slider and "fit".

const here = dirname(fileURLToPath(import.meta.url));
const deck = join(here, '..', '..', 'corpus', 'slide', 'slide-text-bullets.pptx');

async function openDeck(page: Page) {
  await page.goto('/slide');
  const chooser = page.waitForEvent('filechooser');
  await page.getByRole('button', { name: 'Mở', exact: true }).click();
  await (await chooser).setFiles(deck);
  await expect(page.getByTestId('slide-status')).toHaveText(/^Slide 1 \/ \d+$/);
  await expect(page.getByTestId('slide-overlay')).toBeVisible();
}

interface Probe {
  /** Selection frames right after the mousedown was handled (before any frame). */
  sync: number;
  frames: { t: number; n: number }[];
}

/** On the next pointerdown: the number of selection frames right away and in every frame. */
async function armProbe(page: Page) {
  await page.evaluate(`(() => {
    const n = () => document.querySelectorAll('[data-testid="selection-box"]').length;
    const p = { t0: 0, sync: -1, frames: [] };
    window.__slideProbe = p;
    window.addEventListener('pointerdown', () => {
      p.t0 = performance.now();
      const tick = (t) => {
        p.frames.push({ t: t - p.t0, n: n() });
        if (p.frames.length < 30) requestAnimationFrame(tick);
      };
      requestAnimationFrame(tick);
    }, { capture: true, once: true });
    window.addEventListener('pointerdown', () => {
      // React renders a discrete event's update synchronously, before the browser paints.
      queueMicrotask(() => { p.sync = n(); });
    }, { once: true });
  })()`);
}

async function probeResult(page: Page): Promise<Probe> {
  await page.waitForFunction(
    () => (window as unknown as { __slideProbe: Probe }).__slideProbe.frames.length >= 30,
  );
  return page.evaluate(() => (window as unknown as { __slideProbe: Probe }).__slideProbe);
}

/** Milliseconds from the pointerdown to the first painted frame showing a selection frame. */
const firstSelected = (p: Probe) => p.frames.find((f) => f.n > 0)?.t ?? Infinity;

async function overlay(page: Page) {
  return (await page.getByTestId('slide-overlay').boundingBox())!;
}

test.describe('Selecting and dragging', () => {
  test('a click selects the shape on mouse-down, in the first frame', async ({ page }) => {
    await openDeck(page);
    const o = await overlay(page);
    const times: number[] = [];
    const syncs: number[] = [];
    for (let i = 0; i < 3; i++) {
      await page.keyboard.press('Escape');
      await page.mouse.click(o.x + 4, o.y + o.height - 4);
      await expect(page.getByTestId('selection-box')).toHaveCount(0);
      await armProbe(page);
      // The title of slide 1 (108,81 720×162 on the 960×540 slide).
      await page.mouse.move(o.x + o.width * 0.48, o.y + o.height * 0.3);
      await page.mouse.down();
      const probe = await probeResult(page);
      await page.mouse.up();
      syncs.push(probe.sync);
      times.push(firstSelected(probe));
    }
    console.log(`slide select → first frame (ms): ${times.map((t) => t.toFixed(1)).join(', ')}`);
    expect(syncs, 'drawn while handling the mouse-down').toEqual([1, 1, 1]);
    // The first frame after the event (one frame at 60 Hz plus scheduling slack).
    expect(Math.min(...times)).toBeLessThan(34);
  });

  test('a drag shows the outline under the pointer within a frame or two', async ({ page }) => {
    await openDeck(page);
    const o = await overlay(page);
    const x = o.x + o.width * 0.48;
    const y = o.y + o.height * 0.3;
    await page.mouse.click(x, y);
    await expect(page.getByTestId('selection-box')).toHaveCount(1);
    const times: number[] = [];
    for (let i = 0; i < 3; i++) {
      await page.mouse.move(x, y);
      await page.mouse.down();
      // The drag starts; the outline appears.
      await page.mouse.move(x + 6, y + 2, { steps: 2 });
      await expect(page.getByTestId('drag-outline')).toHaveCount(1);
      // From the next pointer move to the first frame where the outline has moved with it.
      await page.evaluate(`(() => {
        const at = () => document.querySelector('[data-testid="drag-outline"]')?.getAttribute('x');
        const before = at();
        window.__dragOutline = new Promise((done) => {
          window.addEventListener('pointermove', () => {
            const t0 = performance.now();
            const tick = () => {
              if (at() !== before) done(performance.now() - t0);
              else if (performance.now() - t0 > 2000) done(Infinity);
              else requestAnimationFrame(tick);
            };
            requestAnimationFrame(tick);
          }, { once: true });
        });
      })()`);
      await page.mouse.move(x + 60, y + 12);
      times.push(
        await page.evaluate(
          () => (window as unknown as { __dragOutline: Promise<number> }).__dragOutline,
        ),
      );
      await page.mouse.move(x, y);
      await page.mouse.up();
      await page.keyboard.press('Control+z');
    }
    console.log(`slide drag → outline (ms): ${times.map((t) => t.toFixed(1)).join(', ')}`);
    // The dashed outline of the shape follows the pointer (engine preview with smart guides).
    expect(Math.min(...times)).toBeLessThan(50);
  });
});

test.describe('Selection frame and text editing', () => {
  test('solid frame with round handles and a rotation handle; dashed while editing text', async ({
    page,
  }) => {
    await openDeck(page);
    const o = await overlay(page);
    await page.mouse.click(o.x + o.width * 0.48, o.y + o.height * 0.3);
    const frame = page.getByTestId('selection-box');
    await expect(frame).toHaveCount(1);
    expect(await frame.evaluate((e) => getComputedStyle(e).strokeDasharray)).toBe('none');
    // The eight sizing handles are round.
    for (const k of ['nw', 'n', 'ne', 'e', 'se', 's', 'sw', 'w']) {
      expect(await page.getByTestId(`handle-${k}`).evaluate((e) => e.tagName)).toBe('circle');
    }
    // The rotation handle sits above the middle of the top edge.
    const top = (await frame.boundingBox())!;
    const rot = (await page.getByTestId('handle-rot').boundingBox())!;
    expect(rot.y + rot.height).toBeLessThan(top.y);
    expect(Math.abs(rot.x + rot.width / 2 - (top.x + top.width / 2))).toBeLessThan(2);
    // Editing the text: a dashed frame, no handles.
    await page.keyboard.press('F2');
    const editing = page.getByTestId('text-editing-frame');
    await expect(editing).toHaveCount(1);
    expect(await editing.evaluate((e) => getComputedStyle(e).strokeDasharray)).not.toBe('none');
    await expect(page.getByTestId('handle-nw')).toHaveCount(0);
  });

  test('a double-click in the text puts the caret where the pointer is', async ({ page }) => {
    await openDeck(page);
    const o = await overlay(page);
    const s = o.width / 960;
    // Slide 2's title, between two letters near its middle.
    await page.getByTestId('thumb-2').click();
    const box = page.getByTestId('slide-a11y-mirror');
    await expect(box).toBeAttached();
    const at = { x: o.x + 300 * s, y: o.y + 60 * s };
    await page.mouse.click(at.x, at.y);
    await expect(page.getByTestId('selection-box')).toHaveCount(1);
    await page.mouse.dblclick(at.x, at.y);
    await expect(page.getByTestId('slide-text-editor')).toBeFocused();
    const caret = page.getByTestId('text-caret');
    await expect(caret).toHaveCount(1);
    // The double-click selected the word under the pointer: one more click places the caret.
    await page.mouse.click(at.x, at.y);
    await expect
      .poll(async () => {
        const c = await caret.boundingBox();
        return c ? Math.abs(c.x - at.x) : Infinity;
      })
      .toBeLessThan(20 * s + 8);
  });
});

test.describe('Window', () => {
  test('the slide is centred on a grey canvas with a shadow', async ({ page }) => {
    await openDeck(page);
    const stage = page.getByTestId('slide-stage');
    const st = (await stage.boundingBox())!;
    const sl = (await page.getByTestId('slide-overlay').boundingBox())!;
    expect(Math.abs(sl.x + sl.width / 2 - (st.x + st.width / 2))).toBeLessThan(2);
    expect(Math.abs(sl.y + sl.height / 2 - (st.y + st.height / 2))).toBeLessThan(2);
    // The canvas behind the slide is grey, not white.
    const bg = await stage.evaluate((e) => {
      let el: HTMLElement | null = e;
      while (el) {
        const c = getComputedStyle(el).backgroundColor;
        if (c && c !== 'rgba(0, 0, 0, 0)' && c !== 'transparent') return c;
        el = el.parentElement;
      }
      return '';
    });
    const [r, g, b] = (bg.match(/\d+/g) ?? []).map(Number);
    expect(r! + g! + b!).toBeLessThan(255 * 3 - 15);
    expect(Math.max(r!, g!, b!) - Math.min(r!, g!, b!)).toBeLessThan(24);
    const shadow = await page
      .getByTestId('slide-stage')
      .locator('canvas')
      .first()
      .evaluate((e) => getComputedStyle(e).boxShadow);
    expect(shadow).not.toBe('none');
  });

  test('slide panel: the current slide highlighted, a line where a dragged slide lands', async ({
    page,
  }) => {
    await openDeck(page);
    await page.getByTestId('thumb-2').click();
    const border = (n: number) =>
      page.getByTestId(`thumb-${n}`).evaluate((e) => getComputedStyle(e).borderTopColor);
    expect(await border(2)).not.toBe(await border(1));
    expect(await border(1)).toBe('rgba(0, 0, 0, 0)');
    // While a slide is dragged over another, that one shows the drop line.
    const dt = await page.evaluateHandle(() => new DataTransfer());
    await page.getByTestId('thumb-1').dispatchEvent('dragstart', { dataTransfer: dt });
    await page.getByTestId('thumb-3').dispatchEvent('dragover', { dataTransfer: dt });
    await expect
      .poll(() => page.getByTestId('thumb-3').evaluate((e) => getComputedStyle(e).boxShadow))
      .toContain('inset');
    await page.getByTestId('thumb-1').dispatchEvent('dragend', { dataTransfer: dt });
    await expect
      .poll(() => page.getByTestId('thumb-3').evaluate((e) => getComputedStyle(e).boxShadow))
      .not.toContain('inset');
  });

  test('status bar: slide n of m, notes, views, zoom slider and fit', async ({ page }) => {
    await openDeck(page);
    await expect(page.getByTestId('slide-status')).toHaveText('Slide 1 / 3');
    await expect(page.getByTestId('status-button-notes')).toBeVisible();
    for (const v of ['normal', 'sorter', 'reading']) {
      await expect(page.getByTestId(`status-button-${v}`)).toBeVisible();
    }
    await expect(page.getByTestId('status-zoom-slider')).toBeVisible();
    await expect(page.getByTestId('status-button-fit')).toBeVisible();
    // Left to right: the slide number on the left, the zoom on the right.
    const left = (await page.getByTestId('slide-status').boundingBox())!;
    const zoom = (await page.getByTestId('status-zoom-slider').boundingBox())!;
    expect(zoom.x).toBeGreaterThan(left.x);
  });

  test('Home tab groups: undo, clipboard, slides, font, paragraph, drawing, editing', async ({
    page,
  }) => {
    await openDeck(page);
    await page.getByRole('tab', { name: 'Trang đầu', exact: true }).click();
    const ids = await page
      .locator('[role="tabpanel"] [data-group]')
      .evaluateAll((els) => [...new Set(els.map((e) => e.getAttribute('data-group')))]);
    const want = ['history', 'clipboard', 'slides', 'font', 'paragraph', 'drawing', 'find'];
    expect(ids.filter((id) => want.includes(id ?? ''))).toEqual(want);
    // Format painter in the clipboard group; the font dialog from the group's corner button.
    await expect(
      page.locator('[data-group="clipboard"] [data-command="format.painter"]').first(),
    ).toBeAttached();
    await expect(
      page.locator('[data-group="font"] [data-command="format.fontDialog"]').first(),
    ).toBeAttached();
  });
});
