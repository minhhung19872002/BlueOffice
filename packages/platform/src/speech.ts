/**
 * Text to speech for read aloud (CORE-LANG-003). The platform speaks; editors only hand it text
 * and follow its word boundaries. The web and the desktop webview (WebView2 / WebKitGTK) speak
 * through the W3C Web Speech API (`speechSynthesis`, `SpeechSynthesisUtterance` and its
 * `boundary` events — https://webaudio.github.io/web-speech-api/); the voices are the ones the
 * browser / operating system has installed. `FakeSpeechProvider` emits boundaries
 * deterministically for tests.
 *
 * Offsets of speech events are UTF-16 code units of the spoken string (what `charIndex` /
 * `charLength` of `SpeechSynthesisEvent` count); BlueDoc positions are UTF-8 byte offsets —
 * `utf16ToByte` / `byteToUtf16` convert.
 */

/** An installed voice. */
export interface SpeechVoice {
  /** Stable id (`voiceURI`). */
  id: string;
  name: string;
  /** BCP 47 language tag (`vi-VN`, `en-US`). */
  lang: string;
  /** Speaks without a network service. */
  local: boolean;
  /** The platform's default voice. */
  default: boolean;
}

/** How to speak one utterance. */
export interface SpeakOptions {
  /** Voice id (absent: the platform's choice for `lang`). */
  voice?: string | null;
  /** Language of the text (BCP 47). */
  lang?: string;
  /** 0.1–10, 1 = normal. */
  rate?: number;
  /** 0–2, 1 = normal. */
  pitch?: number;
  /** 0–1. */
  volume?: number;
}

/** Progress of an utterance; offsets in UTF-16 code units of the spoken text. */
export type SpeechEvent =
  | { type: 'start' }
  | { type: 'word'; offset: number; length: number }
  | { type: 'sentence'; offset: number; length: number }
  | { type: 'pause' }
  | { type: 'resume' }
  | { type: 'end' }
  | { type: 'error'; error: string };

/** A spoken utterance. */
export interface SpeechUtterance {
  pause(): void;
  resume(): void;
  /** Stops speaking (no `end` event follows). */
  cancel(): void;
}

/** Text to speech of the platform. */
export interface SpeechProvider {
  /** Installed voices (may wait briefly for the platform to list them). */
  voices(): Promise<SpeechVoice[]>;
  speak(text: string, options: SpeakOptions, onEvent: (e: SpeechEvent) => void): SpeechUtterance;
  /** The voice list changed (voices installed / loaded late). */
  onVoicesChanged?(listener: () => void): () => void;
}

// ----------------------------------------------------------- BlueDoc engine queries (bo_engine::speech)

/** Read-aloud queries of BlueDoc; they never change the document or the selection. */
export type DocSpeechCommand =
  /** Body paragraphs `from..=to` (absent: all) → `speech.paras`. */
  | { cmd: 'speechText'; from?: number; to?: number }
  /** Highlight rectangles of byte ranges of body paragraphs → `speech.rects` (one list per range). */
  | { cmd: 'speechRects'; ranges: { p: number; start: number; end: number }[] }
  /**
   * Accessible structure of the body paragraphs on pages `fromPage..=toPage` (0-based) and of
   * paragraph `include` → `speech.tree` (screen-reader mirror of the canvas).
   */
  | { cmd: 'speechTree'; fromPage: number; toPage: number; include?: number }
  /** Byte range of the laid-out line showing byte `o` of paragraph `p` → `speech.line`. */
  | { cmd: 'speechLine'; p: number; o: number };

/** A body paragraph of the accessible structure (offsets: UTF-8 bytes of `text`). */
export interface DocTreePara {
  p: number;
  /** Index text (objects are U+FFFC). */
  text: string;
  /** Heading level 1–9. */
  heading?: number;
  /** List level 1–9. */
  list?: number;
  /** Table cell holding the paragraph (outermost table). */
  cell?: {
    /** Paragraphs with the same key are in one table. */
    table: string;
    row: number;
    col: number;
    rows: number;
    cols: number;
    /** Header row: its cells are column headers. */
    header?: boolean;
  };
  links?: { start: number; end: number; href: string }[];
  /** Objects (U+FFFC) and their text alternatives. */
  objects?: { at: number; kind: 'picture' | 'equation' | 'object'; alt: string }[];
  /** First page showing the paragraph (0-based). */
  page: number;
}

