/**
 * Translation (CORE-LANG-001). Editors never call a translation service themselves: they ask the
 * platform's `TranslationProvider`. Providers:
 *
 * - `ServerTranslationProvider` — the BlueOffice server's `POST /api/translate`, which forwards
 *   to the HTTP translation API an administrator configured (off by default: nothing leaves the
 *   organisation unless configured; `docs/deploy/translation.md`). The server never logs text.
 * - `LocalTranslationProvider` — the slot for an on-device model (`LocalTranslationModel`); no
 *   model ships with BlueOffice (licence review pending), so no platform wires one yet.
 * - `FakeTranslationProvider` — deterministic, for tests.
 *
 * `translateTexts` adds what every caller needs on top of a provider: source-language detection
 * for `auto` (Vietnamese diacritics, `detectLanguage`), texts already in the target language left
 * as they are, surrounding white space kept, batching, progress and cancellation.
 */
import { detectLanguage } from './speech';

/** A language the provider translates (primary subtag: `vi`, `en`…). */
export interface TranslationLanguage {
  code: string;
}

/** Why a translation failed. */
export type TranslationErrorCode =
  /** No provider on this platform / nothing configured on the server. */
  | 'unavailable'
  /** Not signed in / no session. */
  | 'unauthorized'
  | 'unsupportedLanguage'
  /** Too much text in one request. */
  | 'tooLarge'
  | 'rateLimited'
  /** The translation service failed or answered something unusable. */
  | 'upstream'
  | 'network'
  | 'cancelled'
  /** Source and target are the same language. */
  | 'sameLanguage';

export class TranslationError extends Error {
  constructor(
    readonly code: TranslationErrorCode,
    message?: string,
  ) {
    super(message ?? code);
    this.name = 'TranslationError';
  }
}

/** `e` as a `TranslationError` (aborts become `cancelled`, others `network`). */
export function translationError(e: unknown): TranslationError {
  if (e instanceof TranslationError) return e;
  if (e instanceof DOMException && e.name === 'AbortError')
    return new TranslationError('cancelled');
  return new TranslationError('network', e instanceof Error ? e.message : String(e));
}

export interface TranslateOptions {
  signal?: AbortSignal;
}

/** A translation service. */
export interface TranslationProvider {
  /** Languages offered (empty: the service is not configured). */
  languages(): Promise<TranslationLanguage[]>;
  /**
   * Translates `texts` (same length and order back) from `from` (`auto`: the service detects)
   * to `to`. Throws `TranslationError`.
   */
  translate(
    texts: readonly string[],
    from: string,
    to: string,
    options?: TranslateOptions,
  ): Promise<string[]>;
  /** Largest batch the provider accepts in one call (texts, characters). */
  readonly limits?: { texts: number; chars: number };
}

// ------------------------------------------------------------------------- shared helpers

/** Default batch limits (the server's defaults are larger). */
export const DEFAULT_BATCH = { texts: 50, chars: 5000 };

/** Splits `texts` (indices) into batches within `limits`; a text longer than the limit is alone. */
export function batches(
  texts: readonly string[],
  limits: { texts: number; chars: number } = DEFAULT_BATCH,
): number[][] {
  const out: number[][] = [];
  let cur: number[] = [];
  let chars = 0;
  texts.forEach((t, i) => {
    if (cur.length && (cur.length >= limits.texts || chars + t.length > limits.chars)) {
      out.push(cur);
      cur = [];
      chars = 0;
    }
    cur.push(i);
    chars += t.length;
  });
  if (cur.length) out.push(cur);
  return out;
}

const throwIfAborted = (signal?: AbortSignal) => {
  if (signal?.aborted) throw new TranslationError('cancelled');
};

/** Leading white space, core, trailing white space. */
function trimParts(t: string): [string, string, string] {
  const m = /^(\s*)([\s\S]*?)(\s*)$/u.exec(t);
  return m ? [m[1]!, m[2]!, m[3]!] : ['', t, ''];
}

