/**
 * Handwriting recognition (CORE-INK-002): ink → text and ink → math. No licence-compatible
 * handwriting model ships with BlueOffice, so editors ask the platform's `HandwritingProvider`:
 *
 * - `BrowserHandwritingProvider` — the W3C Handwriting Recognition API
 *   (`navigator.createHandwritingRecognizer`), when the runtime offers it; text only.
 * - `ServerHandwritingProvider` — the BlueOffice server's `/api/handwriting`, which forwards the
 *   strokes to the recognition service an administrator configured (off by default:
 *   docs/deploy/handwriting.md); text and / or math as configured.
 * - `AutoHandwritingProvider` — per mode, the first provider offering it.
 * - `FakeHandwritingProvider` — deterministic, for tests.
 *
 * With no provider (or none offering a mode) the editor commands are disabled and say why.
 */

/** What to recognise. */
export type HandwritingMode = 'text' | 'math';

/** One stroke in slide points; `times` in ms (per point) when recorded. */
export interface HandwritingStroke {
  points: [number, number][];
  times?: number[];
}

export interface HandwritingRequest {
  mode: HandwritingMode;
  /** BCP 47 tag of the writing (`vi-VN`, `en-US`). */
  lang: string;
  strokes: HandwritingStroke[];
  signal?: AbortSignal;
}

/** Notation of a math result (both go through the equation import). */
export type MathFormat = 'unicodeMath' | 'latex';

export interface HandwritingResult {
  /** Best reading (text, or the equation in `format`). */
  text: string;
  /** Other readings, best first. */
  alternatives?: string[];
  /** For math: the notation of `text`. */
  format?: MathFormat;
}

/** What a provider can do now. */
export interface HandwritingCapabilities {
  text: boolean;
  math: boolean;
  /** Languages offered (primary subtags); empty = any. */
  languages: string[];
}

export type HandwritingErrorCode =
  | 'unavailable'
  | 'unsupportedLanguage'
  | 'unauthorized'
  | 'tooLarge'
  | 'rateLimited'
  | 'upstream'
  | 'empty';

export class HandwritingError extends Error {
  constructor(
    readonly code: HandwritingErrorCode,
    message?: string,
  ) {
    super(message ?? code);
    this.name = 'HandwritingError';
  }
}

export interface HandwritingProvider {
  readonly kind: 'browser' | 'server' | 'local' | 'fake';
  capabilities(): Promise<HandwritingCapabilities>;
  recognize(req: HandwritingRequest): Promise<HandwritingResult>;
}

const NONE: HandwritingCapabilities = { text: false, math: false, languages: [] };

/** Primary subtag (`vi-VN` → `vi`). */
export function primaryLanguage(tag: string): string {
  return (tag.split(/[-_]/)[0] ?? '').toLowerCase();
}

/** Times for a stroke: the recorded ones, else 10 ms per point after `start`. */
function strokeTimes(s: HandwritingStroke, start: number): number[] {
  if (s.times && s.times.length === s.points.length) return s.times.map((t) => start + t);
  return s.points.map((_, i) => start + i * 10);
}

// ------------------------------------------------------------------------------ browser

/** The parts of the W3C Handwriting Recognition API used here. */
interface HwPoint {
  x: number;
  y: number;
  t?: number;
}
interface HwStroke {
  addPoint(p: HwPoint): void;
}
interface HwDrawing {
  addStroke(s: HwStroke): void;
  getPrediction(): Promise<{ text: string }[]>;
}
interface HwRecognizer {
  startDrawing(hints?: {
    recognitionType?: string;
    inputType?: string;
    alternatives?: number;
  }): HwDrawing;
  finish(): void;
}
interface HwNavigator {
  createHandwritingRecognizer?: (c: { languages: string[] }) => Promise<HwRecognizer>;
  queryHandwritingRecognizer?: (c: { languages: string[] }) => Promise<unknown>;
}
type HwStrokeCtor = new () => HwStroke;

/** The runtime's handwriting recogniser (text; math is not part of the API). */
export class BrowserHandwritingProvider implements HandwritingProvider {
  readonly kind = 'browser' as const;

  constructor(
    private readonly nav: HwNavigator,
    private readonly Stroke: HwStrokeCtor,
    /** Language checked by `capabilities` (the editor's language). */
    private readonly lang: () => string = () => 'vi',
  ) {}

  async capabilities(): Promise<HandwritingCapabilities> {
    const lang = primaryLanguage(this.lang());
    if (!this.nav.createHandwritingRecognizer) return NONE;
    if (this.nav.queryHandwritingRecognizer) {
      try {
        const q = await this.nav.queryHandwritingRecognizer({ languages: [lang] });
        if (!q) return NONE;
      } catch {
        return NONE;
      }
    }
    return { text: true, math: false, languages: [lang] };
  }