/** A body paragraph to read (offsets: UTF-8 bytes of `text`, as `Pos`). */
export interface DocSpeechPara {
  p: number;
  /** Index text (objects are U+FFFC). */
  text: string;
  /** Resolved language of its text (`w:lang`). */
  lang?: string;
  /** Language the text reads as (`vi-VN`, `en`…). */
  detected?: string;
  /** Byte ranges not to read (hidden or deleted text). */
  skip?: [number, number][];
}

export interface DocSpeechInfo {
  paras?: DocSpeechPara[];
  rects?: import('./types').SelRect[][];
  tree?: DocTreePara[];
  /** `[start, end)` byte offsets of the line (`speechLine`). */
  line?: [number, number];
}

// ------------------------------------------------------------------------------- text helpers

/** UTF-8 byte offset of UTF-16 offset `u16` in `text` (clamped; never inside a character). */
export function utf16ToByte(text: string, u16: number): number {
  let bytes = 0;
  let i = 0;
  while (i < text.length && i < u16) {
    const cp = text.codePointAt(i) ?? 0;
    const units = cp > 0xffff ? 2 : 1;
    if (i + units > u16) break;
    bytes += cp < 0x80 ? 1 : cp < 0x800 ? 2 : cp < 0x10000 ? 3 : 4;
    i += units;
  }
  return bytes;
}

/** UTF-16 offset of UTF-8 byte offset `byte` in `text` (clamped; never inside a character). */
export function byteToUtf16(text: string, byte: number): number {
  let bytes = 0;
  let i = 0;
  while (i < text.length) {
    const cp = text.codePointAt(i) ?? 0;
    const n = cp < 0x80 ? 1 : cp < 0x800 ? 2 : cp < 0x10000 ? 3 : 4;
    if (bytes + n > byte) break;
    bytes += n;
    i += cp > 0xffff ? 2 : 1;
  }
  return i;
}

/** A range of UTF-16 code units, end exclusive. */
export interface TextRange {
  start: number;
  end: number;
}

