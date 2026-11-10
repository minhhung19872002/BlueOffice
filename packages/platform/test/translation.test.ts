import { describe, expect, it } from 'vitest';
import {
  FakeTranslationProvider,
  LocalTranslationProvider,
  ServerTranslationProvider,
  TranslationError,
  batches,
  defaultTarget,
  serverErrorCode,
  translateTexts,
} from '../src/translation';

const code = async (p: Promise<unknown>) => {
  try {
    await p;
    return null;
  } catch (e) {
    return e instanceof TranslationError ? e.code : String(e);
  }
};

describe('translateTexts', () => {
  it('detects the source per text, keeps blanks, target-language text and white space', async () => {
    const fake = new FakeTranslationProvider();
    const out = await translateTexts(
      fake,
      ['Xin chào các bạn ', '', '  ', 'The quick brown fox', '\tHôm nay trời đẹp', '123'],
      'auto',
      'en',
    );
    expect(out).toEqual([
      '[en] Xin chào các bạn ',
      '',
      '  ',
      'The quick brown fox',
      '\t[en] Hôm nay trời đẹp',
      '123',
    ]);
    // Only the Vietnamese texts were sent, trimmed, with the detected source.
    expect(fake.calls).toEqual([
      { texts: ['Xin chào các bạn', 'Hôm nay trời đẹp'], from: 'vi', to: 'en' },
    ]);
  });

  it('uses the whole set when a text alone is ambiguous', async () => {
    const fake = new FakeTranslationProvider();
    // "Hà Nội và" is mixed (1 of 2 words … ), "Hello all" has no Vietnamese letter.
    await translateTexts(
      fake,
      ['Tôi ở Hà Nội city center today', 'Hello all', 'Xin chào các bạn, hôm nay'],
      'auto',
      'en',
    );
    expect(fake.calls).toEqual([
      {
        texts: ['Tôi ở Hà Nội city center today', 'Xin chào các bạn, hôm nay'],
        from: 'vi',
        to: 'en',
      },
      { texts: ['Hello all'], from: 'auto', to: 'en' },
    ]);
  });

  it('batches within the provider limits and reports progress', async () => {
    const fake = new FakeTranslationProvider(['vi', 'en'], {}, { texts: 2, chars: 1000 });
    const progress: [number, number][] = [];
    const texts = ['một hai', 'ba bốn', 'năm sáu', 'bảy tám', 'chín mười'];
    const out = await translateTexts(fake, texts, 'vi', 'en', {
      onProgress: (d, t) => progress.push([d, t]),
    });
    expect(out).toHaveLength(5);
    expect(fake.calls.map((c) => c.texts.length)).toEqual([2, 2, 1]);
    expect(progress).toEqual([
      [0, 5],
      [2, 5],
      [4, 5],
      [5, 5],
    ]);
  });

  it('splits batches by characters; a long text goes alone', () => {
    expect(batches(['aaaa', 'bb', 'cccccccc', 'd'], { texts: 10, chars: 6 })).toEqual([
      [0, 1],
      [2],
      [3],
    ]);
  });

  it('cancels before and during a batch', async () => {
    const fake = new FakeTranslationProvider();
    const ac = new AbortController();
    ac.abort();
    expect(
      await code(translateTexts(fake, ['Xin chào bạn'], 'vi', 'en', { signal: ac.signal })),
    ).toBe('cancelled');
    const ac2 = new AbortController();
    fake.gate = new Promise(() => {});
    const p = translateTexts(fake, ['Xin chào bạn'], 'vi', 'en', { signal: ac2.signal });
    ac2.abort();
    expect(await code(p)).toBe('cancelled');
  });

  it('reports errors with codes', async () => {
    const fake = new FakeTranslationProvider();
    fake.failWith = 'rateLimited';
    expect(await code(translateTexts(fake, ['Xin chào bạn'], 'vi', 'en'))).toBe('rateLimited');
    expect(await code(translateTexts(fake, ['x'], 'en', 'en'))).toBe('sameLanguage');
    // Everything already in the target language.
    expect(
      await code(translateTexts(new FakeTranslationProvider(), ['Xin chào bạn'], 'auto', 'vi')),
    ).toBe('sameLanguage');
    expect(
      await code(
        translateTexts(new FakeTranslationProvider(['vi', 'en']), ['hello there'], 'en', 'fr'),
      ),
    ).toBe('unsupportedLanguage');
  });

  it('default target is the other of vi / en', () => {
    expect(defaultTarget('vi-VN')).toBe('en');
    expect(defaultTarget('en-US')).toBe('vi');
  });
});

