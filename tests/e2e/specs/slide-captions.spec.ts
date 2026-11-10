import { expect, test, type Page } from '@playwright/test';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

// SLIDE-REMOTE-001: live subtitles in the slide show from the dictation provider — here the
// browser's recogniser, replaced by a scripted `SpeechRecognition` (no microphone, no network);
// the server has no dictation configured. Without a server, the phone remote and the online
// presentation say they are not available.
const here = dirname(fileURLToPath(import.meta.url));
const bullets = join(here, '..', '..', 'corpus', 'slide', 'slide-text-bullets.pptx');

declare global {
  interface Window {
    __say(text: string, final: boolean): void;
    __recognitions: number;
  }
}

async function fakeRecognizer(page: Page) {
  await page.route('**/api/dictation/languages**', (route) =>
    route.fulfill({ status: 404, json: { code: 'unavailable' } }),
  );
  await page.route('**/api/present/config', (route) =>
    route.fulfill({ status: 404, json: { code: 'unavailable' } }),
  );
  await page.addInitScript(() => {
    type Res = { isFinal: boolean; length: number; 0: { transcript: string; confidence: number } };
    const live: { current: FakeRecognition | null } = { current: null };
    window.__recognitions = 0;
    class FakeRecognition {
      lang = '';
      continuous = false;
      interimResults = false;
      maxAlternatives = 1;
      onstart: (() => void) | null = null;
      onresult: ((e: { resultIndex: number; results: Res[] }) => void) | null = null;
      onerror: ((e: { error: string }) => void) | null = null;
      onend: (() => void) | null = null;
      results: Res[] = [];
      start() {
        live.current = this;
        window.__recognitions++;
        setTimeout(() => this.onstart?.(), 0);
      }
      stop() {
        setTimeout(() => this.onend?.(), 0);
      }
      abort() {
        live.current = null;
      }
    }
    window.__say = (text, final) => {
      const r = live.current;
      if (!r) return;
      const last = r.results.at(-1);
      const i = last && !last.isFinal ? r.results.length - 1 : r.results.length;
      r.results[i] = { isFinal: final, length: 1, 0: { transcript: text, confidence: 0.9 } };
      r.onresult?.({ resultIndex: i, results: r.results });
    };
    (window as unknown as { SpeechRecognition: unknown }).SpeechRecognition = FakeRecognition;
  });
}

const cmd = (page: Page, id: string) => page.locator(`[data-command="${id}"]`).first();

test('live subtitles show the speech over the slide show, where and how big they were set', async ({
  page,
}) => {
  const errors: string[] = [];
  page.on('pageerror', (e) => errors.push(e.message));
  await fakeRecognizer(page);
  await page.goto('/slide');
  await page.getByRole('tab', { name: 'Trang đầu' }).click();
  const chooser = page.waitForEvent('filechooser');
  await page.getByRole('button', { name: 'Mở', exact: true }).click();
  await (await chooser).setFiles(bullets);
  await expect(page.getByTestId('slide-status')).toHaveText(/^Slide 1 \/ \d+$/);

  await page.getByRole('tab', { name: 'Trình chiếu' }).click();
  // No server here: the remote and the online presentation explain why they are off.
  await cmd(page, 'show.remote').click();
  await expect(page.getByTestId('remote-unavailable')).toBeVisible();
  await page.getByTestId('remote-close').click();
  await cmd(page, 'show.online').click();
  await expect(page.getByTestId('online-unavailable')).toBeVisible();
  await expect(page.getByTestId('online-start')).toHaveCount(0);
  await page.getByTestId('online-close').click();

  await cmd(page, 'show.captionSettings').click();
  await expect(page.getByTestId('captions-dialog')).toBeVisible();
  await expect(page.getByTestId('captions-unavailable')).toHaveCount(0);
  await page.getByTestId('captions-on').check();
  await page.getByTestId('captions-position').selectOption('top');
  await page.getByTestId('captions-size').selectOption('large');
  await page.getByTestId('captions-apply').click();
  await expect(cmd(page, 'show.captions')).toHaveAttribute('aria-pressed', 'true');

  // Nothing is heard before the show starts.
  expect(await page.evaluate(() => window.__recognitions)).toBe(0);
  await cmd(page, 'show.fromStart').click();
  await expect(page.getByTestId('show-stage')).toHaveAttribute('data-slide', '0');
  await expect.poll(() => page.evaluate(() => window.__recognitions)).toBe(1);
  await page.evaluate(() => window.__say('xin chào', false));
  const captions = page.getByTestId('show-captions');
  await expect(page.getByTestId('captions-interim')).toHaveText('xin chào');
  await page.evaluate(() => window.__say('Xin chào các bạn', true));
  await expect(page.getByTestId('captions-text')).toHaveText('Xin chào các bạn');
  await expect(captions).toHaveAttribute('data-position', 'top');
  await expect(captions).toHaveAttribute('data-size', 'large');
  // At the top of the screen.
  const box = (await captions.boundingBox())!;
  expect(box.y).toBeLessThan(900 / 4);

  // Moving on keeps the subtitles; ending the show stops listening and hides them.
  await page.keyboard.press('ArrowRight');
  await expect(page.getByTestId('captions-text')).toHaveText('Xin chào các bạn');
  await page.keyboard.press('Escape');
  await expect(page.getByTestId('show-stage')).toHaveCount(0);
  await expect(captions).toHaveCount(0);

  // Turned off from the ribbon: the next show has none.
  await cmd(page, 'show.captions').click();
  await expect(cmd(page, 'show.captions')).toHaveAttribute('aria-pressed', 'false');
  await cmd(page, 'show.fromStart').click();
  await expect(page.getByTestId('show-stage')).toHaveAttribute('data-slide', '0');
  await page.evaluate(() => window.__say('không hiện', true));
  await expect(page.getByTestId('show-captions')).toHaveCount(0);
  expect(await page.evaluate(() => window.__recognitions)).toBe(1);
  expect(errors).toEqual([]);
});
