/**
 * Focused reader (CORE-LANG-004): a read-only, reflowed text view of the document without the
 * editing chrome. The reader chooses text size, letter / word spacing, line spacing, column
 * width and a page colour (light / sepia / dark; the app's high-contrast theme wins), can keep
 * 1 / 3 / 5 lines bright and dim the rest (line focus, moved with the arrow keys or by
 * scrolling), mark the syllables of words (`syllables.ts`), hide its own tools (focus), and
 * read aloud from the current line with the spoken word highlighted (`useReadAloud`).
 *
 * The text comes from the model through the engine (`docReaderBlocks`); nothing is edited here.
 */
import { useTranslation } from '@blueoffice/i18n';
import type { SpeechProvider, TextRange } from '@blueoffice/platform';
import type { SettingStore } from '@blueoffice/ui';
import { Minus, PanelTopOpen, Play, Plus, Square, X } from 'lucide-react';
import {
  memo,
  useCallback,
  useEffect,
  useLayoutEffect,
  useMemo,
  useRef,
  useState,
  type CSSProperties,
  type KeyboardEvent as ReactKeyboardEvent,
  type ReactNode,
} from 'react';
import styles from './FocusedReader.module.css';
import {
  COLUMN_CH,
  DEFAULT_READER_PREFS,
  LINE_FOCUS,
  LINE_SPACINGS,
  READER_COLUMNS,
  READER_PALETTES,
  READER_THEMES,
  TEXT_SIZE,
  loadReaderPrefs,
  saveReaderPrefs,
  type LineFocus,
  type ReaderColumn,
  type ReaderPrefs,
  type ReaderTheme,
} from './focusedReaderPrefs';
import { useReadAloud } from './ReadAloudBar';
import type { ReadPosition } from './readAloud';
import { syllables } from './syllables';

/** A paragraph shown in the reader. Offsets of its words are UTF-16 units of `text`. */
export interface ReaderBlock {
  /** Body paragraph index in the document. */
  p: number;
  text: string;
  /** 0-based heading level (0 = Heading 1); null = body text. */
  level: number | null;
  /** Language it is read aloud in (BCP 47). */
  lang?: string | null;
}

/** Where a word was laid out (CSS px from the top of the text). */
export interface WordBox {
  block: number;
  /** UTF-16 offset of the word in its block. */
  offset: number;
  top: number;
  bottom: number;
}

/** A laid-out line: its extent and the word it starts with. */
export interface ReaderLine {
  block: number;
  offset: number;
  top: number;
  bottom: number;
}

/** Lines of the words in reading order: a word past the middle of the line starts a new one. */
export function groupLines(words: readonly WordBox[]): ReaderLine[] {
  const lines: ReaderLine[] = [];
  let cur: ReaderLine | undefined;
  for (const w of words) {
    if (cur && cur.block === w.block && w.top < (cur.top + cur.bottom) / 2) {
      cur.top = Math.min(cur.top, w.top);
      cur.bottom = Math.max(cur.bottom, w.bottom);
      continue;
    }
    cur = { block: w.block, offset: w.offset, top: w.top, bottom: w.bottom };
    lines.push(cur);
  }
  return lines;
}

/** First and last line kept bright around line `at` (`count` lines, fewer at the ends). */
export function lineWindow(total: number, at: number, count: number): { from: number; to: number } {
  const half = Math.floor(Math.max(1, count) / 2);
  const mid = Math.min(Math.max(0, at), Math.max(0, total - 1));
  return { from: Math.max(0, mid - half), to: Math.min(Math.max(0, total - 1), mid + half) };
}

/** The line holding offset `offset` of block `block` (the last line starting at or before it). */
export function lineOf(lines: readonly ReaderLine[], block: number, offset: number): number {
  let found = -1;
  for (let i = 0; i < lines.length; i++) {
    const l = lines[i]!;
    if (l.block < block || (l.block === block && l.offset <= offset)) found = i;
    else break;
  }
  return Math.max(0, found);
}

export interface FocusedReaderProps {
  /** Document name (labels the text). */
  name: string;
  blocks: readonly ReaderBlock[];
  /** Block to start at. */
  start?: number;
  /** Text to speech of the platform (absent: Play explains it is unavailable). */
  speech?: SpeechProvider;
  /** Where the preferences are kept (`platform.shell`). */
  settings: SettingStore;
  /** The app is in its high-contrast theme: its colours replace the page colour. */
  highContrast?: boolean;
  /** Leave the reader; `block` is the paragraph being read. */
  onExit: (block: ReaderBlock | undefined) => void;
  /** Vertical extent of a word element (tests replace the browser's layout). */
  measureWord?: (el: HTMLElement) => { top: number; bottom: number };
}