// Letters only Vietnamese uses among Latin scripts (as `detectLanguage`).
const VI_LETTER = /[ăâđêôơưàảãáạằẳẵắặầẩẫấậèẻẽéẹềểễếệìỉĩíịòỏõóọồổỗốộờởỡớợùủũúụừửữứựỳỷỹýỵ]/iu;

/**
 * Source language of a text for `from` (`auto`: detected, else `fallback` — the language of
 * the whole set — except that a text without a single Vietnamese letter is never assumed
 * Vietnamese: it goes as `auto` and the service decides).
 */
export function sourceLanguage(text: string, from: string, fallback: string | null): string {
  if (from !== 'auto') return from;
  const d = detectLanguage(text);
  if (d) return d;
  if (fallback === 'vi' && !VI_LETTER.test(text)) return 'auto';
  return fallback ?? 'auto';
}

export interface TranslateTextsOptions extends TranslateOptions {
  /** Called after each batch with texts done / total. */
  onProgress?(done: number, total: number): void;
}

/**
 * Translates `texts` through `provider`: blank texts and texts already in `to` come back as
 * they are; with `from` = `auto` each text's language is detected (the whole set's language
 * when a text alone is ambiguous); surrounding white space is kept; batches follow the
 * provider's limits; `signal` cancels between and during batches.
 */
export async function translateTexts(
  provider: TranslationProvider,
  texts: readonly string[],
  from: string,
  to: string,
  options: TranslateTextsOptions = {},
): Promise<string[]> {
  const { signal, onProgress } = options;
  throwIfAborted(signal);
  if (from !== 'auto' && from === to) throw new TranslationError('sameLanguage');
  const out = [...texts];
  const parts = texts.map(trimParts);
  const overall = from === 'auto' ? detectLanguage(texts.join('\n')) : null;
  // Group the texts to translate by source language.
  const groups = new Map<string, number[]>();
  parts.forEach(([, core], i) => {
    if (!/\p{L}/u.test(core)) return;
    const src = sourceLanguage(core, from, overall);
    if (src === to) return;
    const g = groups.get(src) ?? [];
    g.push(i);
    groups.set(src, g);
  });
  const total = [...groups.values()].reduce((n, g) => n + g.length, 0);
  if (total === 0 && from === 'auto' && texts.some((t) => /\p{L}/u.test(t)))
    throw new TranslationError('sameLanguage');
  let done = 0;
  onProgress?.(0, total);
  for (const [src, idx] of groups) {
    const cores = idx.map((i) => parts[i]![1]);
    for (const b of batches(cores, provider.limits ?? DEFAULT_BATCH)) {
      throwIfAborted(signal);
      let res: string[];
      try {
        res = await provider.translate(
          b.map((k) => cores[k]!),
          src,
          to,
          { signal },
        );
      } catch (e) {
        throw translationError(e);
      }
      throwIfAborted(signal);
      if (res.length !== b.length) throw new TranslationError('upstream', 'count mismatch');
      b.forEach((k, j) => {
        const i = idx[k]!;
        const [lead, , trail] = parts[i]!;
        out[i] = lead + res[j]! + trail;
      });
      done += b.length;
      onProgress?.(done, total);
    }
  }
  return out;
}

/** Default target: the other of Vietnamese / English for `uiLang`. */
export function defaultTarget(uiLang: string): string {
  return uiLang.toLowerCase().startsWith('vi') ? 'en' : 'vi';
}

// ------------------------------------------------------------------------------ server

/** The parts of `fetch` the server provider uses. */
export type TranslationFetch = (
  url: string,
  init?: {
    method?: string;
    headers?: Record<string, string>;
    body?: string;
    signal?: AbortSignal;
  },
) => Promise<{ ok: boolean; status: number; json(): Promise<unknown> }>;
type FetchLike = TranslationFetch;

