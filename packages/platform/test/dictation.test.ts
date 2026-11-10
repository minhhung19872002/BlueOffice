import { describe, expect, it } from 'vitest';
import {
  AutoDictationProvider,
  DICTATION_SAMPLE_RATE,
  DictationError,
  FakeDictationProvider,
  LocalDictationProvider,
  PhraseSegmenter,
  ServerDictationProvider,
  WebDictationProvider,
  defaultDictation,
  dictationError,
  dictationServerErrorCode,
  encodeWav,
  recognitionErrorCode,
  resample,
  webDictation,
  type AudioCapture,
  type DictationEvent,
  type DictationFetch,
  type RecognitionEventLike,
  type RecognitionLike,
} from '../src/dictation';

const tick = () => new Promise((r) => setTimeout(r, 0));
const flush = async (n = 5) => {
  for (let i = 0; i < n; i++) await tick();
};

/** A scripted `SpeechRecognition`. */
class FakeRecognition implements RecognitionLike {
  static all: FakeRecognition[] = [];
  lang = '';
  continuous = false;
  interimResults = false;
  maxAlternatives = 0;
  onstart: (() => void) | null = null;
  onresult: ((e: RecognitionEventLike) => void) | null = null;
  onerror: ((e: { error: string; message?: string }) => void) | null = null;
  onend: (() => void) | null = null;
  started = false;
  stopped = false;
  aborted = false;
  results: { isFinal: boolean; length: number; 0: { transcript: string; confidence: number } }[] =
    [];

  constructor() {
    FakeRecognition.all.push(this);
  }

  start() {
    this.started = true;
    this.onstart?.();
  }

  stop() {
    this.stopped = true;
    this.onend?.();
  }

  abort() {
    this.aborted = true;
  }

  /** Reports result `i` (replacing it), the way Chromium re-sends the interim tail. */
  say(i: number, transcript: string, isFinal: boolean, confidence = 0.9) {
    this.results[i] = { isFinal, length: 1, 0: { transcript, confidence } };
    this.onresult?.({ resultIndex: i, results: this.results });
  }
}

function web(now = () => 0) {
  FakeRecognition.all = [];
  const p = new WebDictationProvider(() => new FakeRecognition(), now);
  const events: DictationEvent[] = [];
  const s = p.start('vi-VN', {}, (e) => events.push(e));
  return { p, s, events, rec: () => FakeRecognition.all.at(-1)! };
}