  async recognize(req: HandwritingRequest): Promise<HandwritingResult> {
    if (req.mode !== 'text' || !this.nav.createHandwritingRecognizer) {
      throw new HandwritingError('unavailable');
    }
    if (!req.strokes.some((s) => s.points.length > 0)) throw new HandwritingError('empty');
    let rec: HwRecognizer;
    try {
      rec = await this.nav.createHandwritingRecognizer({
        languages: [primaryLanguage(req.lang)],
      });
    } catch {
      throw new HandwritingError('unsupportedLanguage');
    }
    try {
      const drawing = rec.startDrawing({ recognitionType: 'text', alternatives: 3 });
      let start = 0;
      for (const s of req.strokes) {
        if (s.points.length === 0) continue;
        const st = new this.Stroke();
        const times = strokeTimes(s, start);
        s.points.forEach((p, i) => st.addPoint({ x: p[0], y: p[1], t: times[i] }));
        start = (times[times.length - 1] ?? start) + 100;
        drawing.addStroke(st);
      }
      if (req.signal?.aborted) throw new HandwritingError('unavailable', 'aborted');
      const preds = await drawing.getPrediction();
      const texts = preds.map((p) => p.text).filter((t) => t.trim().length > 0);
      if (texts.length === 0) throw new HandwritingError('empty');
      return { text: texts[0]!, ...(texts.length > 1 ? { alternatives: texts.slice(1) } : {}) };
    } finally {
      rec.finish();
    }
  }
}

/** The runtime's recogniser when `navigator.createHandwritingRecognizer` exists. */
export function webHandwriting(
  w: unknown = globalThis,
  lang?: () => string,
): BrowserHandwritingProvider | undefined {
  const g = w as { navigator?: HwNavigator; HandwritingStroke?: HwStrokeCtor };
  const nav = g.navigator;
  if (!nav || typeof nav.createHandwritingRecognizer !== 'function' || !g.HandwritingStroke) {
    return undefined;
  }
  return new BrowserHandwritingProvider(nav, g.HandwritingStroke, lang);
}

// ------------------------------------------------------------------------------ server

/** `fetch` as the providers use it. */
export type HandwritingFetch = (
  url: string,
  init?: { method?: string; headers?: Record<string, string>; body?: string; signal?: AbortSignal },
) => Promise<{ ok: boolean; status: number; json(): Promise<unknown> }>;

export interface ServerHandwritingOptions {
  /** Server origin ('' = same origin). */
  baseUrl: string;
  headers?: () => Promise<Record<string, string>> | Record<string, string>;
  /** Editor session id (hosted files). */
  session?: string;
  fetch?: HandwritingFetch;
}

/** Error code of a server answer. */
export function handwritingServerErrorCode(status: number, code?: unknown): HandwritingErrorCode {
  if (code === 'unsupportedLanguage') return 'unsupportedLanguage';
  if (status === 404 || status === 501) return 'unavailable';
  if (status === 401 || status === 403) return 'unauthorized';
  if (status === 413) return 'tooLarge';
  if (status === 429) return 'rateLimited';
  if (status === 400) return code === 'unsupportedMode' ? 'unavailable' : 'unsupportedLanguage';
  return 'upstream';
}

/** The BlueOffice server (`GET /api/handwriting/capabilities`, `POST /api/handwriting`). */
export class ServerHandwritingProvider implements HandwritingProvider {
  readonly kind = 'server' as const;
  private readonly fetchImpl: HandwritingFetch;
  private caps: Promise<HandwritingCapabilities & { mathFormat: MathFormat }> | null = null;

  constructor(private readonly opts: ServerHandwritingOptions) {
    this.fetchImpl =
      opts.fetch ??
      ((u, init) => fetch(u, init as RequestInit) as unknown as ReturnType<HandwritingFetch>);
  }

  private url(path: string, q: URLSearchParams = new URLSearchParams()): string {
    if (this.opts.session) q.set('session', this.opts.session);
    const qs = q.toString();
    return `${this.opts.baseUrl.replace(/\/+$/, '')}${path}${qs ? `?${qs}` : ''}`;
  }

  private lookup(): Promise<HandwritingCapabilities & { mathFormat: MathFormat }> {
    this.caps ??= (async () => {
      const off = { ...NONE, mathFormat: 'unicodeMath' as MathFormat };
      try {
        const res = await this.fetchImpl(this.url('/api/handwriting/capabilities'), {
          headers: { ...(await this.opts.headers?.()) },
        });
        if (!res.ok) return off;
        const b = (await res.json()) as {
          modes?: unknown;
          languages?: unknown;
          mathFormat?: unknown;
        };
        const modes = Array.isArray(b.modes) ? b.modes : [];
        return {
          text: modes.includes('text'),
          math: modes.includes('math'),
          languages: Array.isArray(b.languages)
            ? b.languages.filter((l): l is string => typeof l === 'string' && !!l)
            : [],
          mathFormat: b.mathFormat === 'latex' ? 'latex' : 'unicodeMath',
        };
      } catch {
        return off;
      }
    })();
    // A failed or empty lookup is retried next time.
    void this.caps.then((c) => {
      if (!c.text && !c.math) this.caps = null;
    });
    return this.caps;
  }

  async capabilities(): Promise<HandwritingCapabilities> {
    const { text, math, languages } = await this.lookup();
    return { text, math, languages };
  }

