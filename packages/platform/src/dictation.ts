/**
 * Dictation — speech to text (CORE-LANG-002). Editors never touch a microphone or a recogniser
 * themselves: they ask the platform's `DictationProvider` for a stream of interim / final
 * results and turn them into edits (voice commands, spacing, capitals: `@blueoffice/editor`
 * `dictation.ts`). Providers:
 *
 * - `WebDictationProvider` — the W3C Web Speech API recogniser (`SpeechRecognition`, prefixed
 *   `webkitSpeechRecognition`; https://webaudio.github.io/web-speech-api/) where the runtime has
 *   one. Feature-detected: the desktop webview may expose none, or one that fails at start
 *   (reported as `unavailable`). Note that some browsers recognise on a remote service of the
 *   browser vendor, which is why `AutoDictationProvider` prefers the organisation's server when
 *   an administrator configured one.
 * - `ServerDictationProvider` — microphone audio captured through the platform (`AudioCapture`,
 *   PCM frames), cut into phrases at pauses (`PhraseSegmenter`), each phrase sent as 16 kHz mono
 *   WAV to the BlueOffice server's `POST /api/dictation`, which forwards it to the speech-to-text
 *   service an administrator configured (off by default; `docs/deploy/dictation.md`). The server
 *   never logs audio or text.
 * - `LocalDictationProvider` — the same phrase pipeline over an on-device model
 *   (`LocalDictationModel`); no model ships with BlueOffice (licence review pending).
 * - `FakeDictationProvider` — deterministic, for tests.
 */
import { primaryLang } from './speech';

/** Why dictation failed or could not start. */
export type DictationErrorCode =
  /** The user (or a policy) refused the microphone. */
  | 'noPermission'
  /** No microphone, or it cannot be opened. */
  | 'noDevice'
  /** The recognition service cannot be reached. */
  | 'network'
  /** No recogniser on this platform / nothing configured on the server. */
  | 'unavailable'
  | 'unsupportedLanguage'
  /** Not signed in / no session (server provider). */
  | 'unauthorized'
  /** A phrase too long for the service. */
  | 'tooLarge'
  | 'rateLimited'
  /** The recognition service failed or answered something unusable. */
  | 'upstream';

export class DictationError extends Error {
  constructor(
    readonly code: DictationErrorCode,
    message?: string,
  ) {
    super(message ?? code);
    this.name = 'DictationError';
  }
}

/** `e` as a `DictationError` (unknown failures are `upstream`). */
export function dictationError(e: unknown): DictationError {
  if (e instanceof DictationError) return e;
  if (e instanceof DOMException) {
    switch (e.name) {
      case 'NotAllowedError':
      case 'SecurityError':
        return new DictationError('noPermission', e.message);
      case 'NotFoundError':
      case 'NotReadableError':
      case 'OverconstrainedError':
        return new DictationError('noDevice', e.message);
      case 'NetworkError':
        return new DictationError('network', e.message);
      default:
        break;
    }
  }
  if (e instanceof TypeError) return new DictationError('network', e.message);
  return new DictationError('upstream', e instanceof Error ? e.message : String(e));
}

/** A recognised stretch of speech. */
export interface DictationResult {
  text: string;
  /** Final (will not change) or interim (a guess, replaced by the next result). */
  final: boolean;
  /** 0–1 when the recogniser reports it. */
  confidence?: number;
}

/** What a running dictation reports. */
export type DictationEvent =
  | { type: 'start' }
  | ({ type: 'result' } & DictationResult)
  /** The session ended (after `stop()`, or the recogniser stopped on its own). */
  | { type: 'end' }
  /** The session failed; nothing follows. */
  | { type: 'error'; code: DictationErrorCode; message?: string };

export interface DictationOptions {
  /** Report interim results (default true; providers without interim results ignore it). */
  interim?: boolean;
}

/** A running dictation. */
export interface DictationSession {
  /** Stops listening; results still being recognised arrive, then `end`. */
  stop(): void;
  /** Stops at once: no further events. */
  abort(): void;
}

/** Speech to text of the platform. */
export interface DictationProvider {
  /** Where recognition runs (shown to the user). */
  readonly kind: 'browser' | 'server' | 'local' | 'fake';
  /** Can dictation start here at all (configured, recogniser present)? */
  available(): Promise<boolean>;
  /** Languages offered (BCP 47; empty: any the recogniser accepts). */
  languages(): Promise<string[]>;
  /** Starts listening in `lang` (BCP 47, `vi-VN`). Failures arrive as an `error` event. */
  start(
    lang: string,
    options: DictationOptions,
    onEvent: (e: DictationEvent) => void,
  ): DictationSession;
}

