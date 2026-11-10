// @vitest-environment happy-dom
import { createI18n, I18nextProvider } from '@blueoffice/i18n';
import {
  FakeSpeechProvider,
  type EditCommand,
  type EngineHost,
  type SpeechVoice,
} from '@blueoffice/platform';
import type { SettingStore } from '@blueoffice/ui';
import { act, cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { docReaderBlocks, readerStart } from '../src/docFocusedReader';
import {
  FocusedReader,
  groupLines,
  lineOf,
  lineWindow,
  type FocusedReaderProps,
  type ReaderBlock,
  type WordBox,
} from '../src/FocusedReader';
import {
  DEFAULT_READER_PREFS,
  READER_PALETTES,
  READER_PREFS_KEY,
  READER_THEMES,
  contrast,
  parseReaderPrefs,
  type ReaderPrefs,
} from '../src/focusedReaderPrefs';

afterEach(() => {
  cleanup();
  localStorage.clear();
});

const VI: SpeechVoice = { id: 'vi', name: 'An', lang: 'vi-VN', local: true, default: false };
const EN: SpeechVoice = { id: 'en', name: 'Sam', lang: 'en-US', local: true, default: true };

/** A heading (2 words), a Vietnamese paragraph (13 words), an English one (10 words). */
const BLOCKS: ReaderBlock[] = [
  { p: 0, level: 0, lang: 'en-US', text: 'Reading information' },
  {
    p: 2,
    level: null,
    lang: 'vi-VN',
    text: 'Tiếng Việt viết theo từng âm tiết rời nhau nên không cần tách',
  },
  {
    p: 3,
    level: null,
    lang: 'en-US',
    text: 'The children ask an important question about a wonderful table',
  },
];

/** Layout of the tests: three words per line, lines 30 px apart, blocks 1000 px apart. */
const PER_LINE = 3;
const measureWord = (el: HTMLElement) => {
  const block = el.parentElement!;
  const i = Array.from(block.querySelectorAll(':scope > [data-w]')).indexOf(el);
  const top = Number(block.dataset.block) * 1000 + Math.floor(i / PER_LINE) * 30;
  return { top, bottom: top + 20 };
};
// Block 0: line 0. Block 1 (13 words): lines 1–5. Block 2 (10 words): lines 6–9.
const LINES = 10;

const i18n = createI18n('en-US');

function mount(props: Partial<FocusedReaderProps> = {}) {
  const onExit = vi.fn();
  const all: FocusedReaderProps = {
    name: 'report.docx',
    blocks: BLOCKS,
    settings: {},
    onExit,
    measureWord,
    ...props,
  };
  const r = render(
    <I18nextProvider i18n={i18n}>
      <FocusedReader {...all} />
    </I18nextProvider>,
  );
  const root = () => screen.getByTestId('focused-reader');
  const text = () => screen.getByTestId('reader-text');
  const css = (name: string) => root().style.getPropertyValue(name);
  const stored = () => JSON.parse(localStorage.getItem(READER_PREFS_KEY) ?? 'null') as ReaderPrefs;
  return { ...r, onExit, root, text, css, stored };
}

const select = (id: string, value: string) =>
  fireEvent.change(screen.getByTestId(id), { target: { value } });
const key = (el: Element, k: string, more: Partial<KeyboardEventInit> = {}) =>
  fireEvent.keyDown(el, { key: k, ...more });

describe('line layout helpers', () => {
  const box = (block: number, offset: number, top: number): WordBox => ({
    block,
    offset,
    top,
    bottom: top + 20,
  });

  it('groups words into lines by their vertical position and block', () => {
    const lines = groupLines([
      box(0, 0, 0),
      // A taller word on the same line (its top is above the middle of the line).
      { block: 0, offset: 5, top: -4, bottom: 22 },
      box(0, 9, 30),
      box(0, 14, 30),
      // Same top in the next block is still another line.
      box(1, 0, 30),
    ]);
    expect(lines).toEqual([
      { block: 0, offset: 0, top: -4, bottom: 22 },
      { block: 0, offset: 9, top: 30, bottom: 50 },
      { block: 1, offset: 0, top: 30, bottom: 50 },
    ]);
    expect(groupLines([])).toEqual([]);
  });

  it('keeps 1, 3 or 5 lines around the current one, fewer at the ends', () => {
    expect(lineWindow(10, 4, 1)).toEqual({ from: 4, to: 4 });
    expect(lineWindow(10, 4, 3)).toEqual({ from: 3, to: 5 });
    expect(lineWindow(10, 4, 5)).toEqual({ from: 2, to: 6 });
    expect(lineWindow(10, 0, 5)).toEqual({ from: 0, to: 2 });
    expect(lineWindow(10, 9, 3)).toEqual({ from: 8, to: 9 });
    expect(lineWindow(10, 99, 1)).toEqual({ from: 9, to: 9 });
  });

  it('finds the line of a text position', () => {
    const lines = groupLines([box(0, 0, 0), box(1, 0, 100), box(1, 12, 130), box(2, 0, 200)]);
    expect(lineOf(lines, 0, 3)).toBe(0);
    expect(lineOf(lines, 1, 11)).toBe(1);
    expect(lineOf(lines, 1, 12)).toBe(2);
    expect(lineOf(lines, 1, 40)).toBe(2);
    expect(lineOf(lines, 2, 0)).toBe(3);
    expect(lineOf([], 2, 0)).toBe(0);
  });
});

describe('reader preferences', () => {
  it('accepts stored values and falls back for unknown ones', () => {
    expect(parseReaderPrefs(null)).toEqual(DEFAULT_READER_PREFS);
    expect(parseReaderPrefs('not json')).toEqual(DEFAULT_READER_PREFS);
    expect(parseReaderPrefs('[1]')).toEqual(DEFAULT_READER_PREFS);
    const kept: ReaderPrefs = {
      textSize: 30,
      wideSpacing: true,
      lineSpacing: 1.8,
      column: 'wide',
      theme: 'sepia',
      lineFocus: 5,
      syllables: true,
    };
    expect(parseReaderPrefs(JSON.stringify(kept))).toEqual(kept);
    expect(
      parseReaderPrefs(
        JSON.stringify({
          textSize: 900,
          lineSpacing: 7,
          column: 'huge',
          theme: 'pink',
          lineFocus: 2,
        }),
      ),
    ).toEqual({ ...DEFAULT_READER_PREFS, textSize: 48 });
  });

  it('has page colours that meet WCAG 2.2 AA contrast', () => {
    // The formula on its own first: black on white is 21:1.
    expect(contrast('#000000', '#ffffff')).toBeCloseTo(21, 5);
    expect(contrast('#777777', '#ffffff')).toBeCloseTo(4.48, 2);
    for (const theme of READER_THEMES) {
      const c = READER_PALETTES[theme];
      // Text (1.4.3: 4.5:1) on the page and on the tool bar, also the secondary text.
      for (const [fg, bg] of [
        [c.text, c.bg],
        [c.text, c.bar],
        [c.muted, c.bg],
        [c.muted, c.bar],
        [c.markText, c.markBg],
        // A pressed toggle swaps text and page colour.
        [c.bg, c.text],
      ] as const) {
        expect(contrast(fg, bg), `${theme} ${fg} on ${bg}`).toBeGreaterThanOrEqual(4.5);
      }
      // Control borders and the focus ring (1.4.11: 3:1) against what is next to them.
      for (const [fg, bg] of [
        [c.border, c.bg],
        [c.border, c.bar],
        [c.focus, c.bg],
        [c.focus, c.bar],
        // The spoken word stands out from the page.
        [c.markBg, c.text],
      ] as const) {
        expect(contrast(fg, bg), `${theme} ${fg} against ${bg}`).toBeGreaterThanOrEqual(3);
      }
    }
  });
});

describe('FocusedReader', () => {
  it('shows the paragraphs as reflowed, read-only text', () => {
    const { root, text } = mount();
    expect(root().getAttribute('role')).toBe('dialog');
    expect(root().getAttribute('aria-modal')).toBe('true');
    expect(root().getAttribute('aria-label')).toBe('Focused reader');
    const heading = screen.getByRole('heading', { level: 1 });
    expect(heading.textContent).toBe('Reading information');
    const paras = text().querySelectorAll('p');
    expect(Array.from(paras, (p) => p.textContent)).toEqual([BLOCKS[1]!.text, BLOCKS[2]!.text]);
    expect(paras[0]!.getAttribute('lang')).toBe('vi-VN');
    // The text takes the keyboard; nothing in it can be edited.
    expect(document.activeElement).toBe(text());
    expect(text().getAttribute('aria-label')).toBe('Text of report.docx');
    expect(root().querySelector('[contenteditable], input, textarea')).toBeNull();
    expect(text().dataset.lines).toBe(String(LINES));
  });

  it('says so when the document has no text', () => {
    mount({ blocks: [] });
    expect(screen.getByTestId('reader-empty').textContent).toBe(
      'The document has no text to read yet.',
    );
    expect((screen.getByTestId('reader-play') as HTMLButtonElement).disabled).toBe(true);
  });

  it('changes text size, spacing, line spacing, column width and page colour', () => {
    const { root, css, stored } = mount();
    expect(css('--r-size')).toBe('22px');
    expect(css('--r-letter')).toBe('normal');
    expect(css('--r-line')).toBe('1.5');
    expect(css('--r-column')).toBe('62ch');
    expect(root().dataset.theme).toBe('light');
    expect(css('--r-bg')).toBe(READER_PALETTES.light.bg);

    fireEvent.click(screen.getByTestId('reader-larger'));
    fireEvent.click(screen.getByTestId('reader-larger'));
    expect(css('--r-size')).toBe('26px');
    expect(screen.getByTestId('reader-size').textContent).toBe('26 px');
    fireEvent.click(screen.getByTestId('reader-smaller'));
    expect(css('--r-size')).toBe('24px');

    const spacing = screen.getByTestId('reader-spacing');
    expect(spacing.getAttribute('aria-pressed')).toBe('false');
    fireEvent.click(spacing);
    expect(spacing.getAttribute('aria-pressed')).toBe('true');
    expect(css('--r-letter')).toBe('0.12em');
    expect(css('--r-word')).toBe('0.32em');

    select('reader-line-spacing', '2.4');
    expect(css('--r-line')).toBe('2.4');
    select('reader-column', 'narrow');
    expect(css('--r-column')).toBe('42ch');
    select('reader-column', 'wide');
    expect(css('--r-column')).toBe('86ch');

    for (const theme of ['sepia', 'dark'] as const) {
      select('reader-theme', theme);
      expect(root().dataset.theme).toBe(theme);
      expect(css('--r-bg')).toBe(READER_PALETTES[theme].bg);
      expect(css('--r-text')).toBe(READER_PALETTES[theme].text);
    }
    expect(stored()).toEqual({
      ...DEFAULT_READER_PREFS,
      textSize: 24,
      wideSpacing: true,
      lineSpacing: 2.4,
      column: 'wide',
      theme: 'dark',
    });
  });

  it('stops at the smallest and largest text size', () => {
    localStorage.setItem(READER_PREFS_KEY, JSON.stringify({ textSize: 46 }));
    const { css } = mount();
    const larger = screen.getByTestId('reader-larger') as HTMLButtonElement;
    return waitFor(() => expect(css('--r-size')).toBe('46px')).then(() => {
      fireEvent.click(larger);
      expect(css('--r-size')).toBe('48px');
      expect(larger.disabled).toBe(true);
    });
  });

  it('uses the app colours in the high-contrast theme', () => {
    const { root, css } = mount({ highContrast: true });
    expect(root().dataset.theme).toBe('hc');
    // No page colour of its own: the style sheet falls back to the app's tokens.
    expect(css('--r-bg')).toBe('');
    expect(css('--r-text')).toBe('');
    const theme = screen.getByTestId('reader-theme') as HTMLSelectElement;
    expect(theme.disabled).toBe(true);
    expect(theme.title).toBe("The app's high-contrast theme is in use");
    // The other settings still apply.
    fireEvent.click(screen.getByTestId('reader-larger'));
    expect(css('--r-size')).toBe('24px');
  });

  it('keeps the preferences for the next time (localStorage)', async () => {
    const first = mount();
    select('reader-theme', 'sepia');
    select('reader-line-focus', '5');
    fireEvent.click(screen.getByTestId('reader-syllables'));
    first.unmount();
    const { root } = mount();
    await waitFor(() => expect(root().dataset.theme).toBe('sepia'));
    expect((screen.getByTestId('reader-line-focus') as HTMLSelectElement).value).toBe('5');
    expect(screen.getByTestId('reader-syllables').getAttribute('aria-pressed')).toBe('true');
    expect(screen.getByTestId('reader-dim-bottom')).toBeTruthy();
  });

  it('keeps the preferences in the platform setting store when there is one', async () => {
    const values = new Map<string, string | null>([
      [READER_PREFS_KEY, JSON.stringify({ theme: 'dark', textSize: 30 })],
    ]);
    const settings: SettingStore = {
      getSetting: vi.fn((k: string) => Promise.resolve(values.get(k) ?? null)),
      setSetting: vi.fn((k: string, v: string | null) => {
        values.set(k, v);
        return Promise.resolve();
      }),
    };
    const { root, css } = mount({ settings });
    await waitFor(() => expect(root().dataset.theme).toBe('dark'));
    expect(css('--r-size')).toBe('30px');
    select('reader-column', 'narrow');
    expect(settings.setSetting).toHaveBeenCalledTimes(1);
    expect(parseReaderPrefs(values.get(READER_PREFS_KEY) ?? null)).toEqual({
      ...DEFAULT_READER_PREFS,
      theme: 'dark',
      textSize: 30,
      column: 'narrow',
    });
    expect(localStorage.getItem(READER_PREFS_KEY)).toBeNull();
  });

  it('line focus keeps 1 / 3 / 5 lines bright and moves with the arrow keys', () => {
    const { text } = mount();
    expect(screen.queryByTestId('reader-dim-top')).toBeNull();
    select('reader-line-focus', '3');
    const top = () => screen.getByTestId('reader-dim-top').style;
    const bottom = () => screen.getByTestId('reader-dim-bottom').style;
    // Line 0 (the heading): lines 0–1 are bright. Line 1 is the first of block 1: 1000–1020.
    expect(text().dataset.line).toBe('0');
    expect(top().height).toBe('0px');
    expect(bottom().top).toBe('1024px');

    key(text(), 'ArrowDown');
    key(text(), 'ArrowDown');
    // Line 2: lines 1–3 → from 1000 − 4 to (1000 + 2 × 30 + 20) + 4.
    expect(text().dataset.line).toBe('2');
    expect(top().height).toBe('996px');
    expect(bottom().top).toBe('1084px');

    select('reader-line-focus', '1');
    expect(top().height).toBe('1026px');
    expect(bottom().top).toBe('1054px');
    select('reader-line-focus', '5');
    // Lines 0–4.
    expect(top().height).toBe('0px');
    expect(bottom().top).toBe('1114px');

    key(text(), 'ArrowUp');
    expect(text().dataset.line).toBe('1');
    key(text(), 'End');
    expect(text().dataset.line).toBe(String(LINES - 1));
    key(text(), 'ArrowDown');
    expect(text().dataset.line).toBe(String(LINES - 1));
    key(text(), 'PageUp');
    expect(text().dataset.line).toBe(String(LINES - 1 - 5));
    key(text(), 'Home');
    expect(text().dataset.line).toBe('0');
    key(text(), 'ArrowUp');
    expect(text().dataset.line).toBe('0');

    // Arrow keys in a list box choose a value; they do not move the line.
    key(screen.getByTestId('reader-column'), 'ArrowDown');
    expect(text().dataset.line).toBe('0');

    select('reader-line-focus', '0');
    expect(screen.queryByTestId('reader-dim-top')).toBeNull();
    key(text(), 'ArrowDown');
    expect(text().dataset.line).toBe('0');
  });

  it('marks syllables of English words and leaves Vietnamese syllables whole', () => {
    const { text } = mount();
    expect(text().querySelector('[data-syllable]')).toBeNull();
    fireEvent.click(screen.getByTestId('reader-syllables'));
    const word = (block: number, w: string) =>
      Array.from(text().querySelectorAll(`[data-block="${block}"] [data-w]`)).find(
        (el) => el.textContent === w,
      )!;
    const parts = (block: number, w: string) =>
      Array.from(word(block, w).querySelectorAll('[data-syllable]'), (s) => s.textContent);
    expect(parts(0, 'information')).toEqual(['in', 'for', 'ma', 'tion']);
    expect(parts(2, 'children')).toEqual(['chil', 'dren']);
    expect(parts(2, 'table')).toEqual(['ta', 'ble']);
    // The dot is drawn before every syllable but the first.
    const marked = word(0, 'information').querySelectorAll('.syllable');
    expect(marked).toHaveLength(3);
    // One-syllable and Vietnamese words are not divided.
    expect(parts(2, 'The')).toEqual([]);
    expect(text().querySelectorAll('[data-block="1"] [data-syllable]')).toHaveLength(0);
    // The text itself is unchanged (the marks are not characters).
    expect(text().querySelector('[data-block="2"]')!.textContent).toBe(BLOCKS[2]!.text);
    fireEvent.click(screen.getByTestId('reader-syllables'));
    expect(text().querySelector('[data-syllable]')).toBeNull();
  });

  it('hides and shows its tools (focus)', () => {
    const { root, text } = mount();
    fireEvent.click(screen.getByTestId('reader-focus'));
    expect(root().dataset.focusMode).toBe('true');
    expect(screen.queryByRole('toolbar')).toBeNull();
    expect(text().textContent).toContain('Reading information');
    const show = screen.getByTestId('reader-show-tools');
    expect(show.getAttribute('aria-label')).toBe('Show reading tools');
    fireEvent.click(show);
    expect(screen.getByRole('toolbar', { name: 'Reading tools' })).toBeTruthy();
    expect(root().dataset.focusMode).toBeUndefined();
  });

  it('closes with Esc or the close button, reporting the paragraph being read', () => {
    const { text, onExit } = mount({ start: 2 });
    // Opened at block 2: its first line is the current one.
    expect(text().dataset.line).toBe('6');
    key(text(), 'Escape');
    expect(onExit).toHaveBeenCalledTimes(1);
    expect(onExit).toHaveBeenLastCalledWith(BLOCKS[2]);
    fireEvent.click(screen.getByTestId('reader-close'));
    expect(onExit).toHaveBeenCalledTimes(2);
  });

  it('keeps keys and focus inside the reader', () => {
    const outside = vi.fn();
    window.addEventListener('keydown', outside);
    try {
      const { text } = mount();
      key(text(), 'b', { ctrlKey: true });
      key(screen.getByTestId('reader-smaller'), 'F7');
      expect(outside).not.toHaveBeenCalled();
      // Tab wraps from the last control to the first and back.
      const firstControl = screen.getByTestId('reader-smaller');
      expect(document.activeElement).toBe(text());
      key(text(), 'Tab');
      expect(document.activeElement).toBe(firstControl);
      key(firstControl, 'Tab', { shiftKey: true });
      expect(document.activeElement).toBe(text());
    } finally {
      window.removeEventListener('keydown', outside);
    }
  });

  it('labels every control', () => {
    const { root } = mount();
    for (const el of root().querySelectorAll('button, select')) {
      const label =
        el.getAttribute('aria-label') ||
        (el.tagName === 'SELECT' ? el.closest('label')?.textContent : el.textContent);
      expect(label?.trim(), el.getAttribute('data-testid') ?? '').toBeTruthy();
    }
  });

  it('reads aloud from the current line and highlights the spoken word', async () => {
    const speech = new FakeSpeechProvider([VI, EN]);
    const { text } = mount({ speech });
    select('reader-line-focus', '1');
    key(text(), 'ArrowDown');
    key(text(), 'ArrowDown');
    // Line 2 is the second line of the Vietnamese paragraph: its fourth word onwards.
    expect(text().dataset.line).toBe('2');
    const play = screen.getByTestId('reader-play');
    expect(play.textContent).toBe('Read aloud');
    await act(async () => {
      fireEvent.click(play);
      await Promise.resolve();
    });
    await waitFor(() => expect(speech.spoken).toHaveLength(1));
    expect(speech.spoken[0]!.text).toBe('theo từng âm tiết rời nhau nên không cần tách');
    expect(speech.spoken[0]!.options.lang).toBe('vi-VN');
    expect(play.getAttribute('aria-pressed')).toBe('true');
    expect(play.textContent).toBe('Stop reading');
    // The read-aloud controls are inside the reader.
    expect(
      text()
        .closest('[data-testid="focused-reader"]')!
        .contains(screen.getByTestId('read-aloud-bar')),
    ).toBe(true);

    const spokenWords = () =>
      Array.from(text().querySelectorAll('[data-spoken]'), (el) => el.textContent);
    act(() => void speech.step());
    expect(spokenWords()).toEqual(['theo']);
    expect(text().querySelector('[data-spoken]')!.className).toContain('spoken');
    act(() => void speech.step());
    act(() => void speech.step());
    expect(spokenWords()).toEqual(['âm']);
    expect(text().dataset.line).toBe('2');
    // The next word is on the next line: line focus follows the voice.
    act(() => void speech.step());
    expect(spokenWords()).toEqual(['tiết']);
    expect(text().dataset.line).toBe('3');

    // On into the next paragraph, in its language.
    act(() => speech.finish());
    await waitFor(() => expect(speech.spoken).toHaveLength(2));
    expect(speech.spoken[1]!.text).toBe(BLOCKS[2]!.text);
    expect(speech.spoken[1]!.options.lang).toBe('en-US');
    act(() => void speech.step());
    expect(spokenWords()).toEqual(['The']);
    expect(text().dataset.line).toBe('6');

    fireEvent.click(play);
    expect(spokenWords()).toEqual([]);
    expect(play.getAttribute('aria-pressed')).toBe('false');
    expect(screen.queryByTestId('read-aloud-bar')).toBeNull();
  });

  it('explains when the platform cannot speak', async () => {
    mount();
    await act(async () => {
      fireEvent.click(screen.getByTestId('reader-play'));
      await Promise.resolve();
    });
    expect((await screen.findByTestId('read-aloud-notice')).textContent).toBe(
      'This browser or app cannot read aloud.',
    );
  });
});

describe('BlueDoc text for the reader', () => {
  const engine = (paras: unknown[], outline: unknown[]) => {
    const calls: string[] = [];
    const host = {
      command: (c: EditCommand) => {
        calls.push(c.cmd);
        if (c.cmd === 'speechText') return Promise.resolve({ speech: { paras } });
        if (c.cmd === 'outline') return Promise.resolve({ outline });
        return Promise.reject(new Error(`unexpected ${c.cmd}`));
      },
    } as unknown as EngineHost;
    return { host, calls };
  };

  it('takes paragraphs from the engine: heading levels, languages, no hidden text', async () => {
    const { host, calls } = engine(
      [
        { p: 0, text: 'Mở đầu', detected: 'vi' },
        { p: 1, text: '' },
        // "chào" (bytes 4–9: à is two bytes) is deleted or hidden text.
        { p: 2, text: 'Xin chào bạn', lang: 'vi-VN', skip: [[4, 9]] },
        { p: 3, text: 'A￼B', lang: 'en-US' },
        { p: 4, text: ' ￼ ' },
      ],
      [
        { para: 0, level: 0, text: 'Mở đầu', section: 0, page: 0 },
        { para: 2, level: null, text: 'Xin chào bạn', section: 0, page: 0 },
      ],
    );
    const blocks = await docReaderBlocks(host, 'en-US');
    expect(calls.sort()).toEqual(['outline', 'speechText']);
    expect(blocks.map((b) => [b.p, b.level, b.lang])).toEqual([
      [0, 0, 'vi-VN'],
      [2, null, 'vi-VN'],
      [3, null, 'en-US'],
    ]);
    expect(blocks[1]!.text.split(/\s+/)).toEqual(['Xin', 'bạn']);
    // Offsets are those of the read-aloud unit: the length is unchanged.
    expect(blocks[1]!.text).toHaveLength('Xin chào bạn'.length);
    expect(blocks[2]!.text).toBe('A B');

    expect(readerStart(blocks, undefined)).toBe(0);
    expect(readerStart(blocks, 0)).toBe(0);
    // The caret is in an empty paragraph: the next one with text.
    expect(readerStart(blocks, 1)).toBe(1);
    expect(readerStart(blocks, 3)).toBe(2);
    expect(readerStart(blocks, 9)).toBe(2);
    expect(readerStart([], 4)).toBe(0);
  });
});