export interface ServerTranslationOptions {
  /** BlueOffice server base URL (`https://office.example.vn`); `''` = same origin. */
  baseUrl: string;
  /** Extra headers per request (bearer token, `x-session-token`). */
  headers?: () => Promise<Record<string, string>> | Record<string, string>;
  /** Editor session id (hosted files), sent with the request. */
  session?: string;
  fetch?: FetchLike;
  limits?: { texts: number; chars: number };
}

/** Error code of a server answer. */
export function serverErrorCode(status: number, code?: unknown): TranslationErrorCode {
  if (typeof code === 'string' && code === 'unsupportedLanguage') return 'unsupportedLanguage';
  if (status === 404 || status === 501) return 'unavailable';
  if (status === 401 || status === 403) return 'unauthorized';
  if (status === 413) return 'tooLarge';
  if (status === 429) return 'rateLimited';
  if (status === 400) return code === 'sameLanguage' ? 'sameLanguage' : 'unsupportedLanguage';
  return 'upstream';
}

/** The BlueOffice server's translation endpoint (`POST /api/translate`). */
export class ServerTranslationProvider implements TranslationProvider {
  readonly limits: { texts: number; chars: number };
  private readonly fetchImpl: FetchLike;
  private langs: Promise<TranslationLanguage[]> | null = null;

  constructor(private readonly opts: ServerTranslationOptions) {
    this.limits = opts.limits ?? DEFAULT_BATCH;
    this.fetchImpl = opts.fetch ?? ((i, init) => fetch(i, init));
  }

  private url(path: string): string {
    return `${this.opts.baseUrl.replace(/\/+$/, '')}${path}`;
  }

  private async headers(): Promise<Record<string, string>> {
    return { ...(await this.opts.headers?.()) };
  }

  languages(): Promise<TranslationLanguage[]> {
    this.langs ??= (async () => {
      const q = this.opts.session ? `?session=${encodeURIComponent(this.opts.session)}` : '';
      try {
        const res = await this.fetchImpl(this.url(`/api/translate/languages${q}`), {
          headers: await this.headers(),
        });
        if (!res.ok) return [];
        const body = (await res.json()) as { languages?: unknown };
        return Array.isArray(body.languages)
          ? body.languages
              .filter((c): c is string => typeof c === 'string' && !!c)
              .map((code) => ({ code }))
          : [];
      } catch {
        return [];
      }
    })();
    // A failed lookup is retried next time.
    void this.langs.then((l) => {
      if (!l.length) this.langs = null;
    });
    return this.langs;
  }

  async translate(
    texts: readonly string[],
    from: string,
    to: string,
    options: TranslateOptions = {},
  ): Promise<string[]> {
    if (!texts.length) return [];
    const headers = { ...(await this.headers()), 'content-type': 'application/json' };
    throwIfAborted(options.signal);
    let res: Awaited<ReturnType<FetchLike>>;
    try {
      res = await this.fetchImpl(this.url('/api/translate'), {
        method: 'POST',
        headers,
        body: JSON.stringify({
          texts,
          from,
          to,
          ...(this.opts.session ? { session: this.opts.session } : {}),
        }),
        signal: options.signal,
      });
    } catch (e) {
      throw translationError(e);
    }
    if (!res.ok) {
      let code: unknown;
      try {
        code = ((await res.json()) as { code?: unknown }).code;
      } catch {
        code = undefined;
      }
      throw new TranslationError(serverErrorCode(res.status, code), `HTTP ${res.status}`);
    }
    let body: { texts?: unknown };
    try {
      body = (await res.json()) as { texts?: unknown };
    } catch {
      throw new TranslationError('upstream', 'invalid answer');
    }
    const out = body.texts;
    if (
      !Array.isArray(out) ||
      out.length !== texts.length ||
      !out.every((t) => typeof t === 'string')
    )
      throw new TranslationError('upstream', 'invalid answer');
    return out as string[];
  }
}

// ------------------------------------------------------------------------------ local model

/**
 * An on-device translation model (e.g. a WASM / ONNX Vietnamese↔English model with a licence
 * cleared for redistribution). None ships yet: this is the slot a model is dropped into.
 */