/** Languages offered by default in the language picker. */
export const DICTATION_LANGUAGES = ['vi-VN', 'en-US'] as const;

// ------------------------------------------------------------------------------ Web Speech API

/** One alternative of a `SpeechRecognitionResult`. */
export interface RecognitionAlternativeLike {
  transcript: string;
  confidence: number;
}

/** A `SpeechRecognitionResult`. */
export interface RecognitionResultLike {
  readonly isFinal: boolean;
  readonly length: number;
  [index: number]: RecognitionAlternativeLike | undefined;
}

/** A `SpeechRecognitionEvent`. */
export interface RecognitionEventLike {
  resultIndex: number;
  results: { readonly length: number; [index: number]: RecognitionResultLike | undefined };
}

/** The parts of a `SpeechRecognition` used here. */
export interface RecognitionLike {
  lang: string;
  continuous: boolean;
  interimResults: boolean;
  maxAlternatives: number;
  onstart: (() => void) | null;
  onresult: ((e: RecognitionEventLike) => void) | null;
  onerror: ((e: { error: string; message?: string }) => void) | null;
  onend: (() => void) | null;
  start(): void;
  stop(): void;
  abort(): void;
}

/** Error code of a `SpeechRecognitionErrorEvent.error`; `null`: not fatal (keep listening). */
export function recognitionErrorCode(error: string): DictationErrorCode | null {
  switch (error) {
    case 'no-speech':
    case 'aborted':
      return null;
    case 'not-allowed':
      return 'noPermission';
    case 'audio-capture':
      return 'noDevice';
    case 'network':
      return 'network';
    case 'service-not-allowed':
      return 'unavailable';
    case 'language-not-supported':
      return 'unsupportedLanguage';
    default:
      return 'upstream';
  }
}

/** Restarts after the recogniser ends on its own (silence) before giving up. */
const MAX_QUICK_RESTARTS = 3;
/** A run shorter than this counts as a quick restart. */
const QUICK_RUN_MS = 1000;

/** Dictation over the Web Speech API recogniser. */
export class WebDictationProvider implements DictationProvider {
  readonly kind = 'browser' as const;

  constructor(
    private readonly make: () => RecognitionLike,
    private readonly now: () => number = () => Date.now(),
  ) {}

  async available(): Promise<boolean> {
    return true;
  }

  async languages(): Promise<string[]> {
    return [];
  }

  start(
    lang: string,
    options: DictationOptions,
    onEvent: (e: DictationEvent) => void,
  ): DictationSession {
    let live = true;
    let stopping = false;
    let started = false;
    let quick = 0;
    let runStart = 0;
    let rec: RecognitionLike | null = null;
    const emit = (e: DictationEvent) => {
      if (live) onEvent(e);
    };
    const fail = (code: DictationErrorCode, message?: string) => {
      if (!live) return;
      emit({ type: 'error', code, message });
      live = false;
      try {
        rec?.abort();
      } catch {
        // already stopped
      }
    };
    const run = () => {
      let r: RecognitionLike;
      try {
        r = this.make();
      } catch (e) {
        fail('unavailable', e instanceof Error ? e.message : String(e));
        return;
      }
      rec = r;
      r.lang = lang;
      r.continuous = true;
      r.interimResults = options.interim !== false;
      r.maxAlternatives = 1;
      r.onstart = () => {
        runStart = this.now();
        if (!started) {
          started = true;
          emit({ type: 'start' });
        }
      };
      r.onresult = (e) => {
        let interim = '';
        for (let i = e.resultIndex; i < e.results.length; i++) {
          const res = e.results[i];
          const alt = res?.[0];
          if (!res || !alt) continue;
          if (res.isFinal) {
            interim = '';
            emit({
              type: 'result',
              text: alt.transcript,
              final: true,
              ...(alt.confidence > 0 ? { confidence: alt.confidence } : {}),
            });
          } else interim += alt.transcript;
        }
        if (interim) emit({ type: 'result', text: interim, final: false });
      };
      r.onerror = (e) => {
        const code = recognitionErrorCode(e.error);
        if (code) fail(code, e.message);
      };
      r.onend = () => {
        if (!live || rec !== r) return;
        if (stopping) {
          emit({ type: 'end' });
          live = false;
          return;
        }
        // Ended on its own (a long pause): listen again, unless it keeps ending at once.
        quick = this.now() - runStart < QUICK_RUN_MS ? quick + 1 : 0;
        if (quick > MAX_QUICK_RESTARTS) {
          emit({ type: 'end' });
          live = false;
          return;
        }
        run();
      };
      try {
        r.start();
      } catch (e) {
        fail('unavailable', e instanceof Error ? e.message : String(e));
      }
    };
    run();
    return {
      stop: () => {
        if (!live || stopping) return;
        stopping = true;
        try {
          rec?.stop();
        } catch {
          emit({ type: 'end' });
          live = false;
        }
      },
      abort: () => {
        live = false;
        try {
          rec?.abort();
        } catch {
          // already stopped
        }
      },
    };
  }
}

