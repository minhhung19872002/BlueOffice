/**
 * BlueAI (CORE-AI-001): the writing / data assistant. Editors never call a language model
 * themselves: they ask the platform's `AiProvider`. Providers:
 *
 * - `ServerAiProvider` — the BlueOffice server's `POST /api/ai/complete`, which forwards to the
 *   chat-completions service an administrator configured (off by default: nothing leaves the
 *   organisation unless enabled; `docs/deploy/ai.md`). The browser never holds the service's key;
 *   the server never logs prompts or answers.
 * - `FakeAiProvider` — deterministic, for tests.
 *
 * No model ships with BlueOffice. A provider answers with **untrusted text**: the helpers here
 * turn it into plain text (`plainText`), a schema-checked outline (`parseAiOutline`) or one
 * formula line (`extractFormula`, which the sheet engine then parses before it is offered).
 * Nothing an answer contains is ever executed, rendered as markup or applied without the user
 * confirming it in the pane.
 */

/** What the assistant is asked to do (the server holds the instructions of each task). */
export type AiTask = 'rewrite' | 'summarize' | 'draft' | 'outline' | 'formula' | 'analyze';

export const AI_TASKS: readonly AiTask[] = [
  'rewrite',
  'summarize',
  'draft',
  'outline',
  'formula',
  'analyze',
];

/** Rewrite tone presets (formal = administrative register). */
export type AiTone = 'formal' | 'concise' | 'friendly';

export const AI_TONES: readonly AiTone[] = ['formal', 'concise', 'friendly'];

export interface AiRequest {
  task: AiTask;
  /** The material: selected text, document text, cells. */
  text?: string;
  /** The user's own request (draft / outline topic / formula description). */
  instruction?: string;
  tone?: AiTone;
  /** Answer language (primary subtag: `vi`, `en`). */
  language?: string;
}

/** What the user is told before sending anything. */
export interface AiInfo {
  /** Host name of the service the administrator configured (where the text goes). */
  host: string;
  /** Most characters of one request (text + instruction). */
  maxInputChars: number;
}

/** Why a request failed. */
export type AiErrorCode =
  /** No provider on this platform / BlueAI not enabled on the server. */
  | 'unavailable'
  /** Not signed in / no session. */
  | 'unauthorized'
  /** Too much text in one request. */
  | 'tooLarge'
  | 'rateLimited'
  /** The service failed. */
  | 'upstream'
  | 'timeout'
  | 'network'
  | 'cancelled'
  /** Nothing to send. */
  | 'empty'
  /** The answer is not what the task needs (not an outline, not a formula). */
  | 'badAnswer';

export class AiError extends Error {
  constructor(
    readonly code: AiErrorCode,
    message?: string,
    /** `tooLarge`: the limit and the size of the refused request. */
    readonly limit?: { max: number; size: number },
  ) {
    super(message ?? code);
    this.name = 'AiError';
  }
}

/** `e` as an `AiError` (aborts become `cancelled`, others `network`). */
export function aiError(e: unknown): AiError {
  if (e instanceof AiError) return e;
  if (e instanceof DOMException && e.name === 'AbortError') return new AiError('cancelled');
  return new AiError('network', e instanceof Error ? e.message : String(e));
}

export interface AiOptions {
  signal?: AbortSignal;
}

/** A language-model service. */
export interface AiProvider {
  /** `null`: BlueAI is off (not enabled by the administrator); nothing may be sent. */
  info(): Promise<AiInfo | null>;
  /** The answer to `request` (untrusted text). Throws `AiError`. */
  complete(request: AiRequest, options?: AiOptions): Promise<string>;
}

// ------------------------------------------------------------------------- shared helpers

/** Request size limit used when the provider names none. */
export const AI_DEFAULT_MAX_CHARS = 20_000;
/** Longest answer kept (the rest is dropped). */
export const AI_MAX_ANSWER_CHARS = 100_000;

/** Characters a request counts for (as the server counts them). */
export function aiRequestSize(request: Pick<AiRequest, 'text' | 'instruction'>): number {
  return (request.text?.length ?? 0) + (request.instruction?.length ?? 0);
}

