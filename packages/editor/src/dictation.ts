/**
 * Dictation core shared by BlueDoc, BlueSheet and BlueSlide (CORE-LANG-002). The platform's
 * `DictationProvider` reports interim / final results; this module turns a result into an edit:
 *
 * - voice commands, Vietnamese and English ("chấm", "dấu phẩy", "chấm hỏi", "xuống dòng",
 *   "mở ngoặc kép", "xoá từ vừa rồi" … / "period", "comma", "new line", "delete last word" …),
 *   matched longest first, case-insensitively, whatever spelling of the tone mark the recogniser
 *   uses ("xoá" / "xóa");
 * - spacing: words separated by one space; `. , ; : ! ? …` and closing brackets / quotes stick
 *   to the word before them, opening ones to the word after (the same rule for Vietnamese and
 *   English: no space before those marks, one after);
 * - capitals: the first letter of a sentence — at the start of the text, after a line break or
 *   after `. ! ? …` — is upper-cased.
 *
 * A final phrase becomes one edit of the target — replace `remove` UTF-16 units before the caret
 * (a deleted word) and the selection with `text` — so one phrase is one undo step. Interim
 * results are only shown (`DictationTarget.preview`), never written.
 */
import {
  primaryLang,
  type DictationErrorCode,
  type DictationProvider,
  type DictationSession as ProviderSession,
} from '@blueoffice/platform';

/** A unit of dictated text. */
export type DictationToken =
  | { kind: 'word'; text: string }
  /** `. , ; : ! ? …`: no space before. */
  | { kind: 'punct'; text: string }
  /** Opening bracket / quote: no space after. */
  | { kind: 'open'; text: string }
  /** Closing bracket / quote: no space before. */
  | { kind: 'close'; text: string }
  | { kind: 'newline' }
  | { kind: 'paragraph' }
  | { kind: 'deleteWord' };

type Command = Exclude<DictationToken, { kind: 'word' }>;

const P = (text: string): Command => ({ kind: 'punct', text });
const OPEN_PAREN: Command = { kind: 'open', text: '(' };
const CLOSE_PAREN: Command = { kind: 'close', text: ')' };
const OPEN_QUOTE: Command = { kind: 'open', text: '“' };
const CLOSE_QUOTE: Command = { kind: 'close', text: '”' };
const NEWLINE: Command = { kind: 'newline' };
const PARAGRAPH: Command = { kind: 'paragraph' };
const DELETE_WORD: Command = { kind: 'deleteWord' };

/** Voice commands per language (primary subtag): phrase → command. */
export const DICTATION_COMMANDS: Record<string, [string, Command][]> = {
  vi: [
    ['chấm', P('.')],
    ['dấu chấm', P('.')],
    ['phẩy', P(',')],
    ['dấu phẩy', P(',')],
    ['chấm hỏi', P('?')],
    ['dấu chấm hỏi', P('?')],
    ['dấu hỏi', P('?')],
    ['chấm than', P('!')],
    ['dấu chấm than', P('!')],
    ['hai chấm', P(':')],
    ['dấu hai chấm', P(':')],
    ['chấm phẩy', P(';')],
    ['dấu chấm phẩy', P(';')],
    ['ba chấm', P('…')],
    ['dấu ba chấm', P('…')],
    ['xuống dòng', NEWLINE],
    ['dòng mới', NEWLINE],
    ['đoạn mới', PARAGRAPH],
    ['sang đoạn mới', PARAGRAPH],
    ['mở ngoặc', OPEN_PAREN],
    ['mở ngoặc đơn', OPEN_PAREN],
    ['đóng ngoặc', CLOSE_PAREN],
    ['đóng ngoặc đơn', CLOSE_PAREN],
    ['mở ngoặc kép', OPEN_QUOTE],
    ['đóng ngoặc kép', CLOSE_QUOTE],
    ['xoá từ vừa rồi', DELETE_WORD],
    ['xoá từ vừa nói', DELETE_WORD],
    ['xoá từ cuối', DELETE_WORD],
  ],
  en: [
    ['period', P('.')],
    ['full stop', P('.')],
    ['comma', P(',')],
    ['question mark', P('?')],
    ['exclamation mark', P('!')],
    ['exclamation point', P('!')],
    ['colon', P(':')],
    ['semicolon', P(';')],
    ['semi colon', P(';')],
    ['ellipsis', P('…')],
    ['dot dot dot', P('…')],
    ['new line', NEWLINE],
    ['next line', NEWLINE],
    ['new paragraph', PARAGRAPH],
    ['next paragraph', PARAGRAPH],
    ['open parenthesis', OPEN_PAREN],
    ['open paren', OPEN_PAREN],
    ['open bracket', OPEN_PAREN],
    ['left parenthesis', OPEN_PAREN],
    ['close parenthesis', CLOSE_PAREN],
    ['close paren', CLOSE_PAREN],
    ['close bracket', CLOSE_PAREN],
    ['right parenthesis', CLOSE_PAREN],
    ['open quote', OPEN_QUOTE],
    ['begin quote', OPEN_QUOTE],
    ['close quote', CLOSE_QUOTE],
    ['end quote', CLOSE_QUOTE],
    ['unquote', CLOSE_QUOTE],
    ['delete last word', DELETE_WORD],
    ['delete previous word', DELETE_WORD],
  ],
};