/** The Web Speech recogniser of a window, when it has one. */
export function webDictation(w: unknown = globalThis): WebDictationProvider | undefined {
  const g = w as {
    SpeechRecognition?: new () => RecognitionLike;
    webkitSpeechRecognition?: new () => RecognitionLike;
  };
  const Recognition = g.SpeechRecognition ?? g.webkitSpeechRecognition;
  if (typeof Recognition !== 'function') return undefined;
  return new WebDictationProvider(() => new Recognition());
}

// ------------------------------------------------------------------------------ audio capture

/** An open microphone. */
export interface AudioStream {
  /** Sample rate of the frames (Hz). */
  readonly sampleRate: number;
  close(): void;
}

/** Microphone audio as mono PCM frames (−1…1). Throws `DictationError` (`noPermission`…). */
export interface AudioCapture {
  open(onFrames: (pcm: Float32Array) => void): Promise<AudioStream>;
}

interface MediaTrackLike {
  stop(): void;
}
interface MediaStreamLike {
  getTracks(): MediaTrackLike[];
}
interface AudioNodeLike {
  connect(n: unknown): void;
  disconnect(): void;
}
interface ProcessorLike extends AudioNodeLike {
  onaudioprocess:
    ((e: { inputBuffer: { getChannelData(c: number): Float32Array } }) => void) | null;
}
interface AudioContextLike {
  readonly sampleRate: number;
  readonly destination: unknown;
  createMediaStreamSource(s: MediaStreamLike): AudioNodeLike;
  createScriptProcessor(size: number, inputs: number, outputs: number): ProcessorLike;
  close(): Promise<void>;
}

/** Microphone capture through `getUserMedia` + Web Audio, when the window has both. */
export function webAudioCapture(w: unknown = globalThis): AudioCapture | undefined {
  const g = w as {
    navigator?: {
      mediaDevices?: { getUserMedia?(c: { audio: unknown }): Promise<MediaStreamLike> };
    };
    AudioContext?: new () => AudioContextLike;
    webkitAudioContext?: new () => AudioContextLike;
  };
  const media = g.navigator?.mediaDevices;
  const Ctx = g.AudioContext ?? g.webkitAudioContext;
  if (!media?.getUserMedia || typeof Ctx !== 'function') return undefined;
  return {
    async open(onFrames) {
      let stream: MediaStreamLike;
      try {
        stream = await media.getUserMedia!({
          audio: { channelCount: 1, echoCancellation: true, noiseSuppression: true },
        });
      } catch (e) {
        throw dictationError(e);
      }
      const ctx = new Ctx();
      const source = ctx.createMediaStreamSource(stream);
      // ScriptProcessorNode: deprecated but available everywhere (an AudioWorklet needs a module URL).
      const proc = ctx.createScriptProcessor(4096, 1, 1);
      proc.onaudioprocess = (e) => onFrames(new Float32Array(e.inputBuffer.getChannelData(0)));
      source.connect(proc);
      proc.connect(ctx.destination);
      return {
        sampleRate: ctx.sampleRate,
        close: () => {
          proc.onaudioprocess = null;
          source.disconnect();
          proc.disconnect();
          for (const t of stream.getTracks()) t.stop();
          void ctx.close().catch(() => {});
        },
      };
    },
  };
}

// ------------------------------------------------------------------------------ phrases