describe('WebDictationProvider', () => {
  it('reports interim then final results with the language and continuous mode', () => {
    const { s, events, rec } = web();
    expect(rec().lang).toBe('vi-VN');
    expect(rec().continuous).toBe(true);
    expect(rec().interimResults).toBe(true);
    rec().say(0, 'xin', false);
    rec().say(0, 'xin chào', false);
    rec().say(0, 'xin chào chấm', true, 0.8);
    rec().say(1, 'tôi', false);
    s.stop();
    expect(events).toEqual([
      { type: 'start' },
      { type: 'result', text: 'xin', final: false },
      { type: 'result', text: 'xin chào', final: false },
      { type: 'result', text: 'xin chào chấm', final: true, confidence: 0.8 },
      { type: 'result', text: 'tôi', final: false },
      { type: 'end' },
    ]);
  });

  it('listens again when the recogniser stops on its own, gives up when it keeps stopping', () => {
    let t = 0;
    const { events, rec } = web(() => t);
    t = 5000;
    rec().onend?.();
    expect(FakeRecognition.all).toHaveLength(2);
    expect(rec().started).toBe(true);
    // Ends at once four times in a row: the session ends.
    for (let i = 0; i < 4; i++) rec().onend?.();
    expect(events.filter((e) => e.type === 'start')).toHaveLength(1);
    expect(events.at(-1)).toEqual({ type: 'end' });
  });

  it('maps recogniser errors; no-speech is not fatal', () => {
    const { events, rec } = web();
    rec().onerror?.({ error: 'no-speech' });
    expect(events).toEqual([{ type: 'start' }]);
    rec().onerror?.({ error: 'not-allowed' });
    expect(events.at(-1)).toEqual({ type: 'error', code: 'noPermission' });
    expect(rec().aborted).toBe(true);
    // Nothing after an error.
    rec().say(0, 'x', true);
    expect(events).toHaveLength(2);
    expect(
      [
        'audio-capture',
        'network',
        'service-not-allowed',
        'language-not-supported',
        'bad-grammar',
      ].map(recognitionErrorCode),
    ).toEqual(['noDevice', 'network', 'unavailable', 'unsupportedLanguage', 'upstream']);
  });

  it('a recogniser that throws at start is unavailable (desktop webview without one)', () => {
    const p = new WebDictationProvider(() => {
      const r = new FakeRecognition();
      r.start = () => {
        throw new Error('not supported');
      };
      return r;
    });
    const events: DictationEvent[] = [];
    p.start('vi-VN', {}, (e) => events.push(e));
    expect(events).toEqual([{ type: 'error', code: 'unavailable', message: 'not supported' }]);
  });

  it('is feature-detected (prefixed or not)', () => {
    expect(webDictation({})).toBeUndefined();
    expect(webDictation({ webkitSpeechRecognition: FakeRecognition })).toBeInstanceOf(
      WebDictationProvider,
    );
    expect(defaultDictation({ w: {}, server: { baseUrl: '' } })).toBeUndefined();
    expect(defaultDictation({ w: { SpeechRecognition: FakeRecognition }, mode: 'off' })).toBe(
      undefined,
    );
  });

  it('abort stops without further events', () => {
    const { s, events, rec } = web();
    s.abort();
    rec().say(0, 'x', true);
    rec().onend?.();
    expect(events).toEqual([{ type: 'start' }]);
  });
});

describe('dictationError', () => {
  it('maps microphone failures', () => {
    expect(dictationError(new DOMException('x', 'NotAllowedError')).code).toBe('noPermission');
    expect(dictationError(new DOMException('x', 'NotFoundError')).code).toBe('noDevice');
    expect(dictationError(new TypeError('fetch failed')).code).toBe('network');
    expect(dictationError(new DictationError('tooLarge')).code).toBe('tooLarge');
    expect([404, 401, 413, 429, 400, 500].map((s) => dictationServerErrorCode(s))).toEqual([
      'unavailable',
      'unauthorized',
      'tooLarge',
      'rateLimited',
      'unsupportedLanguage',
      'upstream',
    ]);
  });
});

const RATE = 8000;
const tone = (ms: number, amp = 0.3) =>
  Float32Array.from({ length: (RATE * ms) / 1000 }, (_, i) => amp * Math.sin(i / 3));
const silence = (ms: number) => new Float32Array((RATE * ms) / 1000);
const cat = (...parts: Float32Array[]) => {
  const out = new Float32Array(parts.reduce((n, p) => n + p.length, 0));
  let at = 0;
  for (const p of parts) {
    out.set(p, at);
    at += p.length;
  }
  return out;
};

describe('PhraseSegmenter', () => {
  it('cuts phrases at pauses, keeps a little audio before speech, drops clicks', () => {
    const seg = new PhraseSegmenter(RATE, { silenceMs: 400, preRollMs: 100, minSpeechMs: 200 });
    const phrases = seg.push(
      cat(silence(500), tone(600), silence(500), tone(60), silence(500), tone(400), silence(100)),
    );
    // The first phrase: 100 ms pre-roll + 600 ms speech + 400 ms of the pause.
    expect(phrases).toHaveLength(1);
    expect(phrases[0]!.length).toBe((RATE * 1100) / 1000);
    // The click (60 ms) was not a phrase; the last one is still open until flushed.
    const last = seg.flush();
    expect(last).not.toBeNull();
    expect(last!.length).toBeGreaterThanOrEqual((RATE * 400) / 1000);
    expect(seg.flush()).toBeNull();
  });

  it('cuts a long phrase at maxMs', () => {
    const seg = new PhraseSegmenter(RATE, { maxMs: 1000 });
    expect(seg.push(tone(2500))).toHaveLength(2);
  });
});