describe('ServerTranslationProvider', () => {
  const json = (status: number, body: unknown) =>
    new Response(JSON.stringify(body), { status, headers: { 'content-type': 'application/json' } });

  it('posts texts with the session and headers', async () => {
    const seen: { url: string; init?: RequestInit }[] = [];
    const p = new ServerTranslationProvider({
      baseUrl: 'https://office.example/',
      session: 's1',
      headers: () => ({ 'x-session-token': 'tok' }),
      fetch: async (url, init) => {
        seen.push({ url, init });
        if (url.includes('/languages')) return json(200, { languages: ['vi', 'en', 3] });
        const body = JSON.parse(String(init?.body)) as { texts: string[] };
        return json(200, { texts: body.texts.map((t) => t.toUpperCase()) });
      },
    });
    expect(await p.languages()).toEqual([{ code: 'vi' }, { code: 'en' }]);
    expect(await p.translate(['a', 'b'], 'vi', 'en')).toEqual(['A', 'B']);
    expect(seen[0]?.url).toBe('https://office.example/api/translate/languages?session=s1');
    expect(seen[1]?.url).toBe('https://office.example/api/translate');
    expect(JSON.parse(String(seen[1]?.init?.body))).toEqual({
      texts: ['a', 'b'],
      from: 'vi',
      to: 'en',
      session: 's1',
    });
    expect((seen[1]?.init?.headers as Record<string, string>)['x-session-token']).toBe('tok');
  });

  it('maps HTTP errors to codes and checks the answer', async () => {
    const answer = (status: number, body: unknown = {}) =>
      new ServerTranslationProvider({ baseUrl: '', fetch: async () => json(status, body) });
    expect(await code(answer(404).translate(['a'], 'vi', 'en'))).toBe('unavailable');
    expect(await code(answer(401).translate(['a'], 'vi', 'en'))).toBe('unauthorized');
    expect(await code(answer(413).translate(['a'], 'vi', 'en'))).toBe('tooLarge');
    expect(await code(answer(429).translate(['a'], 'vi', 'en'))).toBe('rateLimited');
    expect(await code(answer(502).translate(['a'], 'vi', 'en'))).toBe('upstream');
    expect(
      await code(answer(400, { code: 'unsupportedLanguage' }).translate(['a'], 'vi', 'xx')),
    ).toBe('unsupportedLanguage');
    expect(await code(answer(200, { texts: ['a', 'b'] }).translate(['a'], 'vi', 'en'))).toBe(
      'upstream',
    );
    expect(await answer(404).languages()).toEqual([]);
    const offline = new ServerTranslationProvider({
      baseUrl: '',
      fetch: async () => {
        throw new TypeError('Failed to fetch');
      },
    });
    expect(await code(offline.translate(['a'], 'vi', 'en'))).toBe('network');
    expect(serverErrorCode(501)).toBe('unavailable');
  });

  it('passes the abort signal to fetch', async () => {
    const ac = new AbortController();
    const p = new ServerTranslationProvider({
      baseUrl: '',
      fetch: (_url, init) =>
        new Promise((_, reject) => {
          init?.signal?.addEventListener('abort', () =>
            reject(new DOMException('aborted', 'AbortError')),
          );
        }),
    });
    const r = p.translate(['a'], 'vi', 'en', { signal: ac.signal });
    ac.abort();
    expect(await code(r)).toBe('cancelled');
  });
});

describe('LocalTranslationProvider', () => {
  it('loads the model once and refuses other languages', async () => {
    let loads = 0;
    const p = new LocalTranslationProvider({
      languages: ['vi', 'en'],
      load: async () => {
        loads++;
      },
      translate: async (texts) => texts.map((t) => `<${t}>`),
    });
    expect(await p.languages()).toEqual([{ code: 'vi' }, { code: 'en' }]);
    expect(await p.translate(['a'], 'vi', 'en')).toEqual(['<a>']);
    expect(await p.translate(['b'], 'en', 'vi')).toEqual(['<b>']);
    expect(loads).toBe(1);
    expect(await code(p.translate(['a'], 'vi', 'fr'))).toBe('unsupportedLanguage');
  });
});