// Sentence ends: terminal punctuation (with closing quotes / brackets) then white space; line,
// page and column breaks also end a stretch of speech. Our own rule, deterministic everywhere.
const SENTENCE_END = /[.!?…]+["'”’»)\]]*\s+|[\n\p{Zl}\p{Zp}\v\f]+/gu;

/** Sentences of `text` (each keeps its trailing white space); blank text → `[]`. */
export function splitSentences(text: string): TextRange[] {
  const out: TextRange[] = [];
  let start = 0;
  SENTENCE_END.lastIndex = 0;
  for (let m = SENTENCE_END.exec(text); m; m = SENTENCE_END.exec(text)) {
    const end = m.index + m[0].length;
    if (text.slice(start, end).trim()) out.push({ start, end });
    else if (out.length) out[out.length - 1]!.end = end;
    start = end;
  }
  if (start < text.length && text.slice(start).trim()) out.push({ start, end: text.length });
  return out;
}

/** Most UTF-16 units per utterance: some engines stop or refuse long utterances. */
export const MAX_SPEECH_CHUNK = 220;

/** Is `i` between the two halves of a surrogate pair? */
const insidePair = (text: string, i: number) =>
  i > 0 &&
  i < text.length &&
  /[\uD800-\uDBFF]/.test(text[i - 1] ?? '') &&
  /[\uDC00-\uDFFF]/.test(text[i] ?? '');

/**
 * Utterances of a paragraph: whole sentences, joined while they fit in `max` units; a longer
 * sentence is cut at the last white space before the limit (or, without any, at the limit but
 * never inside a surrogate pair).
 */
export function chunkText(text: string, max = MAX_SPEECH_CHUNK): TextRange[] {
  const pieces: TextRange[] = [];
  for (const s of splitSentences(text)) {
    let a = s.start;
    while (s.end - a > max) {
      let cut = -1;
      for (let i = a + max; i > a; i--) {
        if (/\s/.test(text[i - 1] ?? '')) {
          cut = i;
          break;
        }
      }
      if (cut <= a) {
        cut = a + max;
        if (insidePair(text, cut)) cut -= 1;
      }
      pieces.push({ start: a, end: cut });
      a = cut;
    }
    if (a < s.end) pieces.push({ start: a, end: s.end });
  }
  const out: TextRange[] = [];
  for (const p of pieces) {
    const last = out[out.length - 1];
    // Join short sentences (fewer, smoother utterances) — only whole sentences.
    if (last && last.end === p.start && p.end - last.start <= max && sentenceEnd(text, last.end)) {
      last.end = p.end;
    } else out.push({ ...p });
  }
  return out;
}

const sentenceEnd = (text: string, at: number) =>
  /[.!?…\n\p{Zl}\p{Zp}\v\f]["'”’»)\]]*\s*$/u.test(text.slice(0, at));

/** Range of the word at UTF-16 offset `at` (`length` 0 or missing in a boundary event). */
export function wordAt(text: string, at: number): TextRange {
  let start = Math.max(0, Math.min(at, text.length));
  while (start < text.length && /\s/.test(text[start] ?? '')) start++;
  let end = start;
  while (end < text.length && !/\s/.test(text[end] ?? '')) end++;
  return { start, end };
}

/** Start of the word around UTF-16 offset `at`. */
export function wordStart(text: string, at: number): number {
  let i = Math.max(0, Math.min(at, text.length));
  while (i > 0 && !/\s/.test(text[i - 1] ?? '')) i--;
  return i;
}

// Letters only Vietnamese uses among Latin scripts: ă â đ ê ô ơ ư and the tone marks on vowels.
const VI_LETTER = /[ăâđêôơưàảãáạằẳẵắặầẩẫấậèẻẽéẹềểễếệìỉĩíịòỏõóọồổỗốộờởỡớợùủũúụừửữứựỳỷỹýỵ]/iu;

/**
 * Language a text reads as: `vi` when at least a quarter of its words carry a letter only
 * Vietnamese uses, `en` when it has at least `EN_MIN_WORDS` Latin words and none does (shorter
 * text without diacritics — "Hai", "Ba Vì" — is ambiguous); else `null`. (The same rule as the
 * engine's `bo_spell::lang::detect` for Vietnamese, simpler for English.)
 */
/** Latin words without Vietnamese letters needed to call a text English. */
export const EN_MIN_WORDS = 3;

export function detectLanguage(text: string): 'vi' | 'en' | null {
  const words = text.split(/\s+/).filter((w) => /\p{L}/u.test(w));
  if (!words.length) return null;
  const vi = words.filter((w) => VI_LETTER.test(w)).length;
  if (vi > 0 && vi * 4 >= words.length) return 'vi';
  if (
    vi === 0 &&
    words.length >= EN_MIN_WORDS &&
    words.every((w) => /^[\p{Script=Latin}\p{N}\p{P}\p{S}]+$/u.test(w))
  )
    return 'en';
  return null;
}

/** Primary language subtag, lower case (`vi-VN` → `vi`, `en_US` → `en`). */
export const primaryLang = (tag: string) => tag.split(/[-_]/)[0]!.toLowerCase();

const normTag = (tag: string) => tag.replace(/_/g, '-').toLowerCase();

/**
 * Voice for text in `lang`: the `preferred` voice when it speaks that language, else a voice of
 * exactly that tag, else one of the same primary language (`en` → any `en-*`), the platform's
 * default and local voices first; `null` when none is installed.
 */
export function pickVoice(
  voices: readonly SpeechVoice[],
  lang: string,
  preferred?: string | null,
): SpeechVoice | null {
  const want = normTag(lang);
  const primary = primaryLang(lang);
  const same = voices.filter((v) => primaryLang(v.lang) === primary);
  const pref = preferred ? same.find((v) => v.id === preferred) : undefined;
  if (pref) return pref;
  const rank = (v: SpeechVoice) =>
    (normTag(v.lang) === want ? 0 : 4) + (v.default ? 0 : 2) + (v.local ? 0 : 1);
  return [...same].sort((a, b) => rank(a) - rank(b))[0] ?? null;
}

/**
 * Language to speak a text in: what the text reads as (Vietnamese typed in a run tagged
 * `en-US` is still Vietnamese), else its tagged language, else `fallback` (the UI language).
 */
export function speechLanguage(
  detected: string | null | undefined,
  tagged: string | null | undefined,
  fallback: string,
): string {
  if (detected) return detected.includes('-') ? detected : detected === 'vi' ? 'vi-VN' : detected;
  return tagged || fallback;
}

// ------------------------------------------------------------------------------ Web Speech API

/** The parts of `SpeechSynthesisUtterance` used here. */
export interface UtteranceLike {
  text: string;
  lang: string;
  voice: unknown;
  rate: number;
  pitch: number;
  volume: number;
  onstart: ((e: unknown) => void) | null;
  onend: ((e: unknown) => void) | null;
  onerror: ((e: { error?: string }) => void) | null;
  onboundary: ((e: { name?: string; charIndex: number; charLength?: number }) => void) | null;
  onpause: ((e: unknown) => void) | null;
  onresume: ((e: unknown) => void) | null;
}

/** The parts of `SpeechSynthesisVoice` used here. */
export interface VoiceLike {
  voiceURI: string;
  name: string;
  lang: string;
  localService: boolean;
  default: boolean;
}

/** The parts of `window.speechSynthesis` used here. */
export interface SynthesisLike {
  speak(u: UtteranceLike): void;
  cancel(): void;
  pause(): void;
  resume(): void;
  getVoices(): VoiceLike[];
  addEventListener?(type: 'voiceschanged', l: () => void): void;
  removeEventListener?(type: 'voiceschanged', l: () => void): void;
}

/** Text to speech over the Web Speech API. */
export class WebSpeechProvider implements SpeechProvider {
  constructor(
    private readonly synth: SynthesisLike,
    private readonly makeUtterance: (text: string) => UtteranceLike,
    /** How long `voices()` waits for a late voice list (Chromium loads it asynchronously). */
    private readonly voiceWaitMs = 1500,
  ) {}

  private list(): SpeechVoice[] {
    return this.synth.getVoices().map((v) => ({
      id: v.voiceURI || v.name,
      name: v.name,
      lang: v.lang,
      local: v.localService,
      default: v.default,
    }));
  }

  async voices(): Promise<SpeechVoice[]> {
    const now = this.list();
    if (now.length || !this.synth.addEventListener) return now;
    return new Promise((resolve) => {
      const done = () => {
        clearTimeout(timer);
        this.synth.removeEventListener?.('voiceschanged', done);
        resolve(this.list());
      };
      const timer = setTimeout(done, this.voiceWaitMs);
      this.synth.addEventListener?.('voiceschanged', done);
    });
  }

  onVoicesChanged(listener: () => void): () => void {
    this.synth.addEventListener?.('voiceschanged', listener);
    return () => this.synth.removeEventListener?.('voiceschanged', listener);
  }

  speak(text: string, o: SpeakOptions, onEvent: (e: SpeechEvent) => void): SpeechUtterance {
    const u = this.makeUtterance(text);
    if (o.lang) u.lang = o.lang;
    const voice = o.voice
      ? this.synth.getVoices().find((v) => (v.voiceURI || v.name) === o.voice)
      : undefined;
    if (voice) {
      u.voice = voice;
      u.lang = voice.lang;
    }
    if (o.rate !== undefined) u.rate = o.rate;
    if (o.pitch !== undefined) u.pitch = o.pitch;
    if (o.volume !== undefined) u.volume = o.volume;
    let live = true;
    const emit = (e: SpeechEvent) => {
      if (live) onEvent(e);
    };
    u.onstart = () => emit({ type: 'start' });
    u.onboundary = (e) => {
      const offset = e.charIndex;
      // `charLength` is optional in the spec (and 0 in some engines): use the word's extent.
      const length = e.charLength || wordAt(text, offset).end - offset;
      emit({ type: e.name === 'sentence' ? 'sentence' : 'word', offset, length });
    };
    u.onpause = () => emit({ type: 'pause' });
    u.onresume = () => emit({ type: 'resume' });
    u.onend = () => {
      emit({ type: 'end' });
      live = false;
    };
    u.onerror = (e) => {
      emit({ type: 'error', error: e.error ?? 'synthesis-failed' });
      live = false;
    };
    this.synth.speak(u);
    return {
      pause: () => this.synth.pause(),
      resume: () => this.synth.resume(),
      cancel: () => {
        live = false;
        this.synth.cancel();
      },
    };
  }
}

/** The Web Speech provider of a window, when it has `speechSynthesis`. */
export function webSpeech(w: unknown = globalThis): SpeechProvider | undefined {
  const g = w as {
    speechSynthesis?: SynthesisLike;
    SpeechSynthesisUtterance?: new (text: string) => UtteranceLike;
  };
  const synth = g.speechSynthesis;
  const Utterance = g.SpeechSynthesisUtterance;
  if (!synth || typeof Utterance !== 'function') return undefined;
  return new WebSpeechProvider(synth, (text) => new Utterance(text));
}

// ------------------------------------------------------------------------------- fake provider

/** An utterance of the fake provider. */
export interface FakeUtterance {
  text: string;
  options: SpeakOptions;
  /** Word ranges the fake will report, in order. */
  words: TextRange[];
  /** Next word to report. */
  next: number;
  paused: boolean;
  done: boolean;
  cancelled: boolean;
}

/**
 * Provider for tests: every `\S+` run is a word; `step()` reports the next word of the current
 * utterance (nothing while paused), `finish()` reports the rest and ends it, `run()` plays
 * every queued utterance to the end, `fail()` ends it with an error.
 */
export class FakeSpeechProvider implements SpeechProvider {
  readonly spoken: FakeUtterance[] = [];
  private listeners = new Set<() => void>();
  private handlers = new Map<FakeUtterance, (e: SpeechEvent) => void>();

  constructor(private voiceList: SpeechVoice[] = []) {}

  async voices(): Promise<SpeechVoice[]> {
    return this.voiceList;
  }

  setVoices(v: SpeechVoice[]): void {
    this.voiceList = v;
    for (const l of this.listeners) l();
  }

  onVoicesChanged(listener: () => void): () => void {
    this.listeners.add(listener);
    return () => this.listeners.delete(listener);
  }

  /** The utterance being spoken (not ended, not cancelled). */
  get current(): FakeUtterance | undefined {
    return this.spoken.find((u) => !u.done && !u.cancelled);
  }

  speak(text: string, options: SpeakOptions, onEvent: (e: SpeechEvent) => void): SpeechUtterance {
    const words: TextRange[] = [];
    for (const m of text.matchAll(/\S+/g)) {
      words.push({ start: m.index, end: m.index + m[0].length });
    }
    const u: FakeUtterance = {
      text,
      options,
      words,
      next: 0,
      paused: false,
      done: false,
      cancelled: false,
    };
    this.spoken.push(u);
    this.handlers.set(u, onEvent);
    onEvent({ type: 'start' });
    return {
      pause: () => {
        if (u.done || u.cancelled || u.paused) return;
        u.paused = true;
        this.handlers.get(u)?.({ type: 'pause' });
      },
      resume: () => {
        if (u.done || u.cancelled || !u.paused) return;
        u.paused = false;
        this.handlers.get(u)?.({ type: 'resume' });
      },
      cancel: () => {
        u.cancelled = true;
        this.handlers.delete(u);
      },
    };
  }

  /** Report the next word; at the end of the words, end the utterance. Returns whether anything happened. */
  step(): boolean {
    const u = this.current;
    if (!u || u.paused) return false;
    const emit = this.handlers.get(u);
    const w = u.words[u.next];
    if (w) {
      u.next++;
      emit?.({ type: 'word', offset: w.start, length: w.end - w.start });
    } else {
      u.done = true;
      this.handlers.delete(u);
      emit?.({ type: 'end' });
    }
    return true;
  }

  /** Report the rest of the current utterance and end it. */
  finish(): void {
    const u = this.current;
    if (!u || u.paused) return;
    while (this.current === u && this.step()) {
      // step until it ends
    }
  }

  /** Play every utterance (including ones queued while playing) to the end; `limit` guards loops. */
  run(limit = 10_000): void {
    for (let i = 0; i < limit && this.step(); i++) {
      // keep stepping
    }
  }

  fail(error = 'synthesis-failed'): void {
    const u = this.current;
    if (!u) return;
    u.done = true;
    const emit = this.handlers.get(u);
    this.handlers.delete(u);
    emit?.({ type: 'error', error });
  }
}