// Old / new placement of the tone mark on "oa", "oe", "uy" (both spellings are in use).
const TONE_PLACEMENT: [RegExp, string][] = [
  [/óa/g, 'oá'],
  [/òa/g, 'oà'],
  [/ỏa/g, 'oả'],
  [/õa/g, 'oã'],
  [/ọa/g, 'oạ'],
  [/óe/g, 'oé'],
  [/òe/g, 'oè'],
  [/ỏe/g, 'oẻ'],
  [/õe/g, 'oẽ'],
  [/ọe/g, 'oẹ'],
  [/úy/g, 'uý'],
  [/ùy/g, 'uỳ'],
  [/ủy/g, 'uỷ'],
  [/ũy/g, 'uỹ'],
  [/ụy/g, 'uỵ'],
];

/** A word as commands compare it: NFC, lower case, one tone-mark placement. */
export function normalizeWord(w: string, lang = 'vi'): string {
  let s = w.normalize('NFC').toLocaleLowerCase(lang);
  for (const [re, to] of TONE_PLACEMENT) s = s.replace(re, to);
  return s;
}

const PUNCT_CHARS = new Set(['.', ',', ';', ':', '!', '?', '…']);
const OPEN_CHARS = new Set(['(', '[', '{', '“', '‘', '«']);
const CLOSE_CHARS = new Set([')', ']', '}', '”', '’', '»']);

/** Splits a recognised word into marks before, the word, marks after (`"chào,"` → `chào` `,`). */
function splitMarks(raw: string): DictationToken[] {
  const out: DictationToken[] = [];
  let a = 0;
  let b = raw.length;
  const lead: DictationToken[] = [];
  const trail: DictationToken[] = [];
  while (a < b && OPEN_CHARS.has(raw[a]!)) lead.push({ kind: 'open', text: raw[a++]! });
  while (b > a) {
    const c = raw[b - 1]!;
    if (PUNCT_CHARS.has(c)) trail.unshift({ kind: 'punct', text: c });
    else if (CLOSE_CHARS.has(c)) trail.unshift({ kind: 'close', text: c });
    else break;
    b--;
  }
  // "..." typed by the recogniser is an ellipsis.
  const core = raw.slice(a, b);
  out.push(...lead);
  if (core) out.push({ kind: 'word', text: core });
  const merged: DictationToken[] = [];
  for (const t of trail) {
    const last = merged.at(-1);
    if (t.kind === 'punct' && t.text === '.' && last?.kind === 'punct' && /^\.+$/.test(last.text))
      last.text += '.';
    else merged.push({ ...t });
  }
  for (const t of merged) out.push(t.kind === 'punct' && t.text === '...' ? P('…') : t);
  return out;
}

const tables = new Map<string, { words: string[]; cmd: Command }[]>();

