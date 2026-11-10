import { expect, test, type Page } from '@playwright/test';

// CORE-LANG-004: "Trình đọc tập trung" — the document as reflowed, read-only text with text
// size, spacing, column width, page colour, line focus, syllable marks, focus and read aloud.
// Speech is a fake `speechSynthesis` (no audio device): it reports each word, 120 ms apart.

const cmd = (page: Page, id: string) => page.locator(`[data-command="${id}"]`);

const VI_TEXT =
  'Trình đọc tập trung trình bày lại văn bản thành một cột chữ dễ đọc, không còn thanh công cụ ' +
  'soạn thảo, để người đọc chỉ nhìn vào nội dung. Người đọc tự chọn cỡ chữ, khoảng cách giữa ' +
  'các chữ và các dòng, độ rộng của cột và màu trang phù hợp với mắt mình. Khi bật tập trung ' +
  'dòng, chỉ vài dòng đang đọc sáng rõ còn phần còn lại mờ đi.';
const EN_TEXT = 'The children ask an important question about a wonderful table of information.';

async function fakeSpeech(page: Page) {
  await page.addInitScript(() => {
    type Boundary = { name: string; charIndex: number; charLength: number };
    class Utterance {
      lang = '';
      voice: unknown = null;
      rate = 1;
      pitch = 1;
      volume = 1;
      onstart: ((e: unknown) => void) | null = null;
      onend: ((e: unknown) => void) | null = null;
      onerror: ((e: unknown) => void) | null = null;
      onboundary: ((e: Boundary) => void) | null = null;
      onpause: ((e: unknown) => void) | null = null;
      onresume: ((e: unknown) => void) | null = null;
      constructor(public text: string) {}
    }
    const spoken: { text: string; lang: string }[] = [];
    let timer = 0;
    const synth = {
      speak(u: Utterance) {
        spoken.push({ text: u.text, lang: u.lang });
        const words = [...u.text.matchAll(/\S+/g)];
        let i = 0;
        u.onstart?.({});
        const tick = () => {
          const w = words[i++];
          if (!w) {
            u.onend?.({});
            return;
          }
          u.onboundary?.({ name: 'word', charIndex: w.index, charLength: w[0].length });
          timer = window.setTimeout(tick, 120);
        };
        timer = window.setTimeout(tick, 30);
      },
      cancel: () => window.clearTimeout(timer),
      pause: () => {},
      resume: () => {},
      getVoices: () => [
        { voiceURI: 'vi', name: 'An', lang: 'vi-VN', localService: true, default: true },
        { voiceURI: 'en', name: 'Sam', lang: 'en-US', localService: true, default: false },
      ],
      addEventListener: () => {},
      removeEventListener: () => {},
    };
    Object.defineProperty(window, 'speechSynthesis', { value: synth, configurable: true });
    Object.defineProperty(window, 'SpeechSynthesisUtterance', {
      value: Utterance,
      configurable: true,
    });
    (window as unknown as { __spoken: typeof spoken }).__spoken = spoken;
  });
}

/** A heading, a long Vietnamese paragraph and an English one; the caret back at the start. */
async function sampleDocument(page: Page) {
  await page.goto('/doc');
  await page.getByTestId('new-document').click();
  await expect(page.getByTestId('caret')).toBeVisible();
  await page.keyboard.insertText('Giới thiệu');
  await page.keyboard.press('Control+Alt+1');
  await page.keyboard.press('Enter');
  await page.keyboard.insertText(VI_TEXT);
  await page.keyboard.press('Enter');
  await page.keyboard.insertText(EN_TEXT);
  await page.keyboard.press('Control+Home');
  await expect(page.locator('.bo-page[data-page="1"]')).toHaveAttribute('data-rendered', 'true');
}

async function openReader(page: Page) {
  await page.getByRole('tab', { name: 'Xem', exact: true }).click();
  await cmd(page, 'view.focusedReader').click();
  const reader = page.getByTestId('focused-reader');
  await expect(reader).toBeVisible();
  return reader;
}