  async recognize(req: HandwritingRequest): Promise<HandwritingResult> {
    const caps = await this.lookup();
    if (!caps[req.mode]) throw new HandwritingError('unavailable');
    const q = new URLSearchParams({ mode: req.mode, lang: req.lang });
    let start = 0;
    const strokes = req.strokes
      .filter((s) => s.points.length > 0)
      .map((s) => {
        const t = strokeTimes(s, start);
        start = (t[t.length - 1] ?? start) + 100;
        return { points: s.points.map((p, i) => [p[0], p[1], t[i]!]) };
      });
    if (strokes.length === 0) throw new HandwritingError('empty');
    let res: Awaited<ReturnType<HandwritingFetch>>;
    try {
      res = await this.fetchImpl(this.url('/api/handwriting', q), {
        method: 'POST',
        headers: { ...(await this.opts.headers?.()), 'content-type': 'application/json' },
        body: JSON.stringify({ strokes }),
        ...(req.signal ? { signal: req.signal } : {}),
      });
    } catch {
      throw new HandwritingError('upstream', 'network');
    }
    if (!res.ok) {
      let code: unknown;
      try {
        code = ((await res.json()) as { code?: unknown }).code;
      } catch {
        code = undefined;
      }
      throw new HandwritingError(
        handwritingServerErrorCode(res.status, code),
        `HTTP ${res.status}`,
      );
    }
    let body: { text?: unknown; alternatives?: unknown; format?: unknown };
    try {
      body = (await res.json()) as typeof body;
    } catch {
      throw new HandwritingError('upstream', 'invalid answer');
    }
    if (typeof body.text !== 'string') throw new HandwritingError('upstream', 'invalid answer');
    if (!body.text.trim()) throw new HandwritingError('empty');
    const alternatives = Array.isArray(body.alternatives)
      ? body.alternatives.filter((a): a is string => typeof a === 'string' && !!a.trim())
      : [];
    const format: MathFormat | undefined =
      req.mode === 'math'
        ? body.format === 'latex' || body.format === 'unicodeMath'
          ? body.format
          : caps.mathFormat
        : undefined;
    return {
      text: body.text,
      ...(alternatives.length ? { alternatives } : {}),
      ...(format ? { format } : {}),
    };
  }
}

// ------------------------------------------------------------------------------ choice

/** Per mode, the first of `providers` offering it (e.g. the organisation's server, else the browser). */
export class AutoHandwritingProvider implements HandwritingProvider {
  constructor(private readonly providers: readonly HandwritingProvider[]) {}

  get kind(): HandwritingProvider['kind'] {
    return this.providers[0]?.kind ?? 'browser';
  }

  async capabilities(): Promise<HandwritingCapabilities> {
    const all = await Promise.all(this.providers.map((p) => p.capabilities().catch(() => NONE)));
    return {
      text: all.some((c) => c.text),
      math: all.some((c) => c.math),
      languages: [...new Set(all.flatMap((c) => c.languages))],
    };
  }

  async recognize(req: HandwritingRequest): Promise<HandwritingResult> {
    for (const p of this.providers) {
      const c = await p.capabilities().catch(() => NONE);
      if (c[req.mode]) return p.recognize(req);
    }
    throw new HandwritingError('unavailable');
  }
}

// ------------------------------------------------------------------------------ fake

/** Provider for tests: answers queued results (or fails with `error`) and records requests. */
export class FakeHandwritingProvider implements HandwritingProvider {
  readonly kind = 'fake' as const;
  readonly requests: HandwritingRequest[] = [];
  readonly answers: HandwritingResult[] = [];
  error: HandwritingErrorCode | null = null;

  constructor(
    private readonly caps: HandwritingCapabilities = { text: true, math: true, languages: [] },
  ) {}

  async capabilities(): Promise<HandwritingCapabilities> {
    return this.caps;
  }

  async recognize(req: HandwritingRequest): Promise<HandwritingResult> {
    this.requests.push(req);
    if (!this.caps[req.mode]) throw new HandwritingError('unavailable');
    if (this.error) throw new HandwritingError(this.error);
    const a = this.answers.shift();
    if (!a) throw new HandwritingError('empty');
    return a;
  }
}

// ------------------------------------------------------------------------------ platform default

/**
 * The handwriting provider of a web / webview platform. `mode`: `off` (none), `browser` (the
 * runtime's recogniser only), `server` (the BlueOffice server only) or `auto` (default: the server
 * for what an administrator configured there, the runtime's recogniser for the rest).
 */
export function defaultHandwriting(o: {
  mode?: string;
  server?: ServerHandwritingOptions | null;
  w?: unknown;
  lang?: () => string;
}): HandwritingProvider | undefined {
  const mode = (o.mode ?? 'auto').toLowerCase();
  if (mode === 'off') return undefined;
  const browser = mode === 'server' ? undefined : webHandwriting(o.w ?? globalThis, o.lang);
  const server =
    mode !== 'browser' && o.server ? new ServerHandwritingProvider(o.server) : undefined;
  const list = [server, browser].filter((x): x is NonNullable<typeof x> => !!x);
  if (list.length <= 1) return list[0];
  return new AutoHandwritingProvider(list);
}