export interface SegmenterOptions {
  /** RMS level above which a frame is speech (adaptive floor on top). */
  threshold?: number;
  /** Silence that ends a phrase (ms). */
  silenceMs?: number;
  /** Longest phrase (ms) before it is cut. */
  maxMs?: number;
  /** Shorter speech is noise (ms). */
  minSpeechMs?: number;
  /** Audio kept before speech starts (ms). */
  preRollMs?: number;
}

/** Analysis window (ms). */
const WINDOW_MS = 20;

/**
 * Cuts microphone audio into phrases at pauses: a simple energy detector over 20 ms windows
 * (level above `threshold` and above three times the running noise floor is speech). Our own
 * rule; deterministic, so tests can feed it synthetic audio.
 */
export class PhraseSegmenter {
  private readonly win: number;
  private readonly o: Required<SegmenterOptions>;
  private pending: number[] = [];
  private pre: Float32Array[] = [];
  private cur: Float32Array[] = [];
  private speechWins = 0;
  private silentWins = 0;
  private floor = 0;
  private inPhrase = false;

  constructor(
    readonly sampleRate: number,
    options: SegmenterOptions = {},
  ) {
    this.o = {
      threshold: options.threshold ?? 0.015,
      silenceMs: options.silenceMs ?? 700,
      maxMs: options.maxMs ?? 15_000,
      minSpeechMs: options.minSpeechMs ?? 200,
      preRollMs: options.preRollMs ?? 200,
    };
    this.win = Math.max(1, Math.round((sampleRate * WINDOW_MS) / 1000));
  }

  private wins(ms: number): number {
    return Math.max(1, Math.round(ms / WINDOW_MS));
  }

  /** Feeds frames; returns the phrases completed by them. */
  push(pcm: Float32Array): Float32Array[] {
    const out: Float32Array[] = [];
    for (const s of pcm) {
      this.pending.push(s);
      if (this.pending.length < this.win) continue;
      const w = Float32Array.from(this.pending);
      this.pending = [];
      const done = this.window(w);
      if (done) out.push(done);
    }
    return out;
  }

  private window(w: Float32Array): Float32Array | null {
    let sum = 0;
    for (const s of w) sum += s * s;
    const rms = Math.sqrt(sum / w.length);
    const speech = rms >= this.o.threshold && rms >= this.floor * 3;
    if (!speech) this.floor = this.floor ? this.floor * 0.95 + rms * 0.05 : rms;
    if (!this.inPhrase) {
      this.pre.push(w);
      // The pre-roll plus this window.
      if (this.pre.length > this.wins(this.o.preRollMs) + 1) this.pre.shift();
      if (speech) {
        this.inPhrase = true;
        this.cur = [...this.pre];
        this.pre = [];
        this.speechWins = 1;
        this.silentWins = 0;
      }
      return null;
    }
    this.cur.push(w);
    if (speech) {
      this.speechWins++;
      this.silentWins = 0;
    } else this.silentWins++;
    if (
      this.silentWins >= this.wins(this.o.silenceMs) ||
      this.cur.length >= this.wins(this.o.maxMs)
    )
      return this.close();
    return null;
  }

  private close(): Float32Array | null {
    const enough = this.speechWins >= this.wins(this.o.minSpeechMs);
    const parts = this.cur;
    this.cur = [];
    this.inPhrase = false;
    this.speechWins = 0;
    this.silentWins = 0;
    if (!enough) return null;
    const n = parts.reduce((k, p) => k + p.length, 0);
    const out = new Float32Array(n);
    let at = 0;
    for (const p of parts) {
      out.set(p, at);
      at += p.length;
    }
    return out;
  }

  /** The phrase in progress (stop): `null` when there is none worth sending. */
  flush(): Float32Array | null {
    if (this.pending.length) {
      this.cur.push(Float32Array.from(this.pending));
      this.pending = [];
    }
    return this.inPhrase ? this.close() : null;
  }
}

