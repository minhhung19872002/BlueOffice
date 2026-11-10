/**
 * Preferences and colour themes of the focused reader (CORE-LANG-004). The preferences are kept
 * per user like the other UI settings: through `ShellIntegration.getSetting` / `setSetting`
 * when the platform has them (desktop), else `localStorage`.
 */
import type { SettingStore } from '@blueoffice/ui';

export const READER_PREFS_KEY = 'blueoffice.focusedReader';

export const READER_THEMES = ['light', 'sepia', 'dark'] as const;
export type ReaderTheme = (typeof READER_THEMES)[number];

export const READER_COLUMNS = ['narrow', 'medium', 'wide'] as const;
export type ReaderColumn = (typeof READER_COLUMNS)[number];

/** Column width in `ch` (characters of the reading font). */
export const COLUMN_CH: Record<ReaderColumn, number> = { narrow: 42, medium: 62, wide: 86 };

export const LINE_SPACINGS = [1.2, 1.5, 1.8, 2.4] as const;
export const LINE_FOCUS = [0, 1, 3, 5] as const;
export type LineFocus = (typeof LINE_FOCUS)[number];

export const TEXT_SIZE = { min: 14, max: 48, step: 2 } as const;

export interface ReaderPrefs {
  /** Body text size (CSS px). */
  textSize: number;
  /** Wider letter and word spacing. */
  wideSpacing: boolean;
  /** Line height (multiple of the text size). */
  lineSpacing: number;
  column: ReaderColumn;
  theme: ReaderTheme;
  /** Lines kept bright (0 = line focus off). */
  lineFocus: LineFocus;
  /** Mark the syllables of words. */
  syllables: boolean;
}

export const DEFAULT_READER_PREFS: ReaderPrefs = {
  textSize: 22,
  wideSpacing: false,
  lineSpacing: 1.5,
  column: 'medium',
  theme: 'light',
  lineFocus: 0,
  syllables: false,
};

/** Colours of a reader theme (sRGB hex). */
export interface ReaderPalette {
  bg: string;
  text: string;
  /** Secondary text (position, syllable marks). */
  muted: string;
  /** Tool bar background. */
  bar: string;
  /** Borders of controls. */
  border: string;
  /** The word being read aloud. */
  markBg: string;
  markText: string;
  /** Keyboard focus ring. */
  focus: string;
}

/** Our own palettes; each pair used together meets WCAG 2.2 AA (see the test). */
export const READER_PALETTES: Record<ReaderTheme, ReaderPalette> = {
  light: {
    bg: '#ffffff',
    text: '#1b1b1b',
    muted: '#595959',
    bar: '#f1f1f1',
    border: '#6e6e6e',
    markBg: '#ffe58a',
    markText: '#1b1b1b',
    focus: '#1a4fa0',
  },
  sepia: {
    bg: '#f6eeda',
    text: '#33291a',
    muted: '#5c4a2e',
    bar: '#ece1c6',
    border: '#75633f',
    markBg: '#f0c95c',
    markText: '#2a2010',
    focus: '#7a4a00',
  },
  dark: {
    bg: '#141414',
    text: '#ececec',
    muted: '#b5b5b5',
    bar: '#222222',
    border: '#8a8a8a',
    markBg: '#6b5600',
    markText: '#ffffff',
    focus: '#ffd24a',
  },
};

/** Relative luminance of an `#rrggbb` colour (WCAG 2.2, "relative luminance"). */
export function luminance(hex: string): number {
  const v = /^#([0-9a-f]{6})$/i.exec(hex)?.[1];
  if (!v) return 0;
  const lin = (i: number) => {
    const c = parseInt(v.slice(i, i + 2), 16) / 255;
    return c <= 0.04045 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4;
  };
  return 0.2126 * lin(0) + 0.7152 * lin(2) + 0.0722 * lin(4);
}

/** Contrast ratio of two colours (WCAG 2.2, "contrast ratio"), 1–21. */
export function contrast(a: string, b: string): number {
  const [hi, lo] = [luminance(a), luminance(b)].sort((x, y) => y - x) as [number, number];
  return (hi + 0.05) / (lo + 0.05);
}

const oneOf = <T>(list: readonly T[], v: unknown, fallback: T): T =>
  list.includes(v as T) ? (v as T) : fallback;

/** Valid preferences from stored JSON (unknown or broken values fall back to the defaults). */
export function parseReaderPrefs(text: string | null): ReaderPrefs {
  const d = DEFAULT_READER_PREFS;
  if (!text) return d;
  let o: Partial<Record<keyof ReaderPrefs, unknown>>;
  try {
    const v: unknown = JSON.parse(text);
    if (typeof v !== 'object' || v === null || Array.isArray(v)) return d;
    o = v;
  } catch {
    return d;
  }
  const size =
    typeof o.textSize === 'number' && Number.isFinite(o.textSize)
      ? Math.min(TEXT_SIZE.max, Math.max(TEXT_SIZE.min, Math.round(o.textSize)))
      : d.textSize;
  return {
    textSize: size,
    wideSpacing: typeof o.wideSpacing === 'boolean' ? o.wideSpacing : d.wideSpacing,
    lineSpacing: oneOf<number>(LINE_SPACINGS, o.lineSpacing, d.lineSpacing),
    column: oneOf(READER_COLUMNS, o.column, d.column),
    theme: oneOf(READER_THEMES, o.theme, d.theme),
    lineFocus: oneOf(LINE_FOCUS, o.lineFocus, d.lineFocus),
    syllables: typeof o.syllables === 'boolean' ? o.syllables : d.syllables,
  };
}

export async function loadReaderPrefs(store: SettingStore): Promise<ReaderPrefs> {
  try {
    if (store.getSetting) return parseReaderPrefs(await store.getSetting(READER_PREFS_KEY));
    return parseReaderPrefs(globalThis.localStorage?.getItem(READER_PREFS_KEY) ?? null);
  } catch {
    return DEFAULT_READER_PREFS;
  }
}

export async function saveReaderPrefs(store: SettingStore, prefs: ReaderPrefs): Promise<void> {
  const text = JSON.stringify(prefs);
  if (store.setSetting) {
    await store.setSetting(READER_PREFS_KEY, text);
    return;
  }
  try {
    globalThis.localStorage?.setItem(READER_PREFS_KEY, text);
  } catch {
    // Storage unavailable: the preferences last for this session.
  }
}