describe('WAV encoding', () => {
  it('writes a 16-bit mono RIFF/WAVE file', () => {
    const wav = encodeWav(Float32Array.from([0, 1, -1, 0.5]), 16000);
    const v = new DataView(wav.buffer);
    const ascii = (a: number, n: number) => String.fromCharCode(...wav.slice(a, a + n));
    expect(ascii(0, 4)).toBe('RIFF');
    expect(ascii(8, 8)).toBe('WAVEfmt ');
    expect(v.getUint32(4, true)).toBe(36 + 8);
    expect(v.getUint16(22, true)).toBe(1);
    expect(v.getUint32(24, true)).toBe(16000);
    expect(v.getUint16(34, true)).toBe(16);
    expect(ascii(36, 4)).toBe('data');
    expect([0, 1, 2, 3].map((i) => v.getInt16(44 + i * 2, true))).toEqual([
      0, 32767, -32768, 16384,
    ]);
  });

  it('resamples down by averaging and up linearly', () => {
    expect([...resample(Float32Array.from([1, 3, 5, 7]), 4, 2)]).toEqual([2, 6]);
    expect([...resample(Float32Array.from([0, 2]), 1, 2)]).toEqual([0, 1, 2, 2]);
  });
});

/** A microphone that plays `audio` once opened (or fails). */
function mic(audio: Float32Array, fail?: unknown): AudioCapture & { closed: boolean } {
  const m = {
    closed: false,
    async open(onFrames: (pcm: Float32Array) => void) {
      if (fail) throw fail;
      setTimeout(() => {
        for (let i = 0; i < audio.length; i += 1024) onFrames(audio.slice(i, i + 1024));
      }, 0);
      return {
        sampleRate: RATE,
        close: () => {
          m.closed = true;
        },
      };
    },
  };
  return m;
}

describe('ServerDictationProvider', () => {
  const fakeFetch = (texts: string[], status = 200) => {
    const calls: { url: string; headers?: Record<string, string>; body?: Uint8Array }[] = [];
    const f: DictationFetch = async (url, init) => {
      calls.push({ url, headers: init?.headers, body: init?.body });
      if (url.includes('/languages'))
        return { ok: true, status: 200, json: async () => ({ languages: ['vi', 'en'] }) };
      if (status !== 200) return { ok: false, status, json: async () => ({ code: 'unavailable' }) };
      return {
        ok: true,
        status: 200,
        json: async () => ({ text: texts.shift() ?? '', confidence: 0.7 }),
      };
    };
    return { f, calls };
  };

  it('posts each phrase as 16 kHz WAV and reports the text as a final result', async () => {
    const { f, calls } = fakeFetch(['xin chào chấm', 'tôi là An']);
    const m = mic(cat(tone(500), silence(800), tone(500), silence(200)));
    const p = new ServerDictationProvider({
      baseUrl: 'https://office.example/',
      session: 's1',
      headers: () => ({ 'x-session-token': 't' }),
      fetch: f,
      capture: m,
    });
    expect(await p.available()).toBe(true);
    expect(await p.languages()).toEqual(['vi', 'en']);
    const events: DictationEvent[] = [];
    const s = p.start('vi-VN', {}, (e) => events.push(e));
    await flush();
    s.stop();
    await flush();
    expect(events).toEqual([
      { type: 'start' },
      { type: 'result', text: 'xin chào chấm', final: true, confidence: 0.7 },
      { type: 'result', text: 'tôi là An', final: true, confidence: 0.7 },
      { type: 'end' },
    ]);
    expect(m.closed).toBe(true);
    const posts = calls.filter((c) => !c.url.includes('/languages'));
    expect(posts.map((c) => c.url)).toEqual([
      'https://office.example/api/dictation?lang=vi-VN&session=s1',
      'https://office.example/api/dictation?lang=vi-VN&session=s1',
    ]);
    expect(posts[0]!.headers).toEqual({ 'x-session-token': 't', 'content-type': 'audio/wav' });
    const wav = posts[0]!.body!;
    expect(new DataView(wav.buffer).getUint32(24, true)).toBe(DICTATION_SAMPLE_RATE);
  });

  it('is unavailable when the server has no dictation; errors end the session', async () => {
    const off: DictationFetch = async () => ({ ok: false, status: 404, json: async () => ({}) });
    const p = new ServerDictationProvider({ baseUrl: '', fetch: off, capture: mic(silence(10)) });
    expect(await p.available()).toBe(false);
    const { f } = fakeFetch([], 429);
    const q = new ServerDictationProvider({
      baseUrl: '',
      fetch: f,
      capture: mic(cat(tone(500), silence(900))),
    });
    const events: DictationEvent[] = [];
    q.start('vi-VN', {}, (e) => events.push(e));
    await flush();
    expect(events.at(-1)).toMatchObject({ type: 'error', code: 'rateLimited' });
  });

  it('reports a refused microphone', async () => {
    const { f } = fakeFetch([]);
    const p = new ServerDictationProvider({
      baseUrl: '',
      fetch: f,
      capture: mic(silence(1), new DOMException('denied', 'NotAllowedError')),
    });
    const events: DictationEvent[] = [];
    p.start('vi-VN', {}, (e) => events.push(e));
    await flush();
    expect(events).toEqual([{ type: 'error', code: 'noPermission', message: 'denied' }]);
  });
});

