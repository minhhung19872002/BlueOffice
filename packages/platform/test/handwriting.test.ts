import { describe, expect, it } from 'vitest';
import {
  AutoHandwritingProvider,
  BrowserHandwritingProvider,
  FakeHandwritingProvider,
  HandwritingError,
  ServerHandwritingProvider,
  defaultHandwriting,
  handwritingServerErrorCode,
  primaryLanguage,
  webHandwriting,
  type HandwritingFetch,
} from '../src/handwriting';

const STROKES = [
  {
    points: [
      [10, 10],
      [20, 12],
    ] as [number, number][],
    times: [0, 16],
  },
  {
    points: [[30, 30]] as [number, number][],
  },
];

/** A fake W3C Handwriting Recognition API recording the strokes it gets. */
function fakeApi(predictions: string[], supported = true) {
  const log: {
    languages?: string[];
    strokes: { x: number; y: number; t?: number }[][];
    finished: number;
  } = {
    strokes: [],
    finished: 0,
  };
  class Stroke {
    readonly pts: { x: number; y: number; t?: number }[] = [];
    addPoint(p: { x: number; y: number; t?: number }) {
      this.pts.push(p);
    }
  }
  const navigator = {
    queryHandwritingRecognizer: async (c: { languages: string[] }) =>
      supported && c.languages[0] === 'vi' ? { textAlternatives: true } : null,
    createHandwritingRecognizer: async (c: { languages: string[] }) => {
      log.languages = c.languages;
      return {
        startDrawing: () => ({
          addStroke: (s: Stroke) => log.strokes.push(s.pts),
          getPrediction: async () => predictions.map((text) => ({ text })),
        }),
        finish: () => {
          log.finished++;
        },
      };
    },
  };
  return { w: { navigator, HandwritingStroke: Stroke }, log };
}

describe('BrowserHandwritingProvider', () => {
  it('feature-detects the API', () => {
    expect(webHandwriting({})).toBeUndefined();
    expect(webHandwriting({ navigator: {} })).toBeUndefined();
    expect(webHandwriting(fakeApi([]).w)).toBeInstanceOf(BrowserHandwritingProvider);
  });

  it('recognises text from the strokes, with times', async () => {
    const { w, log } = fakeApi(['xin chào', 'xin chao']);
    const p = webHandwriting(w, () => 'vi-VN')!;
    expect(await p.capabilities()).toEqual({ text: true, math: false, languages: ['vi'] });
    const r = await p.recognize({ mode: 'text', lang: 'vi-VN', strokes: STROKES });
    expect(r).toEqual({ text: 'xin chào', alternatives: ['xin chao'] });
    expect(log.languages).toEqual(['vi']);
    // Recorded times kept; the second stroke starts 100 ms after the first ends, 10 ms per point.
    expect(log.strokes).toEqual([
      [
        { x: 10, y: 10, t: 0 },
        { x: 20, y: 12, t: 16 },
      ],
      [{ x: 30, y: 30, t: 116 }],
    ]);
    expect(log.finished).toBe(1);
    // Math is not part of the API.
    await expect(p.recognize({ mode: 'math', lang: 'vi', strokes: STROKES })).rejects.toThrow(
      HandwritingError,
    );
  });

  it('reports a language it does not have and empty readings', async () => {
    const { w } = fakeApi(['  ']);
    const en = webHandwriting(w, () => 'en-US')!;
    expect((await en.capabilities()).text).toBe(false);
    const vi = webHandwriting(w)!;
    await expect(
      vi.recognize({ mode: 'text', lang: 'vi', strokes: STROKES }),
    ).rejects.toMatchObject({
      code: 'empty',
    });
  });
});

/** A fake server: capabilities and readings, recording requests. */
function fakeServer(opts: { caps?: unknown; status?: number; answer?: unknown; code?: string }) {
  const calls: { url: string; init?: Parameters<HandwritingFetch>[1] }[] = [];
  const f: HandwritingFetch = async (url, init) => {
    calls.push({ url, ...(init ? { init } : {}) });
    if (url.includes('/capabilities')) {
      return opts.caps
        ? { ok: true, status: 200, json: async () => opts.caps }
        : { ok: false, status: 404, json: async () => ({ code: 'unavailable' }) };
    }
    const status = opts.status ?? 200;
    return {
      ok: status < 300,
      status,
      json: async () => (status < 300 ? opts.answer : { code: opts.code }),
    };
  };
  return { f, calls };
}

