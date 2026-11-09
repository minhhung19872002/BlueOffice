import { BLOCKS, WORDS } from './data';
import { VI_KEYWORDS } from './keywords';

/** One character of the picker. */
export interface SymbolEntry {
  cp: number;
  char: string;
  /** Unicode character name (English, from UnicodeData.txt). */
  name: string;
  /** Group id (`data.ts` block id or `vietnamese`). */
  block: string;
}

export interface SymbolBlock {
  id: string;
  /** English name (the UCD block name); the UI may translate it by `id`. */
  name: string;
  entries: SymbolEntry[];
}

/** Virtual group: the letters of the Vietnamese alphabet with every tone mark. */
export const VIETNAMESE_BLOCK = 'vietnamese';

let blocks: SymbolBlock[] | null = null;
let byCp: Map<number, SymbolEntry> | null = null;

function build(): void {
  const words = WORDS.split(' ');
  const map = new Map<number, SymbolEntry>();
  const list: SymbolBlock[] = [];
  for (const b of BLOCKS) {
    const entries: SymbolEntry[] = [];
    b.names.split('|').forEach((enc, i) => {
      if (!enc) return;
      const cp = b.start + i;
      const name = enc
        .split('.')
        .map((w) => words[parseInt(w, 36)] ?? '')
        .join(' ');
      const e = { cp, char: String.fromCodePoint(cp), name, block: b.id };
      entries.push(e);
      map.set(cp, e);
    });
    list.push({ id: b.id, name: b.name, entries });
  }
  // Vietnamese letters: each vowel with no tone and the five tone marks (NFC), then đ.
  const vowels = ['a', 'ă', 'â', 'e', 'ê', 'i', 'o', 'ô', 'ơ', 'u', 'ư', 'y'];
  const tones = ['', '̀', '́', '̉', '̃', '̣'];
  const vi: SymbolEntry[] = [];
  const add = (s: string) => {
    const cp = s.normalize('NFC').codePointAt(0);
    const e = cp === undefined ? undefined : map.get(cp);
    if (e) vi.push({ ...e, block: VIETNAMESE_BLOCK });
  };
  for (const upper of [false, true]) {
    for (const v of vowels) {
      for (const tone of tones) add((upper ? v.toUpperCase() : v) + tone);
    }
    add(upper ? 'Đ' : 'đ');
  }
  list.unshift({ id: VIETNAMESE_BLOCK, name: 'Vietnamese letters', entries: vi });
  blocks = list;
  byCp = map;
}

/** Groups of the picker, the Vietnamese letters first. */
export function symbolBlocks(): readonly SymbolBlock[] {
  if (!blocks) build();
  return blocks ?? [];
}

/** The entry of a code point, when it is in the picker's set. */
export function symbolEntry(cp: number): SymbolEntry | undefined {
  if (!byCp) build();
  return byCp?.get(cp);
}

/** `U+20AB` notation. */
export function formatCodePoint(cp: number): string {
  return `U+${cp.toString(16).toUpperCase().padStart(4, '0')}`;
}

/** Lower case without diacritics (đ → d): "Mũi Tên" → "mui ten". */
export function foldText(s: string): string {
  return s
    .normalize('NFD')
    .replace(/\p{M}/gu, '')
    .replace(/[đĐ]/g, 'd')
    .toLowerCase()
    .replace(/\s+/g, ' ')
    .trim();
}

const CODE_POINT = /^(u\+|0x|\\u)?([0-9a-f]{2,6})$/i;

/**
 * Searches the picker's characters: a code point ("U+20AB", "20ab", "0x20AB"), the
 * character itself, Vietnamese keywords ("đồng", "mui ten") and English name words
 * ("dong sign", "arrow right"). Best matches first, at most `limit` results.
 */