/** `pcm` at `from` Hz resampled to `to` Hz (averaging when going down, linear when going up). */
export function resample(pcm: Float32Array, from: number, to: number): Float32Array {
  if (from === to || !pcm.length) return pcm;
  const n = Math.max(1, Math.floor((pcm.length * to) / from));
  const out = new Float32Array(n);
  const ratio = from / to;
  for (let i = 0; i < n; i++) {
    const a = i * ratio;
    if (ratio > 1) {
      const lo = Math.floor(a);
      const hi = Math.min(pcm.length, Math.max(lo + 1, Math.floor(a + ratio)));
      let sum = 0;
      for (let k = lo; k < hi; k++) sum += pcm[k]!;
      out[i] = sum / (hi - lo);
    } else {
      const lo = Math.floor(a);
      const t = a - lo;
      out[i] = (pcm[lo] ?? 0) * (1 - t) + (pcm[Math.min(lo + 1, pcm.length - 1)] ?? 0) * t;
    }
  }
  return out;
}

/** Sample rate of the audio sent for recognition. */
export const DICTATION_SAMPLE_RATE = 16_000;

/** 16-bit mono PCM WAV of `pcm` (RIFF/WAVE, `fmt ` + `data` chunks). */
export function encodeWavMono(pcm: Float32Array, sampleRate: number): Uint8Array {
  const bytes = new Uint8Array(44 + pcm.length * 2);
  const v = new DataView(bytes.buffer);
  const ascii = (at: number, s: string) => {
    for (let i = 0; i < s.length; i++) v.setUint8(at + i, s.charCodeAt(i));
  };
  ascii(0, 'RIFF');
  v.setUint32(4, 36 + pcm.length * 2, true);
  ascii(8, 'WAVE');
  ascii(12, 'fmt ');
  v.setUint32(16, 16, true);
  v.setUint16(20, 1, true); // PCM
  v.setUint16(22, 1, true); // mono
  v.setUint32(24, sampleRate, true);
  v.setUint32(28, sampleRate * 2, true);
  v.setUint16(32, 2, true);
  v.setUint16(34, 16, true);
  ascii(36, 'data');
  v.setUint32(40, pcm.length * 2, true);
  pcm.forEach((s, i) => {
    const c = Math.max(-1, Math.min(1, s));
    v.setInt16(44 + i * 2, Math.round(c < 0 ? c * 0x8000 : c * 0x7fff), true);
  });
  return bytes;
}

/** Recognises one phrase (16 kHz mono PCM). */
export type PhraseRecognizer = (
  pcm: Float32Array,
  lang: string,
  signal: AbortSignal,
) => Promise<{ text: string; confidence?: number }>;

/**
 * Dictation that captures audio, cuts it into phrases and recognises each phrase (in order) as a
 * final result. No interim results.
 */
export class PhraseDictation {
  constructor(
    private readonly capture: AudioCapture | undefined,
    private readonly recognize: PhraseRecognizer,
    private readonly segmenter: SegmenterOptions = {},
  ) {}

  start(lang: string, onEvent: (e: DictationEvent) => void): DictationSession {
    const ctl = new AbortController();
    let live = true;
    let stopping = false;
    let stream: AudioStream | null = null;
    let seg: PhraseSegmenter | null = null;
    let queue: Promise<void> = Promise.resolve();
    const emit = (e: DictationEvent) => {
      if (live) onEvent(e);
    };
    const fail = (e: unknown) => {
      if (!live) return;
      const err = dictationError(e);
      emit({ type: 'error', code: err.code, message: err.message });
      live = false;
      ctl.abort();
      stream?.close();
    };
    const send = (phrase: Float32Array, rate: number) => {
      const pcm = resample(phrase, rate, DICTATION_SAMPLE_RATE);
      queue = queue.then(async () => {
        if (!live) return;
        try {
          const r = await this.recognize(pcm, lang, ctl.signal);
          const text = r.text.trim();
          if (text)
            emit({
              type: 'result',
              text,
              final: true,
              ...(r.confidence !== undefined ? { confidence: r.confidence } : {}),
            });
        } catch (e) {
          if (!ctl.signal.aborted) fail(e);
        }
      });
    };
    if (!this.capture) {
      queueMicrotask(() => fail(new DictationError('unavailable')));
    } else {
      void this.capture
        .open((pcm) => {
          if (!seg || stopping) return;
          for (const p of seg.push(pcm)) send(p, seg.sampleRate);
        })
        .then((s) => {
          if (!live || stopping) {
            s.close();
            if (live) emit({ type: 'end' });
            live = false;
            return;
          }
          stream = s;
          seg = new PhraseSegmenter(s.sampleRate, this.segmenter);
          emit({ type: 'start' });
        })
        .catch(fail);
    }
    return {
      stop: () => {
        if (!live || stopping) return;
        stopping = true;
        if (!stream) return; // the end follows when the microphone opens
        stream.close();
        const last = seg?.flush();
        if (last && seg) send(last, seg.sampleRate);
        void queue.then(() => {
          emit({ type: 'end' });
          live = false;
        });
      },
      abort: () => {
        live = false;
        ctl.abort();
        stream?.close();
      },
    };
  }
}