test('reflowed text, page settings and their persistence', async ({ page }) => {
  const errors: string[] = [];
  page.on('pageerror', (e) => errors.push(e.message));
  await sampleDocument(page);
  const reader = await openReader(page);
  await expect(reader).toHaveAttribute('aria-label', 'Trình đọc tập trung');
  const text = reader.getByTestId('reader-text');
  await expect(text).toBeFocused();
  // The model's paragraphs: the heading as a heading, the body text as paragraphs.
  await expect(text.getByRole('heading', { level: 1 })).toHaveText('Giới thiệu');
  await expect(text.locator('p')).toHaveCount(2);
  await expect(text.locator('p').nth(0)).toHaveText(VI_TEXT);
  await expect(text.locator('p').nth(1)).toHaveText(EN_TEXT);
  // The text is laid out in several lines.
  expect(Number(await text.getAttribute('data-lines'))).toBeGreaterThan(4);

  const column = reader.getByTestId('reader-column-box');
  // Text size.
  await expect(column).toHaveCSS('font-size', '22px');
  await reader.getByTestId('reader-larger').click();
  await expect(column).toHaveCSS('font-size', '24px');
  await expect(reader.getByTestId('reader-size')).toHaveText('24 px');
  // Letter and word spacing (0.12 em and 0.32 em of 24 px).
  await expect(column).toHaveCSS('letter-spacing', 'normal');
  await reader.getByTestId('reader-spacing').click();
  await expect(column).toHaveCSS('letter-spacing', '2.88px');
  await expect(column).toHaveCSS('word-spacing', '7.68px');
  // Line spacing: 2.4 × 24 px.
  await reader.getByTestId('reader-line-spacing').selectOption('2.4');
  await expect(column).toHaveCSS('line-height', '57.6px');
  // Column width.
  const width = async () => (await column.boundingBox())!.width;
  const medium = await width();
  await reader.getByTestId('reader-column').selectOption('narrow');
  await expect.poll(width).toBeLessThan(medium - 50);
  await reader.getByTestId('reader-column').selectOption('wide');
  await expect.poll(width).toBeGreaterThan(medium + 50);
  // Page colour.
  await expect(reader).toHaveCSS('background-color', 'rgb(255, 255, 255)');
  await reader.getByTestId('reader-theme').selectOption('sepia');
  await expect(reader).toHaveCSS('background-color', 'rgb(246, 238, 218)');
  await expect(reader).toHaveCSS('color', 'rgb(51, 41, 26)');
  await reader.getByTestId('reader-theme').selectOption('dark');
  await expect(reader).toHaveCSS('background-color', 'rgb(20, 20, 20)');
  await expect(reader).toHaveCSS('color', 'rgb(236, 236, 236)');

  // Focus: the tools hide and come back.
  await reader.getByTestId('reader-focus').click();
  await expect(reader.getByRole('toolbar')).toHaveCount(0);
  await expect(text.locator('p').nth(1)).toHaveText(EN_TEXT);
  await reader.getByTestId('reader-show-tools').click();
  await expect(reader.getByRole('toolbar')).toBeVisible();

  // Esc closes; the document is untouched and takes the keyboard again.
  await text.focus();
  await page.keyboard.press('Escape');
  await expect(reader).toBeHidden();
  await expect(page.getByTestId('caret')).toBeVisible();
  await expect(page.locator('.bo-page[data-page="1"]')).toBeVisible();

  // The settings are kept for the next time.
  const again = await openReader(page);
  await expect(again).toHaveAttribute('data-theme', 'dark');
  await expect(again.getByTestId('reader-column-box')).toHaveCSS('font-size', '24px');
  await expect(again.getByTestId('reader-column')).toHaveValue('wide');
  await expect(again.getByTestId('reader-spacing')).toHaveAttribute('aria-pressed', 'true');
  await again.getByTestId('reader-close').click();
  await expect(again).toBeHidden();
  expect(errors).toEqual([]);
});

test('high-contrast app theme replaces the page colour', async ({ page }) => {
  await sampleDocument(page);
  await page.getByRole('tab', { name: 'Xem', exact: true }).click();
  // Light → dark → high contrast.
  await cmd(page, 'view.toggleTheme').click();
  await cmd(page, 'view.toggleTheme').click();
  await expect(page.locator('.bo-root')).toHaveAttribute('data-theme', 'hc');
  const reader = await openReader(page);
  await expect(reader).toHaveAttribute('data-theme', 'hc');
  // The app's high-contrast colours, whatever page colour was chosen.
  await expect(reader).toHaveCSS('background-color', 'rgb(0, 0, 0)');
  await expect(reader).toHaveCSS('color', 'rgb(255, 255, 255)');
  await expect(reader.getByTestId('reader-theme')).toBeDisabled();
  await expect(reader.getByTestId('reader-text').locator('p').nth(1)).toHaveCSS(
    'color',
    'rgb(255, 255, 255)',
  );
});