/** Room kept bright above and below the focused lines (CSS px). */
const FOCUS_PAD = 4;

const domBox = (el: HTMLElement) => el.getBoundingClientRect();

const Block = memo(function Block({
  index,
  block,
  marks,
  spoken,
}: {
  index: number;
  block: ReaderBlock;
  /** Mark syllables. */
  marks: boolean;
  /** Range being read aloud in this block. */
  spoken: TextRange | null;
}) {
  const words = useMemo(
    () => [...block.text.matchAll(/\S+/g)].map((m) => ({ at: m.index, text: m[0] })),
    [block.text],
  );
  const out: ReactNode[] = [];
  words.forEach((w, i) => {
    if (i > 0) out.push(' ');
    const hit = !!spoken && w.at < spoken.end && w.at + w.text.length > spoken.start;
    const parts = marks ? syllables(w.text) : [w.text];
    out.push(
      <span
        key={w.at}
        data-w={w.at}
        data-spoken={hit ? 'true' : undefined}
        className={hit ? styles.spoken : undefined}
      >
        {parts.length > 1
          ? parts.map((s, k) => (
              // The separator dot is drawn by CSS: the text itself is unchanged.
              <span key={k} data-syllable={k} className={k > 0 ? styles.syllable : undefined}>
                {s}
              </span>
            ))
          : w.text}
      </span>,
    );
  });
  const level = block.level;
  const Tag = level === null ? 'p' : (`h${Math.min(6, level + 1)}` as 'h1');
  return (
    <Tag
      data-block={index}
      data-para={block.p}
      lang={block.lang ?? undefined}
      className={level === null ? styles.para : styles.heading}
    >
      {out}
    </Tag>
  );
});