// ------------------------------------------------------------------------------ server

/** The parts of `fetch` the server provider uses. */
export type DictationFetch = (
  url: string,
  init?: {
    method?: string;
    headers?: Record<string, string>;
    body?: Uint8Array;
    signal?: AbortSignal;
  },
) => Promise<{ ok: boolean; status: number; json(): Promise<unknown> }>;

export interface ServerDictationOptions {
  /** BlueOffice server base URL; `''` = same origin. */
  baseUrl: string;
  /** Extra headers per request (bearer token, `x-session-token`). */
  headers?: () => Promise<Record<string, string>> | Record<string, string>;
  /** Editor session id (hosted files). */
  session?: string;
  fetch?: DictationFetch;
  /** The microphone (absent: dictation is unavailable). */
  capture?: AudioCapture;
  segmenter?: SegmenterOptions;
}

/** Error code of a server answer. */
export function dictationServerErrorCode(status: number, code?: unknown): DictationErrorCode {
  if (code === 'unsupportedLanguage') return 'unsupportedLanguage';
  if (status === 404 || status === 501) return 'unavailable';
  if (status === 401 || status === 403) return 'unauthorized';
  if (status === 413) return 'tooLarge';
  if (status === 429) return 'rateLimited';
  if (status === 400) return 'unsupportedLanguage';
  return 'upstream';
}

/** The BlueOffice server's dictation endpoint (`POST /api/dictation`, WAV per phrase). */
export class ServerDictationProvider implements DictationProvider {
  readonly kind = 'server' as const;
  private readonly fetchImpl: DictationFetch;
  private langs: Promise<string[] | null> | null = null;
  private readonly phrases: PhraseDictation;

  constructor(private readonly opts: ServerDictationOptions) {
    this.fetchImpl =
      opts.fetch ??
      ((u, init) => fetch(u, init as RequestInit) as unknown as ReturnType<DictationFetch>);
    this.phrases = new PhraseDictation(
      opts.capture,
      (pcm, lang, signal) => this.recognize(pcm, lang, signal),
      opts.segmenter,
    );
  }

  private url(path: string): string {
    return `${this.opts.baseUrl.replace(/\/+$/, '')}${path}`;
  }

  /** Languages configured on the server; `null` when dictation is off there. */
  private lookup(): Promise<string[] | null> {
    this.langs ??= (async () => {
      const q = this.opts.session ? `?session=${encodeURIComponent(this.opts.session)}` : '';
      try {
        const res = await this.fetchImpl(this.url(`/api/dictation/languages${q}`), {
          headers: { ...(await this.opts.headers?.()) },
        });
        if (!res.ok) return null;
        const body = (await res.json()) as { languages?: unknown };
        return Array.isArray(body.languages)
          ? body.languages.filter((c): c is string => typeof c === 'string' && !!c)
          : [];
      } catch {
        return null;
      }
    })();
    // A failed lookup is retried next time.
    void this.langs.then((l) => {
      if (!l) this.langs = null;
    });
    return this.langs;
  }

  async available(): Promise<boolean> {
    return !!this.opts.capture && (await this.lookup()) !== null;
  }

  async languages(): Promise<string[]> {
    return (await this.lookup()) ?? [];
  }

  private async recognize(
    pcm: Float32Array,
    lang: string,
    signal: AbortSignal,
  ): Promise<{ text: string; confidence?: number }> {
    const q = new URLSearchParams({ lang });
    if (this.opts.session) q.set('session', this.opts.session);
    const res = await this.fetchImpl(this.url(`/api/dictation?${q.toString()}`), {
      method: 'POST',
      headers: { ...(await this.opts.headers?.()), 'content-type': 'audio/wav' },
      body: encodeWavMono(pcm, DICTATION_SAMPLE_RATE),
      signal,
    });
    if (!res.ok) {
      let code: unknown;
      try {
        code = ((await res.json()) as { code?: unknown }).code;
      } catch {
        code = undefined;
      }
      throw new DictationError(dictationServerErrorCode(res.status, code), `HTTP ${res.status}`);
    }
    let body: { text?: unknown; confidence?: unknown };
    try {
      body = (await res.json()) as typeof body;
    } catch {
      throw new DictationError('upstream', 'invalid answer');
    }
    if (typeof body.text !== 'string') throw new DictationError('upstream', 'invalid answer');
    return {
      text: body.text,
      ...(typeof body.confidence === 'number' ? { confidence: body.confidence } : {}),
    };
  }