function table(lang: string): { words: string[]; cmd: Command }[] {
  const key = DICTATION_COMMANDS[lang] ? lang : 'en';
  let t = tables.get(key);
  if (!t) {
    t = DICTATION_COMMANDS[key]!.map(([phrase, cmd]) => ({
      words: phrase.split(' ').map((w) => normalizeWord(w, key)),
      cmd,
    })).sort((x, y) => y.words.length - x.words.length);
    tables.set(key, t);
  }
  return t;
}

/** Tokens of a recognised phrase in `lang` (BCP 47): words, marks and voice commands. */
export function parseDictation(text: string, lang: string): DictationToken[] {
  const key = primaryLang(lang);
  const raw = text.split(/\s+/).filter(Boolean).flatMap(splitMarks);
  const cmds = table(key);
  const out: DictationToken[] = [];
  for (let i = 0; i < raw.length;) {
    let hit: { n: number; cmd: Command } | null = null;
    for (const c of cmds) {
      const n = c.words.length;
      if (i + n > raw.length) continue;
      let ok = true;
      for (let k = 0; k < n; k++) {
        const t = raw[i + k]!;
        if (t.kind !== 'word' || normalizeWord(t.text, key) !== c.words[k]) {
          ok = false;
          break;
        }
      }
      if (ok) {
        hit = { n, cmd: c.cmd };
        break;
      }
    }
    if (hit) {
      out.push({ ...hit.cmd });
      i += hit.n;
      // The recogniser's own mark right after a spoken one ("chấm." → one period).
      const next = raw[i];
      if (hit.cmd.kind === 'punct' && next?.kind === 'punct') i++;
    } else {
      out.push(raw[i]!);
      i++;
    }
  }
  return out;
}

/** How the target writes breaks. */
export interface DictationBreaks {
  /** A line break inside the paragraph ("xuống dòng"). */
  lineBreak: string;
  /** A new paragraph ("đoạn mới"). */
  paragraph: string;
}

const isHSpace = (c: string) => c === ' ' || c === '\t' || c === ' ';

function trimHSpace(s: string): string {
  let n = s.length;
  while (n > 0 && isHSpace(s[n - 1]!)) n--;
  return s.slice(0, n);
}