describe('LocalDictationProvider', () => {
  it('recognises phrases on the device model (loaded once) and refuses other languages', async () => {
    let loads = 0;
    const p = new LocalDictationProvider(
      {
        languages: ['vi'],
        load: async () => {
          loads++;
        },
        transcribe: async (pcm) => ({ text: `${pcm.length > 0 ? 'xin chào' : ''}` }),
      },
      mic(cat(tone(400), silence(800), tone(400), silence(800))),
    );
    const events: DictationEvent[] = [];
    const s = p.start('vi-VN', {}, (e) => events.push(e));
    await flush();
    s.stop();
    await flush();
    expect(events.filter((e) => e.type === 'result')).toHaveLength(2);
    expect(loads).toBe(1);
    const en: DictationEvent[] = [];
    p.start('en-US', {}, (e) => en.push(e));
    await flush();
    expect(en.at(-1)).toMatchObject({ type: 'error', code: 'unsupportedLanguage' });
  });
});

describe('AutoDictationProvider', () => {
  it('uses the first available provider', async () => {
    const off = new FakeDictationProvider([], false);
    const on = new FakeDictationProvider(['vi-VN']);
    const p = new AutoDictationProvider([off, on]);
    expect(await p.available()).toBe(true);
    expect(await p.languages()).toEqual(['vi-VN']);
    const events: DictationEvent[] = [];
    p.start('vi-VN', {}, (e) => events.push(e));
    await flush();
    expect(off.sessions).toHaveLength(0);
    expect(on.sessions).toHaveLength(1);
    on.final('xin chào');
    expect(events.at(-1)).toEqual({ type: 'result', text: 'xin chào', final: true });
    const none = new AutoDictationProvider([off]);
    const e2: DictationEvent[] = [];
    none.start('vi-VN', {}, (e) => e2.push(e));
    await flush();
    expect(e2).toEqual([{ type: 'error', code: 'unavailable' }]);
  });
});

describe('FakeDictationProvider', () => {
  it('reports what the test says, the pending text on stop, then end', () => {
    const p = new FakeDictationProvider();
    const events: DictationEvent[] = [];
    const s = p.start('en-US', { interim: true }, (e) => events.push(e));
    p.interim('hello');
    p.final('hello world', 0.5);
    p.pendingOnStop = 'bye';
    s.stop();
    expect(p.final('late')).toBe(false);
    expect(events).toEqual([
      { type: 'start' },
      { type: 'result', text: 'hello', final: false },
      { type: 'result', text: 'hello world', final: true, confidence: 0.5 },
      { type: 'result', text: 'bye', final: true },
      { type: 'end' },
    ]);
  });
});