describe('ServerHandwritingProvider', () => {
  it('is unavailable until an administrator configures the server', async () => {
    const { f } = fakeServer({});
    const p = new ServerHandwritingProvider({ baseUrl: 'https://bo.example/', fetch: f });
    expect(await p.capabilities()).toEqual({ text: false, math: false, languages: [] });
    await expect(p.recognize({ mode: 'text', lang: 'vi', strokes: STROKES })).rejects.toMatchObject(
      {
        code: 'unavailable',
      },
    );
  });

  it('posts the strokes and reads text and math', async () => {
    const { f, calls } = fakeServer({
      caps: { modes: ['text', 'math'], languages: ['vi', 'en'], mathFormat: 'latex' },
      answer: { text: '\\frac{a}{b}', alternatives: ['a/b'] },
    });
    const p = new ServerHandwritingProvider({
      baseUrl: 'https://bo.example',
      session: 's1',
      headers: () => ({ 'x-session-token': 't' }),
      fetch: f,
    });
    expect(await p.capabilities()).toEqual({ text: true, math: true, languages: ['vi', 'en'] });
    const r = await p.recognize({ mode: 'math', lang: 'vi-VN', strokes: STROKES });
    expect(r).toEqual({ text: '\\frac{a}{b}', alternatives: ['a/b'], format: 'latex' });
    const post = calls.find((c) => c.init?.method === 'POST')!;
    expect(post.url).toBe('https://bo.example/api/handwriting?mode=math&lang=vi-VN&session=s1');
    expect(post.init?.headers).toEqual({
      'x-session-token': 't',
      'content-type': 'application/json',
    });
    expect(JSON.parse(post.init!.body!)).toEqual({
      strokes: [
        {
          points: [
            [10, 10, 0],
            [20, 12, 16],
          ],
        },
        { points: [[30, 30, 116]] },
      ],
    });
  });

  it('maps server errors to codes', async () => {
    expect(handwritingServerErrorCode(429)).toBe('rateLimited');
    expect(handwritingServerErrorCode(413)).toBe('tooLarge');
    expect(handwritingServerErrorCode(401)).toBe('unauthorized');
    expect(handwritingServerErrorCode(400, 'unsupportedMode')).toBe('unavailable');
    expect(handwritingServerErrorCode(502)).toBe('upstream');
    const { f } = fakeServer({ caps: { modes: ['text'] }, status: 429 });
    const p = new ServerHandwritingProvider({ baseUrl: '', fetch: f });
    await expect(p.recognize({ mode: 'text', lang: 'vi', strokes: STROKES })).rejects.toMatchObject(
      {
        code: 'rateLimited',
      },
    );
    await expect(p.recognize({ mode: 'math', lang: 'vi', strokes: STROKES })).rejects.toMatchObject(
      {
        code: 'unavailable',
      },
    );
  });
});

describe('choosing a provider', () => {
  it('takes, per mode, the first provider offering it', async () => {
    const server = new FakeHandwritingProvider({ text: false, math: true, languages: ['vi'] });
    const browser = new FakeHandwritingProvider({ text: true, math: false, languages: ['en'] });
    server.answers.push({ text: 'x^2', format: 'unicodeMath' });
    browser.answers.push({ text: 'hello' });
    const p = new AutoHandwritingProvider([server, browser]);
    expect(await p.capabilities()).toEqual({ text: true, math: true, languages: ['vi', 'en'] });
    expect((await p.recognize({ mode: 'text', lang: 'en', strokes: STROKES })).text).toBe('hello');
    expect((await p.recognize({ mode: 'math', lang: 'en', strokes: STROKES })).text).toBe('x^2');
    expect(browser.requests).toHaveLength(1);
    expect(server.requests).toHaveLength(1);
  });

  it('builds the platform default', () => {
    expect(primaryLanguage('vi-VN')).toBe('vi');
    expect(defaultHandwriting({ w: {} })).toBeUndefined();
    expect(defaultHandwriting({ w: fakeApi([]).w, mode: 'off' })).toBeUndefined();
    expect(defaultHandwriting({ w: fakeApi([]).w })).toBeInstanceOf(BrowserHandwritingProvider);
    expect(defaultHandwriting({ w: {}, server: { baseUrl: '' } })).toBeInstanceOf(
      ServerHandwritingProvider,
    );
    expect(defaultHandwriting({ w: fakeApi([]).w, server: { baseUrl: '' } })).toBeInstanceOf(
      AutoHandwritingProvider,
    );
    expect(
      defaultHandwriting({ w: fakeApi([]).w, server: { baseUrl: '' }, mode: 'browser' }),
    ).toBeInstanceOf(BrowserHandwritingProvider);
  });
});