export function searchSymbols(query: string, limit = 400): SymbolEntry[] {
  const q = query.trim();
  if (!q) return [];
  const out: SymbolEntry[] = [];
  const seen = new Set<number>();
  const push = (e: SymbolEntry | undefined) => {
    if (e && !seen.has(e.cp) && out.length < limit) {
      seen.add(e.cp);
      out.push(e);
    }
  };

  const hex = CODE_POINT.exec(q);
  if (hex?.[2]) {
    push(symbolEntry(parseInt(hex[2], 16)));
    if (hex[1]) return out; // an explicit code point notation
  }
  const chars = [...q];
  if (chars.length === 1) push(symbolEntry(q.codePointAt(0) ?? -1));

  // Vietnamese keywords: their characters first, their groups after the name matches.
  const folded = foldText(q);
  const groups: string[] = [];
  for (const k of VI_KEYWORDS) {
    const hit = k.words.some((w) => {
      const f = foldText(w);
      return f === folded || (folded.length >= 3 && f.startsWith(folded));
    });
    if (!hit) continue;
    for (const c of k.chars ?? '') push(symbolEntry(c.codePointAt(0) ?? -1));
    groups.push(...(k.blocks ?? []));
  }

  // English names: every query word must start a word of the name.
  const terms = folded.split(' ').filter(Boolean);
  if (terms.length > 0 && /[a-z]/.test(folded)) {
    const scored: { e: SymbolEntry; score: number }[] = [];
    for (const b of symbolBlocks()) {
      if (b.id === VIETNAMESE_BLOCK) continue;
      for (const e of b.entries) {
        const name = e.name.toLowerCase();
        const nameWords = name.split(/[ -]/);
        if (!terms.every((t) => nameWords.some((w) => w.startsWith(t)))) continue;
        let score = nameWords.length;
        if (name === folded) score -= 100;
        else if (terms.every((t) => nameWords.includes(t))) score -= 10;
        scored.push({ e, score });
      }
    }
    scored.sort((a, b) => a.score - b.score || a.e.cp - b.e.cp);
    for (const s of scored) push(s.e);
  }

  for (const id of groups) {
    for (const e of symbolBlocks().find((b) => b.id === id)?.entries ?? []) push(e);
  }
  return out;
}

/** Characters that do not show a glyph of their own (spaces, format controls). */
export function isInvisible(char: string): boolean {
  return /^[\p{Z}\p{Cf}]$/u.test(char);
}

/** Entries of the "special characters" tab; labels come from the host (i18n). */
export const SPECIAL_CHARACTERS = [
  { id: 'emDash', char: '—' },
  { id: 'enDash', char: '–' },
  { id: 'nbsp', char: ' ' },
  { id: 'narrowNbsp', char: ' ' },
  { id: 'nbHyphen', char: '‑' },
  { id: 'softHyphen', char: '­' },
  { id: 'zwsp', char: '​' },
  { id: 'zwnj', char: '‌' },
  { id: 'zwj', char: '‍' },
  { id: 'emSpace', char: ' ' },
  { id: 'enSpace', char: ' ' },
  { id: 'thinSpace', char: ' ' },
  { id: 'copyright', char: '©' },
  { id: 'registered', char: '®' },
  { id: 'trademark', char: '™' },
  { id: 'section', char: '§' },
  { id: 'paragraph', char: '¶' },
  { id: 'ellipsis', char: '…' },
  { id: 'openDoubleQuote', char: '“' },
  { id: 'closeDoubleQuote', char: '”' },
  { id: 'openSingleQuote', char: '‘' },
  { id: 'closeSingleQuote', char: '’' },
  { id: 'degree', char: '°' },
  { id: 'bullet', char: '•' },
] as const;

export type SpecialCharacterId = (typeof SPECIAL_CHARACTERS)[number]['id'];

// ------------------------------------------------------------------ recent

export const RECENT_KEY = 'bo.symbols.recent';
export const RECENT_MAX = 24;

function storage(): Storage | null {
  try {
    return typeof localStorage === 'undefined' ? null : localStorage;
  } catch {
    return null;
  }
}

/** Recently inserted characters, newest first (empty when storage is unavailable). */
export function loadRecentSymbols(): string[] {
  try {
    const raw = storage()?.getItem(RECENT_KEY);
    const v: unknown = raw ? JSON.parse(raw) : [];
    return Array.isArray(v)
      ? v.filter((x): x is string => typeof x === 'string' && x.length > 0).slice(0, RECENT_MAX)
      : [];
  } catch {
    return [];
  }
}

/**
 * Moves `char` to the front of the recent list (`current`, else the stored one) and
 * saves it; returns the new list.
 */
export function pushRecentSymbol(char: string, current = loadRecentSymbols()): string[] {
  const next = [char, ...current.filter((c) => c !== char)].slice(0, RECENT_MAX);
  try {
    storage()?.setItem(RECENT_KEY, JSON.stringify(next));
  } catch {
    // Storage full or blocked: the list lives for this session only.
  }
  return next;
}