test('line focus follows the arrow keys and scrolling', async ({ page }) => {
  await sampleDocument(page);
  const reader = await openReader(page);
  const text = reader.getByTestId('reader-text');
  await reader.getByTestId('reader-column').selectOption('narrow');
  await expect(reader.getByTestId('reader-dim-top')).toHaveCount(0);
  await reader.getByTestId('reader-line-focus').selectOption('3');
  const dimTop = reader.getByTestId('reader-dim-top');
  const dimBottom = reader.getByTestId('reader-dim-bottom');
  await expect(dimBottom).toBeVisible();
  await expect(text).toHaveAttribute('data-line', '0');

  /** Height of the bright band and where its middle is in the view. */
  const band = async () => {
    const a = (await dimTop.boundingBox())!;
    const b = (await dimBottom.boundingBox())!;
    const view = (await text.boundingBox())!;
    const top = a.y + a.height;
    return { height: b.y - top, middle: (top + b.y) / 2 - (view.y + view.height / 2) };
  };
  await text.focus();
  for (let i = 0; i < 3; i++) await page.keyboard.press('ArrowDown');
  await expect(text).toHaveAttribute('data-line', '3');
  // Three lines of 22 px text at line spacing 1.5 (33 px each) are bright, in the middle.
  const three = await band();
  expect(three.height).toBeGreaterThan(80);
  expect(three.height).toBeLessThan(125);
  expect(Math.abs(three.middle)).toBeLessThan(20);
  // The words of the focused line are inside the bright band, the first line is not.
  const inBand = async (line: number) => {
    const a = (await dimTop.boundingBox())!;
    const b = (await dimBottom.boundingBox())!;
    return text.evaluate(
      (el, [n, from, to]) => {
        // Lines of the paragraph by the vertical position of its words.
        const tops: number[] = [];
        for (const w of el.querySelectorAll('p [data-w]')) {
          const r = w.getBoundingClientRect();
          if (!tops.some((t) => Math.abs(t - r.top) < 4)) tops.push(r.top);
        }
        const top = tops[n]!;
        return top >= from! && top < to!;
      },
      [line, a.y + a.height, b.y] as const,
    );
  };
  // Line 3 of the text is the third line of the first paragraph (line 0 is the heading).
  expect(await inBand(2)).toBe(true);
  expect(await inBand(0)).toBe(false);

  await reader.getByTestId('reader-line-focus').selectOption('1');
  const one = await band();
  expect(one.height).toBeGreaterThan(20);
  expect(one.height).toBeLessThan(three.height - 40);
  await reader.getByTestId('reader-line-focus').selectOption('5');
  await expect.poll(async () => (await band()).height).toBeGreaterThan(three.height + 40);

  await text.focus();
  await page.keyboard.press('ArrowUp');
  await expect(text).toHaveAttribute('data-line', '2');
  // Scrolling moves the focus to the line in the middle of the view.
  await text.hover();
  await page.mouse.wheel(0, 140);
  await expect
    .poll(async () => Number(await text.getAttribute('data-line')))
    .toBeGreaterThanOrEqual(5);
  await page.keyboard.press('Home');
  await expect(text).toHaveAttribute('data-line', '0');
  // The choice is kept.
  await page.keyboard.press('Escape');
  const again = await openReader(page);
  await expect(again.getByTestId('reader-line-focus')).toHaveValue('5');
  await expect(again.getByTestId('reader-dim-bottom')).toBeVisible();
});