/**
 * An answer as plain text: line ends normalised, control characters (anything that is not a
 * tab or a line feed) removed, surrounding blank space trimmed, capped in length.
 */
export function plainText(answer: string): string {
  let out = '';
  for (const ch of answer.slice(0, AI_MAX_ANSWER_CHARS).replace(/\r\n?/g, '\n')) {
    const c = ch.codePointAt(0) ?? 0;
    // C0 controls except tab and line feed, DEL and C1 controls, line / paragraph separators,
    // the byte order mark.
    const control =
      (c < 0x20 && c !== 0x09 && c !== 0x0a) ||
      (c >= 0x7f && c <= 0x9f) ||
      c === 0x2028 ||
      c === 0x2029 ||
      c === 0xfeff;
    if (!control) out += ch;
  }
  return out.trim();
}

const throwIfAborted = (signal?: AbortSignal) => {
  if (signal?.aborted) throw new AiError('cancelled');
};

/**
 * Sends `request` through `provider` with the checks every caller needs: BlueAI must be on,
 * the request must not be empty nor exceed the provider's size limit (refused **before**
 * anything is sent), and the answer comes back as plain text.
 */
export async function aiComplete(
  provider: AiProvider | undefined,
  request: AiRequest,
  options: AiOptions = {},
): Promise<string> {
  throwIfAborted(options.signal);
  if (!provider) throw new AiError('unavailable');
  let info: AiInfo | null;
  try {
    info = await provider.info();
  } catch (e) {
    throw aiError(e);
  }
  if (!info) throw new AiError('unavailable');
  const size = aiRequestSize(request);
  if (!request.text?.trim() && !request.instruction?.trim()) throw new AiError('empty');
  const max = info.maxInputChars > 0 ? info.maxInputChars : AI_DEFAULT_MAX_CHARS;
  if (size > max) throw new AiError('tooLarge', undefined, { max, size });
  throwIfAborted(options.signal);
  let answer: string;
  try {
    answer = await provider.complete(request, options);
  } catch (e) {
    throw aiError(e);
  }
  throwIfAborted(options.signal);
  if (typeof answer !== 'string') throw new AiError('badAnswer');
  const text = plainText(answer);
  if (!text) throw new AiError('badAnswer');
  return text;
}

// ------------------------------------------------------------------------------ outlines

/** A slide of an outline: a title and bullets (level 0–8). */
export interface AiOutlineSlide {
  title: string;
  bullets: { text: string; level: number }[];
}

export const AI_OUTLINE_LIMITS = { slides: 50, bullets: 20, titleChars: 300, bulletChars: 500 };
/** Deepest bullet level (ECMA-376 Part 1, §21.1.2.2.7 `pPr@lvl` 0–8). */
const MAX_LEVEL = 8;

const oneLine = (s: string) => plainText(s).replace(/\s+/g, ' ');

const BULLET_MARK = /^(?:[-*+•–—·]|\d{1,3}[.)])\s+/u;

/**
 * Rule-based outline → slides, no AI involved: every line that is neither indented nor starts
 * with a list mark is a slide title; the other lines are bullets of the slide above (one level
 * per tab or per two spaces of indentation beyond the slide's least indented bullet). Blank
 * lines are skipped;
 * `#` heading marks of titles are dropped. Bullets before any title go to an untitled slide.
 */
