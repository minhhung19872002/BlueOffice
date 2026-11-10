/**
 * Read aloud session (CORE-LANG-003), shared by BlueDoc, BlueSheet and BlueSlide: speaks a list
 * of text units (paragraphs, cells, text boxes) through the platform's `SpeechProvider`, one
 * utterance per chunk of sentences (`chunkText`), and reports the word being spoken as a UTF-16
 * range of its unit so the editor can highlight it. The voice follows the language of each unit
 * unless the user picked one; a unit in a language without an installed voice stops reading with
 * a `noVoice` notice. Pause / resume, previous / next unit, speed and voice changes (the current
 * sentence restarts from the word being spoken).
 */
import {
  chunkText,
  pickVoice,
  primaryLang,
  wordStart,
  type SpeechProvider,
  type SpeechUtterance,
  type SpeechVoice,
  type TextRange,
} from '@blueoffice/platform';

/** A piece of text to read. Offsets are UTF-16 code units of `text`. */
export interface ReadUnit {
  text: string;
  /** Language to speak it in (BCP 47); absent → the session's fallback language. */
  lang?: string | null;
  /** Ranges not to speak (hidden, deleted text). */
  skip?: TextRange[];
  /** Only this part is read (a selection); absent → all of it. */
  range?: TextRange;
}

export type ReadAloudState = 'idle' | 'playing' | 'paused';

/** The word being spoken: unit index and UTF-16 range in its text. */
export interface ReadPosition {
  unit: number;
  start: number;
  end: number;
}

export interface ReadAloudListener {
  /** A word starts (also the first word of a unit). */
  onWord?(at: ReadPosition): void;
  /** Reading moved to unit `unit`. */
  onUnit?(unit: number): void;
  onState?(state: ReadAloudState): void;
  /** No installed voice speaks `lang` (reading stops). */
  onNoVoice?(lang: string): void;
  /** The platform failed to speak. */
  onError?(error: string): void;
  /** Everything was read. */
  onDone?(): void;
}

export interface ReadAloudSettings {
  rate: number;
  pitch: number;
  volume: number;
  /** Voice the user picked (absent: by the language of each unit). */
  voice: string | null;
  /** Language of units without one. */
  fallbackLang: string;
  /** Longest utterance in UTF-16 units. */
  maxChunk?: number;
}

export const READ_RATES = [0.5, 0.75, 1, 1.25, 1.5, 1.75, 2] as const;

// U+FFFC (object placeholder of the index text) and skipped text are spoken as spaces: the
// spoken string keeps the unit's length so offsets map back one to one.
export function speakable(u: ReadUnit): string {
  let t = u.text.replace(/￼/g, ' ');
  for (const r of u.skip ?? []) {
    const a = Math.max(0, r.start);
    const b = Math.min(t.length, r.end);
    if (b > a) t = t.slice(0, a) + ' '.repeat(b - a) + t.slice(b);
  }
  return t;
}

export class ReadAloudSession {
  private state: ReadAloudState = 'idle';
  private unit = 0;
  /** Chunks of the current unit (UTF-16 ranges, clipped to the unit's range). */
  private chunks: TextRange[] = [];
  private chunk = 0;
  /** Start of the current utterance in the unit (it may begin inside a chunk). */
  private from = 0;
  /** Start of the last word reported. */
  private wordAt = -1;
  private utterance: SpeechUtterance | null = null;
  /** Utterance generation: events of cancelled utterances are ignored. */
  private gen = 0;
  private voiceList: SpeechVoice[] | null = null;
  private texts: string[];

  constructor(
    private readonly provider: SpeechProvider,
    readonly units: readonly ReadUnit[],
    private settings: ReadAloudSettings,
    private readonly listener: ReadAloudListener = {},
  ) {
    this.texts = units.map(speakable);
  }

  get current(): { state: ReadAloudState; unit: number; word: number } {
    return { state: this.state, unit: this.unit, word: this.wordAt };
  }

  get options(): ReadAloudSettings {
    return this.settings;
  }

  /** Voices of the platform (listed once per session). */
  async voices(): Promise<SpeechVoice[]> {
    this.voiceList ??= await this.provider.voices();
    return this.voiceList;
  }

  /** Language a unit is spoken in. */
  langOf(i: number): string {
    return this.units[i]?.lang || this.settings.fallbackLang;
  }

  /** Start reading at unit `unit`, UTF-16 offset `offset` (snapped to the start of its word). */
  async start(unit = 0, offset?: number): Promise<void> {
    await this.voices();
    this.cancelUtterance();
    this.go(unit, offset);
  }

  pause(): void {
    if (this.state !== 'playing') return;
    this.utterance?.pause();
    this.setState('paused');
  }

  resume(): void {
    if (this.state !== 'paused') return;
    this.utterance?.resume();
    this.setState('playing');
  }

  toggle(): void {
    if (this.state === 'playing') this.pause();
    else if (this.state === 'paused') this.resume();
  }

