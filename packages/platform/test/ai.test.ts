import { describe, expect, it } from 'vitest';
import {
  AI_OUTLINE_LIMITS,
  AiError,
  FakeAiProvider,
  ServerAiProvider,
  aiComplete,
  aiRequestSize,
  aiServerErrorCode,
  extractFormula,
  outlineFromText,
  outlineText,
  parseAiOutline,
  plainText,
  type AiFetch,
} from '../src/ai';

const code = async (p: Promise<unknown>) => {
  try {
    await p;
  } catch (e) {
    return e instanceof AiError ? e.code : `other:${String(e)}`;
  }
  return 'none';
};

/** A fake BlueOffice server: records the calls, answers what the test queued. */
function fakeServer(
  answers: Record<string, { status?: number; body?: unknown; html?: boolean; fail?: boolean }>,
) {
  const calls: { url: string; method: string; headers: Record<string, string>; body: unknown }[] =
    [];
  const fetch: AiFetch = async (url, init) => {
    calls.push({
      url,
      method: init?.method ?? 'GET',
      headers: init?.headers ?? {},
      body: init?.body ? (JSON.parse(init.body) as unknown) : undefined,
    });
    const key = Object.keys(answers).find((k) => url.includes(k));
    const a = key ? answers[key]! : { status: 404 };
    if (a.fail) throw new TypeError('failed to fetch');
    const status = a.status ?? 200;
    return {
      ok: status >= 200 && status < 300,
      status,
      json: async () => {
        if (a.html) throw new SyntaxError('Unexpected token <');
        return a.body;
      },
    };
  };
  return { fetch, calls, answers };
}

describe('ServerAiProvider', () => {
  it('is off when the server says 404, answers HTML or is unreachable', async () => {
    for (const config of [{ status: 404 }, { html: true }, { fail: true }, { body: {} }]) {
      const s = fakeServer({ '/api/ai/config': config });
      const p = new ServerAiProvider({ baseUrl: 'https://office.test/', fetch: s.fetch });
      expect(await p.info()).toBeNull();
      // Nothing is sent while off.
      expect(await code(aiComplete(p, { task: 'summarize', text: 'Bí mật' }))).toBe('unavailable');
      expect(s.calls.every((c) => c.method === 'GET')).toBe(true);
    }
  });

  it('reads the configuration and posts a request with the caller headers', async () => {
    const s = fakeServer({
      '/api/ai/config': { body: { enabled: true, host: 'llm.internal', maxInputChars: 500 } },
      '/api/ai/complete': { body: { text: 'Bản tóm tắt.' } },
    });
    const p = new ServerAiProvider({
      baseUrl: 'https://office.test/',
      headers: () => ({ Authorization: 'Bearer t' }),
      session: 's 1',
      fetch: s.fetch,
    });
    expect(await p.info()).toEqual({ host: 'llm.internal', maxInputChars: 500 });
    // The configuration is looked up once while BlueAI is on.
    await p.info();
    expect(s.calls.map((c) => c.url)).toEqual(['https://office.test/api/ai/config?session=s%201']);
    const out = await aiComplete(p, {
      task: 'rewrite',
      text: 'Xin chào',
      tone: 'formal',
      language: 'vi',
    });
    expect(out).toBe('Bản tóm tắt.');
    const post = s.calls.at(-1)!;
    expect(post.url).toBe('https://office.test/api/ai/complete');
    expect(post.method).toBe('POST');
    expect(post.headers.Authorization).toBe('Bearer t');
    expect(post.body).toEqual({
      task: 'rewrite',
      text: 'Xin chào',
      instruction: '',
      tone: 'formal',
      language: 'vi',
      session: 's 1',
    });
  });

  it('maps server errors', async () => {
    expect(aiServerErrorCode(404)).toBe('unavailable');
    expect(aiServerErrorCode(401)).toBe('unauthorized');
    expect(aiServerErrorCode(413)).toBe('tooLarge');
    expect(aiServerErrorCode(429)).toBe('rateLimited');
    expect(aiServerErrorCode(504)).toBe('timeout');
    expect(aiServerErrorCode(502)).toBe('upstream');
    expect(aiServerErrorCode(400)).toBe('empty');
    const s = fakeServer({
      '/api/ai/config': { body: { enabled: true, host: 'h', maxInputChars: 100 } },
      '/api/ai/complete': { status: 429, body: { code: 'rateLimited' } },
    });
    const p = new ServerAiProvider({ baseUrl: '', fetch: s.fetch });
    expect(await code(aiComplete(p, { task: 'summarize', text: 'a' }))).toBe('rateLimited');
    s.answers['/api/ai/complete'] = { body: { texts: ['wrong shape'] } };
    expect(await code(aiComplete(p, { task: 'summarize', text: 'a' }))).toBe('upstream');
    s.answers['/api/ai/complete'] = { fail: true };
    expect(await code(aiComplete(p, { task: 'summarize', text: 'a' }))).toBe('network');
    // Turned off on the server meanwhile: the cached "on" is forgotten.
    s.answers['/api/ai/complete'] = { status: 404 };
    s.answers['/api/ai/config'] = { status: 404 };
    expect(await code(aiComplete(p, { task: 'summarize', text: 'a' }))).toBe('unavailable');
    expect(await p.info()).toBeNull();
  });
});