export interface LocalTranslationModel {
  /** Language pairs the model handles (`['vi', 'en']` = both directions between them). */
  readonly languages: readonly string[];
  /** Loads weights on first use. */
  load?(signal?: AbortSignal): Promise<void>;
  translate(
    texts: readonly string[],
    from: string,
    to: string,
    signal?: AbortSignal,
  ): Promise<string[]>;
}

/** A `TranslationProvider` over a `LocalTranslationModel` (nothing leaves the device). */
export class LocalTranslationProvider implements TranslationProvider {
  readonly limits = { texts: 16, chars: 2000 };
  private loaded: Promise<void> | null = null;

  constructor(private readonly model: LocalTranslationModel) {}

  async languages(): Promise<TranslationLanguage[]> {
    return this.model.languages.map((code) => ({ code }));
  }

  async translate(
    texts: readonly string[],
    from: string,
    to: string,
    options: TranslateOptions = {},
  ): Promise<string[]> {
    const langs = this.model.languages;
    if (!langs.includes(to) || (from !== 'auto' && !langs.includes(from)))
      throw new TranslationError('unsupportedLanguage');
    throwIfAborted(options.signal);
    try {
      this.loaded ??= this.model.load?.(options.signal) ?? Promise.resolve();
      await this.loaded;
    } catch (e) {
      this.loaded = null;
      throw translationError(e);
    }
    try {
      return await this.model.translate(texts, from, to, options.signal);
    } catch (e) {
      throw translationError(e);
    }
  }
}

// ------------------------------------------------------------------------------ fake

/**
 * Deterministic provider for tests: `[to] text` (or a word dictionary when given), counts calls,
 * can fail with a code or wait for cancellation.
 */
export class FakeTranslationProvider implements TranslationProvider {
  calls: { texts: string[]; from: string; to: string }[] = [];
  /** Throws this code on the next calls. */
  failWith: TranslationErrorCode | null = null;
  /** Each call waits for this promise (tests cancel meanwhile). */
  gate: Promise<void> | null = null;
  readonly limits: { texts: number; chars: number };

  constructor(
    private readonly langs: string[] = ['vi', 'en'],
    private readonly dictionary: Record<string, string> = {},
    limits: { texts: number; chars: number } = DEFAULT_BATCH,
  ) {
    this.limits = limits;
  }

  async languages(): Promise<TranslationLanguage[]> {
    return this.langs.map((code) => ({ code }));
  }

  async translate(
    texts: readonly string[],
    from: string,
    to: string,
    options: TranslateOptions = {},
  ): Promise<string[]> {
    this.calls.push({ texts: [...texts], from, to });
    if (this.gate) {
      const signal = options.signal;
      await Promise.race([
        this.gate,
        new Promise<void>((_, reject) => {
          if (signal?.aborted) reject(new TranslationError('cancelled'));
          signal?.addEventListener('abort', () => reject(new TranslationError('cancelled')));
        }),
      ]);
    }
    if (options.signal?.aborted) throw new TranslationError('cancelled');
    if (this.failWith) throw new TranslationError(this.failWith);
    if (!this.langs.includes(to)) throw new TranslationError('unsupportedLanguage');
    return texts.map((t) => this.dictionary[t] ?? `[${to}] ${t}`);
  }
}

// ----------------------------------------------------------- BlueDoc engine commands (bo_engine::translate)

/** A text segment of a body paragraph (`bo_edit::translate::TextSegment`). */
export interface DocTextSegment {
  /** Body paragraph id. */
  p: number;
  /** Segment index inside the paragraph. */
  s: number;
  text: string;
}

/** Whole-document translation commands of BlueDoc. */
export type DocTranslateCommand =
  /** Text segments of every body paragraph → `translation`. */
  | { cmd: 'translationSegments' }
  /** Writes translated segments back keeping run formatting (one undo step) → `count`. */
  | { cmd: 'translationApply'; segments: DocTextSegment[] };