  stop(): void {
    this.cancelUtterance();
    this.setState('idle');
  }

  /** Next unit (stops after the last). */
  next(): void {
    if (this.state === 'idle') return;
    this.cancelUtterance();
    if (this.unit + 1 >= this.units.length) {
      this.finish();
      return;
    }
    this.go(this.unit + 1);
  }

  /** Start of the previous unit (the current one from its start when it is the first). */
  previous(): void {
    if (this.state === 'idle') return;
    this.cancelUtterance();
    this.go(Math.max(0, this.unit - 1));
  }

  /** Change speed / voice / pitch; a playing sentence restarts at the word being spoken. */
  update(next: Partial<ReadAloudSettings>): void {
    this.settings = { ...this.settings, ...next };
    if (this.state === 'idle') return;
    const at = this.wordAt >= 0 ? this.wordAt : this.from;
    const paused = this.state === 'paused';
    this.cancelUtterance();
    this.go(this.unit, at);
    if (paused && this.state === 'playing') this.pause();
  }

  dispose(): void {
    this.cancelUtterance();
    this.state = 'idle';
  }

  // ------------------------------------------------------------------------------------------

  private setState(s: ReadAloudState): void {
    if (this.state === s) return;
    this.state = s;
    this.listener.onState?.(s);
  }

  private cancelUtterance(): void {
    this.gen++;
    this.utterance?.cancel();
    this.utterance = null;
  }

  private finish(): void {
    this.cancelUtterance();
    this.setState('idle');
    this.listener.onDone?.();
  }

  /** Begin unit `i` (skipping units with nothing to say) at `offset`. */
  private go(i: number, offset?: number): void {
    for (; i < this.units.length; i++, offset = undefined) {
      const u = this.units[i]!;
      const text = this.texts[i]!;
      const lo = u.range?.start ?? 0;
      const hi = u.range?.end ?? text.length;
      const chunks = chunkText(text, this.settings.maxChunk)
        .map((c) => ({ start: Math.max(c.start, lo), end: Math.min(c.end, hi) }))
        .filter((c) => c.end > c.start && text.slice(c.start, c.end).trim());
      const at = offset === undefined ? lo : wordStart(text, Math.max(lo, Math.min(offset, hi)));
      const k = chunks.findIndex(
        (c) => c.end > at && text.slice(Math.max(at, c.start), c.end).trim(),
      );
      if (k < 0) continue;
      this.unit = i;
      this.chunks = chunks;
      this.chunk = k;
      this.from = Math.max(at, chunks[k]!.start);
      this.wordAt = -1;
      this.listener.onUnit?.(i);
      this.speak();
      return;
    }
    this.finish();
  }

  private voiceFor(lang: string): SpeechVoice | null | undefined {
    const voices = this.voiceList ?? [];
    const chosen = this.settings.voice ? voices.find((v) => v.id === this.settings.voice) : null;
    // A voice the user picked speaks text of its own language; other languages pick by language.
    if (chosen && primaryLang(chosen.lang) === primaryLang(lang)) return chosen;
    // A platform that lists no voices at all still speaks with its own choice for `lang`.
    if (!voices.length) return undefined;
    return pickVoice(voices, lang);
  }

  private speak(): void {
    const c = this.chunks[this.chunk];
    if (!c) return;
    const text = this.texts[this.unit]!;
    const lang = this.langOf(this.unit);
    const voice = this.voiceFor(lang);
    if (voice === null) {
      this.cancelUtterance();
      this.setState('idle');
      this.listener.onNoVoice?.(lang);
      return;
    }
    const base = this.from;
    const spoken = text.slice(base, c.end);
    const gen = ++this.gen;
    this.setState('playing');
    this.utterance = this.provider.speak(
      spoken,
      {
        voice: voice?.id ?? null,
        lang: voice?.lang ?? lang,
        rate: this.settings.rate,
        pitch: this.settings.pitch,
        volume: this.settings.volume,
      },
      (e) => {
        if (gen !== this.gen) return;
        if (e.type === 'word') {
          const start = base + e.offset;
          const end = Math.min(c.end, start + Math.max(1, e.length));
          this.wordAt = start;
          this.listener.onWord?.({ unit: this.unit, start, end });
        } else if (e.type === 'end') {
          this.afterChunk();
        } else if (e.type === 'error') {
          // `interrupted` / `canceled` come from our own cancel (already filtered by `gen`).
          this.utterance = null;
          this.setState('idle');
          this.listener.onError?.(e.error);
        }
      },
    );
  }

  private afterChunk(): void {
    this.utterance = null;
    if (this.chunk + 1 < this.chunks.length) {
      this.chunk++;
      this.from = this.chunks[this.chunk]!.start;
      this.speak();
    } else if (this.unit + 1 < this.units.length) {
      this.go(this.unit + 1);
    } else {
      this.finish();
    }
  }
}
