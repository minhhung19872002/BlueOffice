/**
 * BlueOffice-PROMPT §5 performance targets measured in the real web build (Chromium,
 * WASM engine in its Web Worker, canvas renderer):
 *  - "Mở .docx 300 trang có ảnh < 5 s (web)": from choosing the file to the first page
 *    painted (pictures decoded by the browser);
 *  - "Cuộn 60 fps": frame times while scrolling through the 300 pages.
 * The benchmark document comes from `bo-cli sample-big` (300 A4 pages, one 640×480
 * picture per page). Numbers go to the console and tests/fidelity/output/perf-web.json.
 * Headless CI runners are slower and noisier than a desktop browser, so the hard §5
 * limits are asserted only with BO_PERF_STRICT=1; otherwise the test guards against
 * gross regressions (10× the target).
 */
import { expect, test } from '@playwright/test';
import { execFileSync } from 'node:child_process';
import { existsSync, mkdirSync, statSync, writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const here = dirname(fileURLToPath(import.meta.url));
const root = join(here, '..', '..', '..');
const outDir = join(root, 'tests', 'fidelity', 'output');
const strict = process.env.BO_PERF_STRICT === '1';
const exe = process.platform === 'win32' ? 'bo-cli.exe' : 'bo-cli';

/** The most recently built bo-cli (BO_CLI, CARGO_TARGET_DIR or ./target). */
function cli(): string | null {
  if (process.env.BO_CLI && existsSync(process.env.BO_CLI)) return process.env.BO_CLI;
  const dirs = [process.env.CARGO_TARGET_DIR, join(root, 'target')].filter((d): d is string => !!d);
  const built = dirs
    .flatMap((d) => ['release', 'debug'].map((p) => join(d, p, exe)))
    .filter((f) => existsSync(f))
    .sort((a, b) => statSync(b).mtimeMs - statSync(a).mtimeMs);
  return built[0] ?? null;
}

const results: Record<string, unknown> = {};

test.describe.configure({ mode: 'serial' });

test('opens a 300-page .docx with pictures and scrolls through it', async ({ page }) => {
  test.setTimeout(240_000);
  const bin = cli();
  test.skip(!bin, 'bo-cli is not built (cargo build -p bo-cli)');
  mkdirSync(outDir, { recursive: true });
  const file = join(outDir, 'perf-big-300.docx');
  execFileSync(bin!, ['sample-big', file, '--pages', '300'], { stdio: 'ignore' });

  await page.goto('/doc');
  await expect(page.getByTestId('status-engine')).toHaveText('Lõi: WASM');
  const chooser = page.waitForEvent('filechooser');
  await page.getByRole('button', { name: 'Mở tệp từ máy' }).click();
  const t0 = Date.now();
  await (await chooser).setFiles(file);
  const firstPage = page.locator('.bo-page[data-page="1"]');
  await expect(firstPage).toHaveAttribute('data-rendered', 'true', { timeout: 120_000 });
  const openMs = Date.now() - t0;
  await expect(page.getByTestId('status-page')).toHaveText(/^Trang 1\/300$/);
  results.openMs = openMs;
  results.fileMB = statSync(file).size / (1024 * 1024);

  // Scroll at ~2 400 px/s for 4 s with requestAnimationFrame and record frame times.
  const scroll = await page.evaluate(async () => {
    const scroller = document.querySelector<HTMLElement>('.bo-doc-scroller')!;
    const deltas: number[] = [];
    await new Promise<void>((resolve) => {
      let last = performance.now();
      const start = last;
      const step = (now: number) => {
        deltas.push(now - last);
        last = now;
        scroller.scrollTop += 40;
        if (now - start < 4000) requestAnimationFrame(step);
        else resolve();
      };
      requestAnimationFrame(step);
    });
    deltas.shift();
    const sorted = [...deltas].sort((a, b) => a - b);
    const total = deltas.reduce((s, d) => s + d, 0);
    return {
      frames: deltas.length,
      fps: (deltas.length * 1000) / total,
      p50Ms: sorted[Math.floor(sorted.length * 0.5)]!,
      p95Ms: sorted[Math.floor(sorted.length * 0.95)]!,
      maxMs: sorted[sorted.length - 1]!,
      over33Ms: deltas.filter((d) => d > 33.4).length,
      scrolledPx: scroller.scrollTop,
    };
  });
  results.scroll = scroll;
  // Pages under the viewport were really painted after the scroll.
  await expect
    .poll(async () =>
      page.locator('.bo-page[data-rendered="true"]').evaluateAll((els) =>
        els.some((e) => {
          const r = e.getBoundingClientRect();
          return (
            r.bottom > 0 && r.top < window.innerHeight && Number(e.getAttribute('data-page')) > 5
          );
        }),
      ),
    )
    .toBe(true);

  writeFileSync(join(outDir, 'perf-web.json'), JSON.stringify(results, null, 2));
  console.log('perf-web', JSON.stringify(results));

  expect(openMs).toBeLessThan(strict ? 5_000 : 50_000);
  expect(scroll.fps).toBeGreaterThan(strict ? 55 : 6);
});