const WORD_CHAR = /[\p{L}\p{N}\p{M}'’_-]/u;

/** Does a sentence start after `s`? */
export function sentenceStart(s: string, breaks: readonly string[]): boolean {
  let t = trimHSpace(s);
  while (t && OPEN_CHARS.has(t.at(-1)!)) t = trimHSpace(t.slice(0, -1));
  if (!t) return true;
  const last = t.at(-1)!;
  if (breaks.includes(last) || last === '\n' || last === '\r' || last === '\v') return true;
  let u = t;
  while (u && CLOSE_CHARS.has(u.at(-1)!)) u = u.slice(0, -1);
  return /[.!?…]$/u.test(u);
}

function capitalize(w: string, lang: string): string {
  const cp = w.codePointAt(0);
  if (cp === undefined) return w;
  const first = String.fromCodePoint(cp);
  return first.toLocaleUpperCase(lang) + w.slice(first.length);
}

export interface ComposeOptions extends DictationBreaks {
  lang: string;
  /** Upper-case sentence starts (default true). */
  capitalize?: boolean;
}

/** The edit of one phrase: remove `remove` UTF-16 units before the caret, then insert `text`. */
export interface DictationEdit {
  remove: number;
  text: string;
}

/** `before` (text before the caret in the paragraph / cell) after applying `tokens`. */
export function applyTokens(
  before: string,
  tokens: readonly DictationToken[],
  o: ComposeOptions,
): string {
  const breaks = [o.lineBreak, o.paragraph].filter((b) => b.length === 1);
  const isBreak = (c: string | undefined) =>
    c !== undefined && (breaks.includes(c) || c === '\n' || c === '\v');
  const caps = o.capitalize !== false;
  let s = before;
  const needsSpace = () => {
    const last = s.at(-1);
    return last !== undefined && !/\s/u.test(last) && !OPEN_CHARS.has(last) && !isBreak(last);
  };
  for (const t of tokens) {
    switch (t.kind) {
      case 'word': {
        const start = caps && sentenceStart(s, breaks);
        if (needsSpace()) s += ' ';
        s += start ? capitalize(t.text, o.lang) : t.text;
        break;
      }
      case 'open':
        if (needsSpace()) s += ' ';
        s += t.text;
        break;
      case 'punct':
      case 'close':
        s = trimHSpace(s) + t.text;
        break;
      case 'newline':
        s = trimHSpace(s) + o.lineBreak;
        break;
      case 'paragraph':
        s = trimHSpace(s) + o.paragraph;
        break;
      case 'deleteWord': {
        let t2 = trimHSpace(s);
        const last = t2.at(-1);
        if (last === undefined || isBreak(last)) {
          s = t2;
          break;
        }
        if (WORD_CHAR.test(last)) {
          while (t2 && WORD_CHAR.test(t2.at(-1)!)) t2 = t2.slice(0, -1);
          // A low surrogate's pair goes with it.
        } else {
          const cp = t2.codePointAt(t2.length - 2);
          t2 = t2.slice(0, cp !== undefined && cp > 0xffff ? -2 : -1);
        }
        s = trimHSpace(t2);
        break;
      }
    }
  }
  return s;
}

/** The edit turning `before` into `after` (common prefix kept, never splitting a pair). */
export function diffEdit(before: string, after: string): DictationEdit {
  let k = 0;
  const n = Math.min(before.length, after.length);
  while (k < n && before.charCodeAt(k) === after.charCodeAt(k)) k++;
  if (k > 0 && k < before.length) {
    const c = before.charCodeAt(k - 1);
    if (c >= 0xd800 && c <= 0xdbff) k--;
  }
  return { remove: before.length - k, text: after.slice(k) };
}

/**
 * The edit of a phrase at a caret with `before` / `after` around it: voice commands, spacing
 * and capitals applied; a space is added before a word that directly follows.
 */
export function composeDictation(
  before: string,
  after: string,
  text: string,
  o: ComposeOptions,
): DictationEdit {
  const tokens = parseDictation(text, o.lang);
  const next = applyTokens(before, tokens, o);
  const edit = diffEdit(before, next);
  const first = after.codePointAt(0);
  if (
    edit.text &&
    first !== undefined &&
    WORD_CHAR.test(String.fromCodePoint(first)) &&
    !/\s$/u.test(edit.text) &&
    !OPEN_CHARS.has(edit.text.at(-1)!)
  )
    edit.text += ' ';
  return edit;
}

// ------------------------------------------------------------------------------ session

/** Where dictated text goes (the caret of the editor). */
export interface DictationTarget extends DictationBreaks {
  /** Text before the caret (the selection start) and after it in the paragraph / cell; `null`: nowhere to write. */
  context(): Promise<{ before: string; after: string } | null>;
  /** Replaces the selection and `remove` UTF-16 units before it by `text`, as one undo step. */
  apply(edit: DictationEdit): Promise<void>;
  /** Shows the interim text at the caret, not in the document (`null` hides it). */
  preview(text: string | null): void;
}

export type DictationState = 'idle' | 'starting' | 'listening' | 'paused' | 'error';

export interface DictationListener {
  onState?(state: DictationState): void;
  onError?(code: DictationErrorCode | 'noTarget'): void;
  /** The interim text (`''`: none). */
  onInterim?(text: string): void;
  /** A final phrase was written. */
  onPhrase?(text: string, edit: DictationEdit): void;
}

/**
 * One dictation run: listens through the provider, previews interim results, writes each final
 * phrase (in order) to the target. `pause` stops listening and keeps the run; `stop` ends it
 * (results still being recognised are written).
 */
export class DictationSession {
  /** The provider session listening now (a holder: events may come before `start` returns). */
  private run: { s: ProviderSession | null } | null = null;
  private stateValue: DictationState = 'idle';
  private writes: Promise<void> = Promise.resolve();
  /** Text before the caret after the last write (interim previews compose against it). */
  private lastBefore = '';
  private lastAfter = '';
  private disposed = false;

  constructor(
    private readonly provider: DictationProvider,
    private readonly target: DictationTarget,
    private langValue: string,
    private readonly listener: DictationListener = {},
  ) {}

  get state(): DictationState {
    return this.stateValue;
  }

  get lang(): string {
    return this.langValue;
  }

  private set(state: DictationState): void {
    if (this.stateValue === state) return;
    this.stateValue = state;
    this.listener.onState?.(state);
  }

  private compose(before: string, after: string, text: string): DictationEdit {
    return composeDictation(before, after, text, {
      lang: this.langValue,
      lineBreak: this.target.lineBreak,
      paragraph: this.target.paragraph,
    });
  }

  /** Starts (or resumes) listening. */
  async start(): Promise<void> {
    if (this.disposed || this.run) return;
    const ctx = await this.target.context();
    if (this.disposed) return;
    if (!ctx) {
      this.set('error');
      this.listener.onError?.('noTarget');
      return;
    }
    this.lastBefore = ctx.before;
    this.lastAfter = ctx.after;
    this.set('starting');
    const run: { s: ProviderSession | null } = { s: null };
    this.run = run;
    run.s = this.provider.start(this.langValue, { interim: true }, (e) => {
      if (this.disposed) return;
      switch (e.type) {
        case 'start':
          if (this.run === run) this.set('listening');
          break;
        case 'result':
          if (e.final) this.final(e.text);
          else if (this.run === run) this.interim(e.text);
          break;
        case 'end':
          this.clearPreview();
          if (this.run === run) {
            this.run = null;
            if (this.stateValue !== 'paused') this.set('idle');
          }
          break;
        case 'error':
          this.clearPreview();
          if (this.run === run) this.run = null;
          this.set('error');
          this.listener.onError?.(e.code);
          break;
      }
    });
  }

  private interim(text: string): void {
    const t = text.trim();
    if (!t) {
      this.clearPreview();
      return;
    }
    const e = this.compose(this.lastBefore, this.lastAfter, t);
    this.target.preview(e.text || null);
    this.listener.onInterim?.(e.text);
  }

  private clearPreview(): void {
    this.target.preview(null);
    this.listener.onInterim?.('');
  }

  private final(text: string): void {
    this.clearPreview();
    const t = text.trim();
    if (!t) return;
    this.writes = this.writes.then(async () => {
      if (this.disposed) return;
      const ctx = await this.target.context();
      if (!ctx || this.disposed) return;
      const e = this.compose(ctx.before, ctx.after, t);
      if (!e.remove && !e.text) return;
      await this.target.apply(e);
      this.lastBefore = ctx.before.slice(0, ctx.before.length - e.remove) + e.text;
      this.lastAfter = ctx.after;
      this.listener.onPhrase?.(t, e);
    });
  }

  /** Every final phrase received so far is written. */
  written(): Promise<void> {
    return this.writes;
  }

  pause(): void {
    if (this.stateValue !== 'listening' && this.stateValue !== 'starting') return;
    this.set('paused');
    const r = this.run;
    this.run = null;
    r?.s?.stop();
    this.clearPreview();
  }

  async resume(): Promise<void> {
    if (this.stateValue !== 'paused' && this.stateValue !== 'error') return;
    await this.start();
  }

  async toggle(): Promise<void> {
    if (this.stateValue === 'paused' || this.stateValue === 'error') await this.resume();
    else this.pause();
  }

  /** Listens in another language from now on. */
  async setLanguage(lang: string): Promise<void> {
    if (lang === this.langValue) return;
    this.langValue = lang;
    if (this.stateValue === 'listening' || this.stateValue === 'starting') {
      const r = this.run;
      this.run = null;
      r?.s?.stop();
      await this.start();
    }
  }

  /** Ends the run; results still being recognised are written. */
  stop(): void {
    const r = this.run;
    this.run = null;
    r?.s?.stop();
    this.clearPreview();
    this.set('idle');
  }

  dispose(): void {
    const r = this.run;
    this.run = null;
    this.disposed = true;
    r?.s?.abort();
    this.target.preview(null);
    this.stateValue = 'idle';
  }
}