  start(
    lang: string,
    _options: DictationOptions,
    onEvent: (e: DictationEvent) => void,
  ): DictationSession {
    return this.phrases.start(lang, onEvent);
  }
}

// ------------------------------------------------------------------------------ local model

/**
 * An on-device speech recognition model (e.g. a WASM / ONNX model with a licence cleared for
 * redistribution). None ships yet: this is the slot a model is dropped into.
 */
export interface LocalDictationModel {
  /** Languages it recognises (primary subtags: `vi`, `en`). */
  readonly languages: readonly string[];
  /** Loads weights on first use. */
  load?(signal?: AbortSignal): Promise<void>;
  /** Text of one phrase of 16 kHz mono PCM. */
  transcribe(
    pcm: Float32Array,
    lang: string,
    signal?: AbortSignal,
  ): Promise<{ text: string; confidence?: number }>;
}

/** A `DictationProvider` over a `LocalDictationModel` (audio never leaves the device). */
export class LocalDictationProvider implements DictationProvider {
  readonly kind = 'local' as const;
  private loaded: Promise<void> | null = null;
  private readonly phrases: PhraseDictation;

  constructor(
    private readonly model: LocalDictationModel,
    capture: AudioCapture | undefined,
    segmenter?: SegmenterOptions,
  ) {
    this.phrases = new PhraseDictation(
      capture,
      async (pcm, lang, signal) => {
        if (!this.model.languages.includes(primaryLang(lang)))
          throw new DictationError('unsupportedLanguage');
        try {
          this.loaded ??= this.model.load?.(signal) ?? Promise.resolve();
          await this.loaded;
        } catch (e) {
          this.loaded = null;
          throw e;
        }
        return this.model.transcribe(pcm, lang, signal);
      },
      segmenter,
    );
  }

  async available(): Promise<boolean> {
    return true;
  }

  async languages(): Promise<string[]> {
    return [...this.model.languages];
  }

  start(
    lang: string,
    _options: DictationOptions,
    onEvent: (e: DictationEvent) => void,
  ): DictationSession {
    return this.phrases.start(lang, onEvent);
  }
}

// ------------------------------------------------------------------------------ choice

/**
 * The first available of `providers` (checked at each start): e.g. the organisation's server
 * when configured, else the browser's recogniser.
 */
export class AutoDictationProvider implements DictationProvider {
  private chosen: DictationProvider | null;

  constructor(private readonly providers: readonly DictationProvider[]) {
    this.chosen = providers[0] ?? null;
  }

  get kind(): DictationProvider['kind'] {
    return this.chosen?.kind ?? 'browser';
  }

  private async pick(): Promise<DictationProvider | null> {
    for (const p of this.providers) {
      if (await p.available().catch(() => false)) {
        this.chosen = p;
        return p;
      }
    }
    return null;
  }

  async available(): Promise<boolean> {
    return (await this.pick()) !== null;
  }

  async languages(): Promise<string[]> {
    return (await this.pick())?.languages() ?? [];
  }

  start(
    lang: string,
    options: DictationOptions,
    onEvent: (e: DictationEvent) => void,
  ): DictationSession {
    let inner: DictationSession | null = null;
    let state: 'starting' | 'running' | 'stopped' | 'aborted' = 'starting';
    void this.pick().then((p) => {
      if (state === 'aborted') return;
      if (!p) {
        onEvent({ type: 'error', code: 'unavailable' });
        return;
      }
      inner = p.start(lang, options, onEvent);
      if (state === 'stopped') inner.stop();
      else state = 'running';
    });
    return {
      stop: () => {
        if (state === 'starting') state = 'stopped';
        inner?.stop();
      },
      abort: () => {
        state = 'aborted';
        inner?.abort();
      },
    };
  }
}

// ------------------------------------------------------------------------------ fake