describe('aiComplete', () => {
  it('refuses a request over the size limit before sending it', async () => {
    const fake = new FakeAiProvider({ host: 'ai.test', maxInputChars: 10 });
    const req = { task: 'draft' as const, text: '123456', instruction: '12345' };
    expect(aiRequestSize(req)).toBe(11);
    const err = await aiComplete(fake, req).catch((e: unknown) => e);
    expect(err).toBeInstanceOf(AiError);
    expect((err as AiError).code).toBe('tooLarge');
    expect((err as AiError).limit).toEqual({ max: 10, size: 11 });
    expect(fake.calls).toHaveLength(0);
    expect(await aiComplete(fake, { ...req, text: '12345' })).toBe('Draft: 12345');
  });

  it('sends nothing without a provider, while off or with nothing to send', async () => {
    expect(await code(aiComplete(undefined, { task: 'summarize', text: 'a' }))).toBe('unavailable');
    const fake = new FakeAiProvider();
    expect(await code(aiComplete(fake, { task: 'summarize', text: '  ' }))).toBe('empty');
    fake.off();
    expect(await code(aiComplete(fake, { task: 'summarize', text: 'a' }))).toBe('unavailable');
    expect(fake.calls).toHaveLength(0);
  });

  it('returns plain text and reports failures and cancellation', async () => {
    const fake = new FakeAiProvider();
    fake.answers.summarize = '\u0000  Dòng 1\r\nDòng\u0007 2\u2028 \u001b[31m ';
    expect(await aiComplete(fake, { task: 'summarize', text: 'a' })).toBe('Dòng 1\nDòng 2 [31m');
    fake.answers.summarize = ' \n ';
    expect(await code(aiComplete(fake, { task: 'summarize', text: 'a' }))).toBe('badAnswer');
    fake.failWith = 'timeout';
    expect(await code(aiComplete(fake, { task: 'summarize', text: 'a' }))).toBe('timeout');
    fake.failWith = null;
    fake.gate = new Promise(() => {});
    const ac = new AbortController();
    const pending = code(aiComplete(fake, { task: 'summarize', text: 'a' }, { signal: ac.signal }));
    ac.abort();
    expect(await pending).toBe('cancelled');
  });

  it('keeps markup as text (nothing is interpreted)', () => {
    const html = '<img src=x onerror="alert(1)"><script>steal()</script>';
    expect(plainText(html)).toBe(html);
  });
});

describe('outlineFromText (rule-based, no AI)', () => {
  it('turns unindented lines into titles and the others into bullets', () => {
    const slides = outlineFromText(
      [
        'Kế hoạch quý IV',
        '  Mục tiêu doanh thu',
        '    Miền Bắc',
        '  Nhân sự',
        '',
        '# Rủi ro',
        '- Tỷ giá',
        '  - Biến động USD',
        '* Nguồn cung',
        'Kết luận',
      ].join('\n'),
    );
    expect(slides).toEqual([
      {
        title: 'Kế hoạch quý IV',
        bullets: [
          { text: 'Mục tiêu doanh thu', level: 0 },
          { text: 'Miền Bắc', level: 1 },
          { text: 'Nhân sự', level: 0 },
        ],
      },
      {
        title: 'Rủi ro',
        bullets: [
          { text: 'Tỷ giá', level: 0 },
          { text: 'Biến động USD', level: 1 },
          { text: 'Nguồn cung', level: 0 },
        ],
      },
      { title: 'Kết luận', bullets: [] },
    ]);
  });

  it('handles tabs, numbered items, bullets before a title and empty input', () => {
    expect(outlineFromText('\tÝ lẻ\r\nTiêu đề\r\n\t1. Một\r\n\t\t2) Hai')).toEqual([
      { title: '', bullets: [{ text: 'Ý lẻ', level: 0 }] },
      {
        title: 'Tiêu đề',
        bullets: [
          { text: 'Một', level: 0 },
          { text: 'Hai', level: 1 },
        ],
      },
    ]);
    expect(outlineFromText(' \n\n')).toEqual([]);
    // Levels stop at 8 and counts at the limits.
    const deep = outlineFromText(`T\n  a\n${' '.repeat(40)}b`);
    expect(deep[0]!.bullets[1]!.level).toBe(8);
    const many = outlineFromText(Array.from({ length: 80 }, (_, i) => `Trang ${i}`).join('\n'));
    expect(many).toHaveLength(AI_OUTLINE_LIMITS.slides);
  });

  it('prints an outline back as indented text', () => {
    const slides = outlineFromText('A\n  b\n    c\nD');
    expect(outlineText(slides)).toBe('A\n  - b\n    - c\nD');
  });
});