export function FocusedReader({
  name,
  blocks,
  start = 0,
  speech,
  settings,
  highContrast = false,
  onExit,
  measureWord = domBox,
}: FocusedReaderProps) {
  const { t, i18n } = useTranslation();
  const root = useRef<HTMLDivElement>(null);
  const scroller = useRef<HTMLDivElement>(null);
  const page = useRef<HTMLDivElement>(null);
  const [prefs, setPrefs] = useState<ReaderPrefs>(DEFAULT_READER_PREFS);
  const [focusMode, setFocusMode] = useState(false);
  const [lines, setLines] = useState<ReaderLine[]>([]);
  const [line, setLine] = useState(0);
  const [spoken, setSpoken] = useState<ReadPosition | null>(null);
  const aloud = useReadAloud(speech, 'paragraph');

  const cur = Math.min(line, Math.max(0, lines.length - 1));
  // Layout callbacks read the latest values without being re-created.
  const live = useRef({ lines, cur, prefs });
  useLayoutEffect(() => {
    live.current = { lines, cur, prefs };
  });

  // Keys go to the reader, not to the document's text input.
  useEffect(() => scroller.current?.focus(), []);

  // A choice made before the stored preferences arrive wins over them.
  const touched = useRef(false);
  useEffect(() => {
    let on = true;
    void loadReaderPrefs(settings).then((p) => {
      if (on && !touched.current) setPrefs(p);
    });
    return () => {
      on = false;
    };
  }, [settings]);

  const change = (next: Partial<ReaderPrefs>) => {
    const v = { ...prefs, ...next };
    touched.current = true;
    setPrefs(v);
    void saveReaderPrefs(settings, v).catch(() => {});
  };

  /** Scroll so line `i` is in the middle of the view (line focus) or at its top. */
  const show = useCallback((i: number, list: readonly ReaderLine[], middle: boolean) => {
    const sc = scroller.current;
    const host = page.current;
    const l = list[i];
    if (!sc || !host || !l) return;
    const hostTop = host.getBoundingClientRect().top;
    const scTop = sc.getBoundingClientRect().top;
    sc.scrollTop += middle
      ? (l.top + l.bottom) / 2 + hostTop - (scTop + sc.clientHeight / 2)
      : l.top + hostTop - scTop - 24;
  }, []);

  // Lines of the reflowed text; the line being read keeps its text when the layout changes.
  const started = useRef(false);
  const measure = useCallback(() => {
    const host = page.current;
    if (!host) return;
    const base = host.getBoundingClientRect().top;
    const words: WordBox[] = [];
    host.querySelectorAll<HTMLElement>('[data-block]').forEach((b) => {
      const block = Number(b.dataset.block);
      b.querySelectorAll<HTMLElement>('[data-w]').forEach((w) => {
        const r = measureWord(w);
        words.push({
          block,
          offset: Number(w.dataset.w),
          top: r.top - base,
          bottom: r.bottom - base,
        });
      });
    });
    const next = groupLines(words);
    const before = live.current.lines[live.current.cur];
    let at: number;
    const first = !started.current;
    if (first) {
      at = Math.max(
        0,
        next.findIndex((l) => l.block >= start),
      );
      started.current = next.length > 0;
    } else {
      at = before ? lineOf(next, before.block, before.offset) : 0;
    }
    setLines(next);
    setLine(at);
    const focus = live.current.prefs.lineFocus > 0;
    const sc = scroller.current;
    const now = next[at];
    if (first || focus) show(at, next, focus);
    // The line being read stays where it was on the screen.
    else if (sc && before && now) sc.scrollTop += now.top - before.top;
  }, [measureWord, show, start]);

  useLayoutEffect(measure, [measure, blocks, prefs, focusMode]);
  useEffect(() => {
    window.addEventListener('resize', measure);
    const host = page.current;
    const ro =
      host && typeof ResizeObserver === 'function' ? new ResizeObserver(() => measure()) : null;
    // Late fonts change the width of the column's text.
    if (host && ro) ro.observe(host);
    return () => {
      window.removeEventListener('resize', measure);
      ro?.disconnect();
    };
  }, [measure]);

  const move = (to: number) => {
    const i = Math.min(Math.max(0, to), Math.max(0, lines.length - 1));
    setLine(i);
    show(i, lines, true);
  };

  /**
   * The current line follows scrolling: with line focus the line nearest the middle of the
   * view, else the first line in view.
   */
  const onScroll = () => {
    const sc = scroller.current;
    const host = page.current;
    if (!sc || !host || !lines.length) return;
    const top = sc.getBoundingClientRect().top - host.getBoundingClientRect().top;
    let best = 0;
    if (prefs.lineFocus) {
      const mid = top + sc.clientHeight / 2;
      let gap = Infinity;
      lines.forEach((l, i) => {
        const d = Math.abs((l.top + l.bottom) / 2 - mid);
        if (d < gap) {
          gap = d;
          best = i;
        }
      });
    } else {
      best = Math.max(
        0,
        lines.findIndex((l) => l.bottom > top),
      );
    }
    if (best !== cur) setLine(best);
  };

  /** The line reading starts at: the focused one, else the first line in view. */
  const currentLine = (): ReaderLine | undefined => lines[cur];

  const play = async () => {
    if (aloud.active) {
      aloud.stop();
      return;
    }
    const l = currentLine();
    await aloud.start({
      units: blocks.map((b) => ({ text: b.text, lang: b.lang })),
      unit: l?.block ?? 0,
      offset: l?.offset ?? 0,
      onWord: (at) => setSpoken(at),
      onStop: () => setSpoken(null),
    });
  };

  // The view follows the word being read.
  useEffect(() => {
    if (!spoken) return;
    const { lines: list, prefs: p } = live.current;
    if (p.lineFocus) {
      const i = lineOf(list, spoken.unit, spoken.start);
      setLine(i);
      show(i, list, true);
      return;
    }
    const el = page.current?.querySelector<HTMLElement>(
      `[data-block="${spoken.unit}"] [data-spoken]`,
    );
    el?.scrollIntoView?.({ block: 'nearest' });
  }, [spoken, show]);

  const exit = () => onExit(blocks[currentLine()?.block ?? start]);

  const onKeyDown = (e: ReactKeyboardEvent<HTMLDivElement>) => {
    // The editor's shortcuts stay off while the reader is open.
    e.stopPropagation();
    const target = e.target as HTMLElement;
    const field = target.tagName === 'SELECT' || target.tagName === 'INPUT';
    if (e.key === 'Escape') {
      e.preventDefault();
      exit();
      return;
    }
    if (e.key === 'Tab') {
      // Focus stays inside the reader.
      const items = Array.from(
        root.current?.querySelectorAll<HTMLElement>(
          'button:not(:disabled), select:not(:disabled), [tabindex="0"]',
        ) ?? [],
      );
      const first = items[0];
      const last = items[items.length - 1];
      if (e.shiftKey && document.activeElement === first) {
        e.preventDefault();
        last?.focus();
      } else if (!e.shiftKey && document.activeElement === last) {
        e.preventDefault();
        first?.focus();
      }
      return;
    }
    if (!prefs.lineFocus || field || e.ctrlKey || e.altKey || e.metaKey) return;
    const pageStep = Math.max(1, prefs.lineFocus);
    switch (e.key) {
      case 'ArrowDown':
        move(cur + 1);
        break;
      case 'ArrowUp':
        move(cur - 1);
        break;
      case 'PageDown':
        move(cur + pageStep);
        break;
      case 'PageUp':
        move(cur - pageStep);
        break;
      case 'Home':
        move(0);
        break;
      case 'End':
        move(lines.length - 1);
        break;
      default:
        return;
    }
    e.preventDefault();
  };

  const theme: ReaderTheme | 'hc' = highContrast ? 'hc' : prefs.theme;
  const palette = highContrast ? null : READER_PALETTES[prefs.theme];
  const vars = {
    ...(palette
      ? {
          '--r-bg': palette.bg,
          '--r-text': palette.text,
          '--r-muted': palette.muted,
          '--r-bar': palette.bar,
          '--r-border': palette.border,
          '--r-mark-bg': palette.markBg,
          '--r-mark-text': palette.markText,
          '--r-focus': palette.focus,
        }
      : {}),
    '--r-size': `${prefs.textSize}px`,
    '--r-line': String(prefs.lineSpacing),
    '--r-letter': prefs.wideSpacing ? '0.12em' : 'normal',
    '--r-word': prefs.wideSpacing ? '0.32em' : 'normal',
    '--r-column': `${COLUMN_CH[prefs.column]}ch`,
  } as CSSProperties;

  const win =
    prefs.lineFocus && lines.length ? lineWindow(lines.length, cur, prefs.lineFocus) : null;
  const winTop = win ? (lines[win.from]?.top ?? 0) - FOCUS_PAD : 0;
  const winBottom = win ? (lines[win.to]?.bottom ?? 0) + FOCUS_PAD : 0;
  const number = (n: number) => n.toLocaleString(i18n.language);

  return (
    <div
      ref={root}
      className={styles.root}
      role="dialog"
      aria-modal="true"
      aria-label={t('reader.title')}
      data-testid="focused-reader"
      data-theme={theme}
      data-focus-mode={focusMode ? 'true' : undefined}
      style={vars}
      onKeyDown={onKeyDown}
    >
      {focusMode ? (
        <button
          type="button"
          className={styles.showTools}
          aria-label={t('reader.showTools')}
          title={t('reader.showTools')}
          data-testid="reader-show-tools"
          onClick={() => setFocusMode(false)}
        >
          <PanelTopOpen size={16} aria-hidden />
        </button>
      ) : (
        <div className={styles.bar} role="toolbar" aria-label={t('reader.tools')}>
          <div className={styles.group} role="group" aria-label={t('reader.textSize')}>
            <button
              type="button"
              className={styles.button}
              aria-label={t('reader.smaller')}
              title={t('reader.smaller')}
              data-testid="reader-smaller"
              disabled={prefs.textSize <= TEXT_SIZE.min}
              onClick={() =>
                change({ textSize: Math.max(TEXT_SIZE.min, prefs.textSize - TEXT_SIZE.step) })
              }
            >
              <Minus size={16} aria-hidden />
            </button>
            <output className={styles.value} data-testid="reader-size" aria-live="polite">
              {t('reader.textSizeValue', { size: prefs.textSize })}
            </output>
            <button
              type="button"
              className={styles.button}
              aria-label={t('reader.larger')}
              title={t('reader.larger')}
              data-testid="reader-larger"
              disabled={prefs.textSize >= TEXT_SIZE.max}
              onClick={() =>
                change({ textSize: Math.min(TEXT_SIZE.max, prefs.textSize + TEXT_SIZE.step) })
              }
            >
              <Plus size={16} aria-hidden />
            </button>
          </div>
          <button
            type="button"
            className={styles.toggle}
            aria-pressed={prefs.wideSpacing}
            data-testid="reader-spacing"
            onClick={() => change({ wideSpacing: !prefs.wideSpacing })}
          >
            {t('reader.wideSpacing')}
          </button>
          <label className={styles.label}>
            {t('reader.lineSpacing')}
            <select
              className={styles.select}
              data-testid="reader-line-spacing"
              value={String(prefs.lineSpacing)}
              onChange={(e) => change({ lineSpacing: Number(e.target.value) })}
            >
              {LINE_SPACINGS.map((v) => (
                <option key={v} value={String(v)}>
                  {number(v)}
                </option>
              ))}
            </select>
          </label>
          <label className={styles.label}>
            {t('reader.column')}
            <select
              className={styles.select}
              data-testid="reader-column"
              value={prefs.column}
              onChange={(e) => change({ column: e.target.value as ReaderColumn })}
            >
              {READER_COLUMNS.map((v) => (
                <option key={v} value={v}>
                  {t(`reader.columns.${v}`)}
                </option>
              ))}
            </select>
          </label>
          <label className={styles.label}>
            {t('reader.theme')}
            <select
              className={styles.select}
              data-testid="reader-theme"
              value={prefs.theme}
              disabled={highContrast}
              title={highContrast ? t('reader.themeHighContrast') : undefined}
              onChange={(e) => change({ theme: e.target.value as ReaderTheme })}
            >
              {READER_THEMES.map((v) => (
                <option key={v} value={v}>
                  {t(`reader.themes.${v}`)}
                </option>
              ))}
            </select>
          </label>
          <label className={styles.label}>
            {t('reader.lineFocus')}
            <select
              className={styles.select}
              data-testid="reader-line-focus"
              value={String(prefs.lineFocus)}
              onChange={(e) => change({ lineFocus: Number(e.target.value) as LineFocus })}
            >
              {LINE_FOCUS.map((v) => (
                <option key={v} value={String(v)}>
                  {v === 0 ? t('reader.lineFocusOff') : t('reader.lineFocusLines', { count: v })}
                </option>
              ))}
            </select>
          </label>
          <button
            type="button"
            className={styles.toggle}
            aria-pressed={prefs.syllables}
            data-testid="reader-syllables"
            onClick={() => change({ syllables: !prefs.syllables })}
          >
            {t('reader.syllables')}
          </button>
          <button
            type="button"
            className={styles.toggle}
            aria-pressed={false}
            data-testid="reader-focus"
            onClick={() => {
              setFocusMode(true);
              scroller.current?.focus();
            }}
          >
            {t('reader.focus')}
          </button>
          <span className={styles.spacer} />
          <button
            type="button"
            className={styles.toggle}
            aria-pressed={aloud.active}
            data-testid="reader-play"
            disabled={!blocks.length}
            onClick={() => void play()}
          >
            {aloud.active ? <Square size={14} aria-hidden /> : <Play size={14} aria-hidden />}
            {aloud.active ? t('reader.stopReading') : t('reader.play')}
          </button>
          <button
            type="button"
            className={styles.button}
            aria-label={t('reader.close')}
            title={t('reader.close')}
            data-testid="reader-close"
            onClick={exit}
          >
            <X size={16} aria-hidden />
          </button>
        </div>
      )}
      <div
        ref={scroller}
        className={styles.scroller}
        tabIndex={0}
        role="document"
        aria-label={t('reader.text', { name })}
        data-testid="reader-text"
        data-line={lines.length ? cur : undefined}
        data-lines={lines.length}
        onScroll={onScroll}
      >
        <div
          ref={page}
          className={styles.page}
          data-line-focus={prefs.lineFocus ? prefs.lineFocus : undefined}
        >
          <div className={styles.column} data-testid="reader-column-box">
            {blocks.length ? (
              blocks.map((b, i) => (
                <Block
                  key={`${b.p}:${i}`}
                  index={i}
                  block={b}
                  marks={prefs.syllables}
                  spoken={spoken && spoken.unit === i ? spoken : null}
                />
              ))
            ) : (
              <p className={styles.empty} data-testid="reader-empty">
                {t('reader.empty')}
              </p>
            )}
          </div>
          {win && (
            <>
              <div
                className={styles.dim}
                style={{ top: 0, height: Math.max(0, winTop) }}
                data-testid="reader-dim-top"
                aria-hidden
              />
              <div
                className={styles.dim}
                style={{ top: winBottom, bottom: 0 }}
                data-testid="reader-dim-bottom"
                aria-hidden
              />
            </>
          )}
        </div>
      </div>
      {aloud.bar}
    </div>
  );
}