/** A session of the fake provider. */
export interface FakeDictationSession {
  lang: string;
  options: DictationOptions;
  stopped: boolean;
  aborted: boolean;
  ended: boolean;
}

/**
 * Provider for tests: `interim(text)` / `final(text)` report results to the running session,
 * `fail(code)` ends it with an error; `stop()` from the session reports `pendingOnStop` as a
 * final result (what a recogniser still had) then `end`. `startError` fails every start.
 */
export class FakeDictationProvider implements DictationProvider {
  readonly kind = 'fake' as const;
  readonly sessions: FakeDictationSession[] = [];
  /** Every start fails with this code. */
  startError: DictationErrorCode | null = null;
  /** Reported as a final result when the session is stopped. */
  pendingOnStop: string | null = null;
  private handlers = new Map<FakeDictationSession, (e: DictationEvent) => void>();

  constructor(
    private readonly langs: string[] = [],
    private readonly isAvailable = true,
  ) {}

  async available(): Promise<boolean> {
    return this.isAvailable;
  }

  async languages(): Promise<string[]> {
    return this.langs;
  }

  /** The running session (not ended, stopped-and-ended or aborted). */
  get current(): FakeDictationSession | undefined {
    return this.sessions.find((s) => !s.ended && !s.aborted);
  }

  start(
    lang: string,
    options: DictationOptions,
    onEvent: (e: DictationEvent) => void,
  ): DictationSession {
    const s: FakeDictationSession = {
      lang,
      options,
      stopped: false,
      aborted: false,
      ended: false,
    };
    this.sessions.push(s);
    if (this.startError) {
      s.ended = true;
      const code = this.startError;
      queueMicrotask(() => onEvent({ type: 'error', code }));
      return { stop: () => {}, abort: () => {} };
    }
    this.handlers.set(s, onEvent);
    onEvent({ type: 'start' });
    return {
      stop: () => {
        if (s.ended || s.aborted || s.stopped) return;
        s.stopped = true;
        const emit = this.handlers.get(s);
        if (this.pendingOnStop) {
          emit?.({ type: 'result', text: this.pendingOnStop, final: true });
          this.pendingOnStop = null;
        }
        s.ended = true;
        this.handlers.delete(s);
        emit?.({ type: 'end' });
      },
      abort: () => {
        s.aborted = true;
        this.handlers.delete(s);
      },
    };
  }

  private emit(e: DictationEvent): boolean {
    const s = this.current;
    const h = s && this.handlers.get(s);
    if (!s || !h) return false;
    h(e);
    return true;
  }

  interim(text: string): boolean {
    return this.emit({ type: 'result', text, final: false });
  }

  final(text: string, confidence?: number): boolean {
    return this.emit({
      type: 'result',
      text,
      final: true,
      ...(confidence !== undefined ? { confidence } : {}),
    });
  }

  /** The recogniser ends on its own. */
  end(): void {
    const s = this.current;
    if (!s) return;
    const h = this.handlers.get(s);
    s.ended = true;
    this.handlers.delete(s);
    h?.({ type: 'end' });
  }

  fail(code: DictationErrorCode = 'network'): void {
    const s = this.current;
    if (!s) return;
    const h = this.handlers.get(s);
    s.ended = true;
    this.handlers.delete(s);
    h?.({ type: 'error', code });
  }
}

// ------------------------------------------------------------------------------ platform default

/**
 * The dictation provider of a web / webview platform. `mode`: `off` (none), `browser` (the
 * runtime's recogniser only), `server` (the BlueOffice server only) or `auto` (default: the
 * server when an administrator configured dictation there, else the runtime's recogniser).
 */
export function defaultDictation(o: {
  mode?: string;
  /** The server provider's options without the microphone (absent: no server). */
  server?: Omit<ServerDictationOptions, 'capture'> | null;
  w?: unknown;
}): DictationProvider | undefined {
  const mode = (o.mode ?? 'auto').toLowerCase();
  if (mode === 'off') return undefined;
  const w = o.w ?? globalThis;
  const browser = mode === 'server' ? undefined : webDictation(w);
  const capture = webAudioCapture(w);
  const server =
    mode !== 'browser' && o.server && capture
      ? new ServerDictationProvider({ ...o.server, capture })
      : undefined;
  const list = [server, browser].filter((x): x is NonNullable<typeof x> => !!x);
  if (list.length <= 1) return list[0];
  return new AutoDictationProvider(list);
}