describe('parseAiOutline (schema check of a provider answer)', () => {
  const bad = (answer: string) => {
    try {
      parseAiOutline(answer);
    } catch (e) {
      return e instanceof AiError ? e.code : 'other';
    }
    return 'accepted';
  };

  it('accepts the schema, also inside a code fence or with text around', () => {
    const json = '{"slides":[{"title":" Mở đầu ","bullets":["Một\\nhai",""]},{"title":"Kết"}]}';
    const want = [
      { title: 'Mở đầu', bullets: [{ text: 'Một hai', level: 0 }] },
      { title: 'Kết', bullets: [] },
    ];
    expect(parseAiOutline(json)).toEqual(want);
    expect(parseAiOutline('```json\n' + json + '\n```')).toEqual(want);
    expect(parseAiOutline(`Đây là dàn ý:\n${json}\nChúc bạn thành công.`)).toEqual(want);
  });

  it('rejects everything else', () => {
    expect(bad('Tôi không thể giúp.')).toBe('badAnswer');
    expect(bad('{"slides": [')).toBe('badAnswer');
    expect(bad('{"slides":[]}')).toBe('badAnswer');
    expect(bad('{"slides":"x"}')).toBe('badAnswer');
    expect(bad('{"slides":[{"title":1,"bullets":[]}]}')).toBe('badAnswer');
    expect(bad('{"slides":[{"title":"a","bullets":"b"}]}')).toBe('badAnswer');
    expect(bad('{"slides":[{"title":"a","bullets":[{"text":"b"}]}]}')).toBe('badAnswer');
    expect(bad('{"slides":["a"]}')).toBe('badAnswer');
    // No extra keys: an answer cannot smuggle commands or other fields.
    expect(bad('{"slides":[{"title":"a","bullets":[]}],"run":"deleteAll"}')).toBe('badAnswer');
    expect(bad('{"slides":[{"title":"a","bullets":[],"action":"x"}]}')).toBe('badAnswer');
    const tooMany = JSON.stringify({
      slides: Array.from({ length: AI_OUTLINE_LIMITS.slides + 1 }, () => ({ title: 'a' })),
    });
    expect(bad(tooMany)).toBe('badAnswer');
    const longTitle = JSON.stringify({
      slides: [{ title: 'a'.repeat(AI_OUTLINE_LIMITS.titleChars + 1) }],
    });
    expect(bad(longTitle)).toBe('badAnswer');
    const manyBullets = JSON.stringify({
      slides: [{ title: 'a', bullets: Array.from({ length: 21 }, () => 'b') }],
    });
    expect(bad(manyBullets)).toBe('badAnswer');
  });

  it('accepts what the fake provider answers', async () => {
    const fake = new FakeAiProvider();
    const answer = await aiComplete(fake, {
      task: 'outline',
      instruction: 'Mở đầu: chào, mục tiêu; Kết luận',
    });
    expect(parseAiOutline(answer)).toEqual([
      {
        title: 'Mở đầu',
        bullets: [
          { text: 'chào', level: 0 },
          { text: 'mục tiêu', level: 0 },
        ],
      },
      { title: 'Kết luận', bullets: [] },
    ]);
  });
});

describe('extractFormula', () => {
  it('takes the first formula line', () => {
    expect(extractFormula('=SUM(A1:A3)')).toBe('=SUM(A1:A3)');
    expect(extractFormula('```\n=SUMIF(A:A,"Bắc",B:B)\n```')).toBe('=SUMIF(A:A,"Bắc",B:B)');
    expect(extractFormula('Công thức:\n`=A1+B1`\n=A2')).toBe('=A1+B1');
  });

  it('rejects an answer without a formula', () => {
    for (const answer of ['Không có công thức', '=', 'SUM(A1:A3)', `=${'1+'.repeat(5000)}1`]) {
      expect(() => extractFormula(answer)).toThrow(AiError);
    }
  });
});

describe('FakeAiProvider', () => {
  it('answers deterministically per task', async () => {
    const fake = new FakeAiProvider();
    expect(await fake.info()).toEqual({ host: 'ai.test', maxInputChars: 2000 });
    expect(await fake.complete({ task: 'rewrite', text: 'abc', tone: 'friendly' })).toBe(
      '[friendly] abc',
    );
    expect(await fake.complete({ task: 'summarize', text: 'abc' })).toBe('Summary (3 chars): abc');
    expect(await fake.complete({ task: 'formula', instruction: 'tổng' })).toBe('=SUM(A1:A3)');
    expect(await fake.complete({ task: 'analyze', text: 'a\tb\n1\t2\n' })).toBe('Rows: 2');
    expect(fake.calls.map((c) => c.task)).toEqual(['rewrite', 'summarize', 'formula', 'analyze']);
  });
});