test('syllable marks on English words only', async ({ page }) => {
  await sampleDocument(page);
  const reader = await openReader(page);
  const text = reader.getByTestId('reader-text');
  await expect(text.locator('[data-syllable]')).toHaveCount(0);
  await reader.getByTestId('reader-syllables').click();
  const parts = (word: string) =>
    text
      .locator('p')
      .nth(1)
      .locator('[data-w]')
      .filter({ hasText: new RegExp(`^${word}$`) })
      .locator('[data-syllable]');
  await expect(parts('children')).toHaveText(['chil', 'dren']);
  await expect(parts('important')).toHaveText(['im', 'por', 'tant']);
  await expect(parts('wonderful')).toHaveText(['won', 'der', 'ful']);
  await expect(parts('The')).toHaveCount(0);
  // A dot is drawn between syllables; the text itself does not change.
  const dot = await parts('children')
    .nth(1)
    .evaluate((el) => getComputedStyle(el, '::before').content);
  expect(dot).toBe('"·"');
  expect(
    await parts('children')
      .nth(0)
      .evaluate((el) => getComputedStyle(el, '::before').content),
  ).toBe('none');
  await expect(text.locator('p').nth(1)).toHaveText(EN_TEXT);
  // Vietnamese is written syllable by syllable: nothing to mark.
  await expect(text.locator('p').nth(0).locator('[data-syllable]')).toHaveCount(0);
  await expect(text.locator('p').nth(0)).toHaveText(VI_TEXT);
});

test('read aloud starts at the current line and highlights the spoken word', async ({ page }) => {
  await fakeSpeech(page);
  await sampleDocument(page);
  const reader = await openReader(page);
  const text = reader.getByTestId('reader-text');
  await reader.getByTestId('reader-column').selectOption('narrow');
  await reader.getByTestId('reader-line-focus').selectOption('1');
  await text.focus();
  await page.keyboard.press('ArrowDown');
  await page.keyboard.press('ArrowDown');
  await expect(text).toHaveAttribute('data-line', '2');
  // The first word of line 2 (the second line of the Vietnamese paragraph).
  const firstWord = await text.evaluate((el) => {
    const words = Array.from(el.querySelectorAll<HTMLElement>('p [data-w]'));
    const top = words[0]!.getBoundingClientRect().top;
    const next = words.find((w) => w.getBoundingClientRect().top > top + 4)!;
    return { text: next.textContent ?? '', offset: Number(next.dataset.w) };
  });
  expect(firstWord.offset).toBeGreaterThan(0);

  const play = reader.getByTestId('reader-play');
  await expect(play).toHaveText('Đọc to');
  await play.click();
  await expect(reader.getByTestId('read-aloud-bar')).toBeVisible();
  await expect(play).toHaveAttribute('aria-pressed', 'true');
  const spoken = () =>
    page.evaluate(
      () => (window as unknown as { __spoken: { text: string; lang: string }[] }).__spoken,
    );
  await expect.poll(async () => (await spoken()).length).toBeGreaterThan(0);
  const first = (await spoken())[0]!;
  expect(first.lang).toBe('vi-VN');
  // Reading begins with that line, not with the heading or the start of the paragraph.
  expect(first.text.startsWith(`${firstWord.text} `)).toBe(true);
  expect(VI_TEXT.slice(firstWord.offset).startsWith(first.text.trimEnd())).toBe(true);

  // The spoken word is highlighted and moves on.
  const mark = text.locator('[data-spoken]');
  await expect(mark).toHaveCount(1);
  const seen = new Set<string>();
  await expect
    .poll(async () => {
      const w = await mark
        .first()
        .getAttribute('data-w', { timeout: 1000 })
        .catch(() => null);
      if (w) seen.add(w);
      return seen.size;
    })
    .toBeGreaterThanOrEqual(3);
  await expect(mark.first()).toHaveCSS('background-color', 'rgb(255, 229, 138)');
  // Line focus follows the voice.
  await expect
    .poll(async () => Number(await text.getAttribute('data-line')), { timeout: 20_000 })
    .toBeGreaterThan(2);

  // Pause from the bar, then stop from the reader.
  await reader.getByTestId('read-aloud-play').click();
  await expect(reader.getByTestId('read-aloud-bar')).toHaveAttribute('data-state', 'paused');
  await play.click();
  await expect(reader.getByTestId('read-aloud-bar')).toHaveCount(0);
  await expect(mark).toHaveCount(0);
  await expect(play).toHaveAttribute('aria-pressed', 'false');
  await page.keyboard.press('Escape');
  await expect(reader).toBeHidden();
});