export function outlineFromText(text: string): AiOutlineSlide[] {
  const slides: AiOutlineSlide[] = [];
  for (const raw of text.replace(/\r\n?/g, '\n').split('\n')) {
    if (!raw.trim()) continue;
    const indentText = /^[ \t]*/.exec(raw)![0];
    const indent = [...indentText].reduce((n, ch) => n + (ch === '\t' ? 2 : 1), 0);
    const depth = Math.floor(indent / 2);
    const body = raw.trim();
    const marked = BULLET_MARK.test(body);
    if (depth === 0 && !marked) {
      if (slides.length >= AI_OUTLINE_LIMITS.slides) break;
      const title = oneLine(body.replace(/^#{1,6}\s+/, '')).slice(0, AI_OUTLINE_LIMITS.titleChars);
      slides.push({ title, bullets: [] });
      continue;
    }
    if (!slides.length) slides.push({ title: '', bullets: [] });
    const slide = slides[slides.length - 1]!;
    if (slide.bullets.length >= AI_OUTLINE_LIMITS.bullets) continue;
    const textOnly = oneLine(body.replace(BULLET_MARK, '')).slice(0, AI_OUTLINE_LIMITS.bulletChars);
    if (!textOnly) continue;
    slide.bullets.push({ text: textOnly, level: depth });
  }
  // Levels are relative to the least indented bullet of each slide.
  for (const slide of slides) {
    const min = Math.min(...slide.bullets.map((b) => b.level));
    for (const b of slide.bullets) b.level = Math.min(MAX_LEVEL, b.level - min);
  }
  return slides;
}

/** The JSON object inside an answer (code fences and text around it dropped). */
function jsonObjectOf(answer: string): unknown {
  const text = plainText(answer);
  const start = text.indexOf('{');
  const end = text.lastIndexOf('}');
  if (start < 0 || end <= start) throw new AiError('badAnswer', 'no JSON object');
  try {
    return JSON.parse(text.slice(start, end + 1)) as unknown;
  } catch {
    throw new AiError('badAnswer', 'invalid JSON');
  }
}

const isRecord = (v: unknown): v is Record<string, unknown> =>
  typeof v === 'object' && v !== null && !Array.isArray(v);

/**
 * The outline of a provider answer, checked against the schema
 * `{ "slides": [{ "title": string, "bullets": string[] }] }` — nothing else is accepted (no
 * other keys, no nested objects, within `AI_OUTLINE_LIMITS`); strings become single lines of
 * plain text. Throws `AiError('badAnswer')` otherwise.
 */
export function parseAiOutline(answer: string): AiOutlineSlide[] {
  const bad = (why: string) => new AiError('badAnswer', why);
  const root = jsonObjectOf(answer);
  if (!isRecord(root)) throw bad('not an object');
  if (Object.keys(root).some((k) => k !== 'slides')) throw bad('unexpected key');
  const slides = root.slides;
  if (!Array.isArray(slides) || !slides.length) throw bad('no slides');
  if (slides.length > AI_OUTLINE_LIMITS.slides) throw bad('too many slides');
  return slides.map((s: unknown): AiOutlineSlide => {
    if (!isRecord(s)) throw bad('slide is not an object');
    if (Object.keys(s).some((k) => k !== 'title' && k !== 'bullets')) throw bad('unexpected key');
    if (typeof s.title !== 'string') throw bad('title is not a string');
    const bullets = s.bullets ?? [];
    if (!Array.isArray(bullets)) throw bad('bullets is not a list');
    if (bullets.length > AI_OUTLINE_LIMITS.bullets) throw bad('too many bullets');
    const title = oneLine(s.title);
    if (title.length > AI_OUTLINE_LIMITS.titleChars) throw bad('title too long');
    return {
      title,
      bullets: bullets.flatMap((b: unknown) => {
        if (typeof b !== 'string') throw bad('bullet is not a string');
        const text = oneLine(b);
        if (text.length > AI_OUTLINE_LIMITS.bulletChars) throw bad('bullet too long');
        return text ? [{ text, level: 0 }] : [];
      }),
    };
  });
}

/** An outline as indented text (the preview shown before slides are created). */
export function outlineText(slides: readonly AiOutlineSlide[]): string {
  return slides
    .map((s) =>
      [s.title, ...s.bullets.map((b) => `${'  '.repeat(b.level + 1)}- ${b.text}`)].join('\n'),
    )
    .join('\n');
}

// ------------------------------------------------------------------------------ formulas

/** Longest formula text of a cell (ECMA-376 Part 1, §18.3.1.40 `f`: 8192 characters). */
export const AI_MAX_FORMULA_CHARS = 8192;

/**
 * The one formula of a provider answer: the first line starting with `=` (code fences and
 * back-ticks around it dropped). Throws `AiError('badAnswer')` when there is none. The caller
 * still has the sheet engine parse it before offering it.
 */
export function extractFormula(answer: string): string {
  const lines = plainText(answer)
    .split('\n')
    .map((l) =>
      l
        .trim()
        .replace(/^`+|`+$/g, '')
        .trim(),
    );
  const formula = lines.find((l) => l.startsWith('=') && l.length > 1);
  if (!formula || formula.length > AI_MAX_FORMULA_CHARS) throw new AiError('badAnswer');
  return formula;
}

// ------------------------------------------------------------------------------ server

/** The parts of `fetch` the server provider uses. */
export type AiFetch = (
  url: string,
  init?: {
    method?: string;
    headers?: Record<string, string>;
    body?: string;
    signal?: AbortSignal;
  },
) => Promise<{ ok: boolean; status: number; json(): Promise<unknown> }>;

export interface ServerAiOptions {
  /** BlueOffice server base URL (`https://office.example.vn`); `''` = same origin. */
  baseUrl: string;
  /** Extra headers per request (bearer token, `x-session-token`). */
  headers?: () => Promise<Record<string, string>> | Record<string, string>;
  /** Editor session id (hosted files), sent with the request. */
  session?: string;
  fetch?: AiFetch;
}

/** Error code of a server answer. */
export function aiServerErrorCode(status: number, code?: unknown): AiErrorCode {
  if (status === 404 || status === 501) return 'unavailable';
  if (status === 401 || status === 403) return 'unauthorized';
  if (status === 413) return 'tooLarge';
  if (status === 429) return 'rateLimited';
  if (status === 504 || code === 'timeout') return 'timeout';
  if (status === 400) return 'empty';
  return 'upstream';
}

/** The BlueOffice server's BlueAI endpoints (`GET /api/ai/config`, `POST /api/ai/complete`). */
export class ServerAiProvider implements AiProvider {
  private readonly fetchImpl: AiFetch;
  private known: Promise<AiInfo | null> | null = null;

  constructor(private readonly opts: ServerAiOptions) {
    this.fetchImpl = opts.fetch ?? ((i, init) => fetch(i, init));
  }

  private url(path: string): string {
    return `${this.opts.baseUrl.replace(/\/+$/, '')}${path}`;
  }

  private async headers(): Promise<Record<string, string>> {
    return { ...(await this.opts.headers?.()) };
  }

  info(): Promise<AiInfo | null> {
    if (this.known) return this.known;
    const current = (async (): Promise<AiInfo | null> => {
      const q = this.opts.session ? `?session=${encodeURIComponent(this.opts.session)}` : '';
      try {
        const res = await this.fetchImpl(this.url(`/api/ai/config${q}`), {
          headers: await this.headers(),
        });
        if (!res.ok) return null;
        const body = (await res.json()) as {
          enabled?: unknown;
          host?: unknown;
          maxInputChars?: unknown;
        };
        if (body.enabled !== true) return null;
        return {
          host: typeof body.host === 'string' ? body.host : '',
          maxInputChars:
            typeof body.maxInputChars === 'number' && body.maxInputChars > 0
              ? body.maxInputChars
              : AI_DEFAULT_MAX_CHARS,
        };
      } catch {
        // Unreachable, or not a BlueOffice server (a static site answers HTML): off.
        return null;
      }
    })();
    this.known = current;
    // "Off" is looked up again next time (the administrator may have enabled it meanwhile).
    void current.then((i) => {
      if (!i && this.known === current) this.known = null;
    });
    return current;
  }

  async complete(request: AiRequest, options: AiOptions = {}): Promise<string> {
    const headers = { ...(await this.headers()), 'content-type': 'application/json' };
    throwIfAborted(options.signal);
    let res: Awaited<ReturnType<AiFetch>>;
    try {
      res = await this.fetchImpl(this.url('/api/ai/complete'), {
        method: 'POST',
        headers,
        body: JSON.stringify({
          task: request.task,
          text: request.text ?? '',
          instruction: request.instruction ?? '',
          ...(request.tone ? { tone: request.tone } : {}),
          ...(request.language ? { language: request.language } : {}),
          ...(this.opts.session ? { session: this.opts.session } : {}),
        }),
        signal: options.signal,
      });
    } catch (e) {
      throw aiError(e);
    }
    if (!res.ok) {
      let code: unknown;
      try {
        code = ((await res.json()) as { code?: unknown }).code;
      } catch {
        code = undefined;
      }
      // The server turned BlueAI off meanwhile: forget the cached "on".
      if (res.status === 404) this.known = null;
      throw new AiError(aiServerErrorCode(res.status, code), `HTTP ${res.status}`);
    }
    let body: { text?: unknown };
    try {
      body = (await res.json()) as { text?: unknown };
    } catch {
      throw new AiError('upstream', 'invalid answer');
    }
    if (typeof body.text !== 'string') throw new AiError('upstream', 'invalid answer');
    return body.text;
  }
}

// ------------------------------------------------------------------------------ fake

/**
 * Deterministic provider for tests. Answers are computed from the request alone:
 * `rewrite` → `[tone] text`; `summarize` → `Summary (<n> chars): <first 40 chars>`;
 * `draft` → `Draft: <instruction>`; `outline` → JSON with one slide per `;`-separated part of
 * the instruction (bullets after `:` split on `,`); `formula` → `=SUM(A1:A3)`;
 * `analyze` → `Rows: <lines of text>`. `answers` overrides a task's answer; `failWith` throws;
 * `gate` holds each call (tests cancel meanwhile); `off()` turns BlueAI off.
 */
export class FakeAiProvider implements AiProvider {
  calls: AiRequest[] = [];
  /** Throws this code on the next calls. */
  failWith: AiErrorCode | null = null;
  /** Each call waits for this promise. */
  gate: Promise<void> | null = null;
  /** Fixed answers per task. */
  answers: Partial<Record<AiTask, string>> = {};
  private current: AiInfo | null;

  constructor(info: AiInfo | null = { host: 'ai.test', maxInputChars: 2000 }) {
    this.current = info;
  }

  /** Turns BlueAI off (as when the administrator has not enabled it). */
  off(): void {
    this.current = null;
  }

  async info(): Promise<AiInfo | null> {
    return this.current;
  }

  async complete(request: AiRequest, options: AiOptions = {}): Promise<string> {
    this.calls.push({ ...request });
    if (this.gate) {
      const signal = options.signal;
      await Promise.race([
        this.gate,
        new Promise<void>((_, reject) => {
          if (signal?.aborted) reject(new AiError('cancelled'));
          signal?.addEventListener('abort', () => reject(new AiError('cancelled')));
        }),
      ]);
    }
    if (options.signal?.aborted) throw new AiError('cancelled');
    if (!this.current) throw new AiError('unavailable');
    if (this.failWith) throw new AiError(this.failWith);
    const fixed = this.answers[request.task];
    if (fixed !== undefined) return fixed;
    const text = request.text ?? '';
    const instruction = request.instruction ?? '';
    switch (request.task) {
      case 'rewrite':
        return `[${request.tone ?? 'concise'}] ${text}`;
      case 'summarize':
        return `Summary (${text.length} chars): ${text.slice(0, 40)}`;
      case 'draft':
        return `Draft: ${instruction}`;
      case 'outline':
        return JSON.stringify({
          slides: instruction
            .split(';')
            .map((p) => p.trim())
            .filter(Boolean)
            .map((p) => {
              const [title = '', rest = ''] = p.split(':');
              return {
                title: title.trim(),
                bullets: rest
                  .split(',')
                  .map((b) => b.trim())
                  .filter(Boolean),
              };
            }),
        });
      case 'formula':
        return '=SUM(A1:A3)';
      case 'analyze':
        return `Rows: ${text.split('\n').filter((l) => l.trim()).length}`;
    }
  }
}
