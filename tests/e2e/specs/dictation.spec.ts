import { expect, test, type Page } from '@playwright/test';

// CORE-LANG-002: dictation through the browser's recogniser — replaced here by a scripted
// `SpeechRecognition` (no microphone, no network); the server has no dictation configured.
const cmd = (page: Page, id: string) => page.locator(`[data-command="${id}"]`);

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

test('BlueDoc: interim preview at the caret, voice commands, one undo step per phrase', async ({
  page,
}) => {
  const errors: string[] = [];
  page.on('pageerror', (e) => errors.push(e.message));
  await fakeRecognizer(page);
  await page.goto('/doc');
  await page.getByTestId('new-document').click();
  await expect(page.getByTestId('caret')).toBeVisible();
  await page.getByRole('tab', { name: 'Trang đầu' }).click();
  await cmd(page, 'edit.dictate').click();
  await expect(page.getByTestId('dictation-status')).toHaveText('Đang nghe…');

  // Interim text: only drawn at the caret, not in the document.
  await page.evaluate(() => window.__say('xin chào', false));
  await expect(page.getByTestId('dictation-preview')).toHaveText('Xin chào');
  await expect(page.getByTestId('dictation-interim')).toHaveText('Xin chào');
  await expect(page.getByTestId('status-words')).toHaveText('0 từ');

  await page.evaluate(() => window.__say('xin chào chấm', true));
  await expect(page.getByTestId('status-words')).toHaveText('2 từ');
  await expect(page.getByTestId('dictation-preview')).toHaveCount(0);
  await page.evaluate(() => window.__say('hôm nay trời đẹp phẩy đi chơi thôi chấm than', true));
  await expect(page.getByTestId('status-words')).toHaveText('9 từ');
  await page.evaluate(() => window.__say('ba bốn xoá từ vừa rồi', true));
  await expect(page.getByTestId('status-words')).toHaveText('10 từ');

  // Stop, then undo: one phrase at a time.
  await cmd(page, 'edit.dictate').click();
  await expect(page.getByTestId('dictation-bar')).toHaveCount(0);
  await cmd(page, 'edit.undo').click();
  await expect(page.getByTestId('status-words')).toHaveText('9 từ');
  await cmd(page, 'edit.undo').click();
  await expect(page.getByTestId('status-words')).toHaveText('2 từ');
  expect(errors).toEqual([]);
});

test('BlueDoc: says when the browser has no recogniser', async ({ page }) => {
  await page.route('**/api/dictation/languages**', (route) =>
    route.fulfill({ status: 404, json: { code: 'unavailable' } }),
  );
  await page.addInitScript(() => {
    const w = window as unknown as Record<string, unknown>;
    delete w.SpeechRecognition;
    delete w.webkitSpeechRecognition;
  });
  await page.goto('/doc');
  await page.getByTestId('new-document').click();
  await expect(page.getByTestId('caret')).toBeVisible();
  await page.getByRole('tab', { name: 'Trang đầu' }).click();
  await cmd(page, 'edit.dictate').click();
  await expect(page.getByTestId('dictation-status')).toContainText('chưa dùng được');
});

test('BlueSheet: dictates into the active cell editor', async ({ page }) => {
  await fakeRecognizer(page);
  await page.goto('/sheet');
  await expect(page.getByRole('tab', { name: 'Sheet1' })).toBeVisible();
  await page.getByRole('tab', { name: 'Trang đầu' }).click();
  await cmd(page, 'edit.dictate').click();
  await expect(page.getByTestId('dictation-status')).toHaveText('Đang nghe…');
  await page.evaluate(() => window.__say('tổng cộng hai chấm một trăm', true));
  await expect(page.getByLabel('Thanh công thức')).toHaveValue('Tổng cộng: một trăm');
  await cmd(page, 'edit.dictate').click();
  await page.getByLabel('Ô đang nhập').press('Enter');
  const box = page.getByLabel('Hộp tên');
  await box.click();
  await box.fill('A1');
  await box.press('Enter');
  await expect(page.getByLabel('Thanh công thức')).toHaveValue('Tổng cộng: một trăm');
});
