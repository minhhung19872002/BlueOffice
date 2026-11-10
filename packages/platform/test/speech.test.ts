import { describe, expect, it } from 'vitest';
import {
  FakeSpeechProvider,
  WebSpeechProvider,
  byteToUtf16,
  chunkText,
  detectLanguage,
  pickVoice,
  speechLanguage,
  splitSentences,
  utf16ToByte,
  webSpeech,
  wordAt,
  wordStart,
  type SpeechEvent,
  type SpeechVoice,
  type SynthesisLike,
  type UtteranceLike,
  type VoiceLike,
} from '../src';

describe('UTF-16 ↔ UTF-8 offsets', () => {
  it('counts Vietnamese letters as 2 or 3 bytes', () => {
    // "Việt": V(1) i(1) ệ(U+1EC7, 3 bytes) t(1) — 4 UTF-16 units, 6 bytes.
    const t = 'Việt Nam';
    expect(utf16ToByte(t, 4)).toBe(6);
    expect(byteToUtf16(t, 6)).toBe(4);
    // "đ" (U+0111) is 2 bytes.
    expect(utf16ToByte('đi', 1)).toBe(2);
    expect(byteToUtf16('đi', 2)).toBe(1);
  });

  it('keeps decomposed diacritics as separate code points', () => {
    // "e" + U+0302 (combining circumflex, 2 bytes) + U+0301 (combining acute, 2 bytes).
    const t = 'ế x';
    expect(utf16ToByte(t, 3)).toBe(5);
    expect(byteToUtf16(t, 5)).toBe(3);
  });

  it('never splits a surrogate pair', () => {
    // 😀 U+1F600: 2 UTF-16 units, 4 bytes.
    const t = 'a😀b';
    expect(utf16ToByte(t, 3)).toBe(5);
    expect(utf16ToByte(t, 2)).toBe(1); // inside the pair → before it
    expect(byteToUtf16(t, 5)).toBe(3);
    expect(byteToUtf16(t, 3)).toBe(1); // inside the 4 bytes → before it
    expect(byteToUtf16(t, 99)).toBe(t.length);
  });
});

describe('sentences and chunks', () => {
  it('splits at terminal punctuation and line breaks', () => {
    const LS = String.fromCharCode(0x2028); // line break (w:br) in the index text
    const t = `Xin chào. Bạn khỏe không? Tôi khỏe!${LS}Cảm ơn`;
    const s = splitSentences(t).map((r) => t.slice(r.start, r.end));
    expect(s).toEqual(['Xin chào. ', 'Bạn khỏe không? ', `Tôi khỏe!${LS}`, 'Cảm ơn']);
  });

  it('joins short sentences up to the limit', () => {
    const t = 'One. Two. Three.';
    expect(chunkText(t, 100)).toEqual([{ start: 0, end: t.length }]);
    const r = chunkText(t, 10).map((c) => t.slice(c.start, c.end));
    expect(r).toEqual(['One. Two. ', 'Three.']);
  });

  it('cuts a long sentence at white space', () => {
    const t = 'aaaa bbbb cccc dddd';
    const r = chunkText(t, 10).map((c) => t.slice(c.start, c.end));
    expect(r).toEqual(['aaaa bbbb ', 'cccc dddd']);
    // Chunks cover the text without gaps.
    expect(r.join('')).toBe(t);
  });

  it('cuts a long word without breaking an emoji', () => {
    const t = 'abc😀def';
    // A hard cut at 4 would fall between the two halves of 😀 (units 3 and 4).
    const r = chunkText(t, 4);
    expect(r[0]).toEqual({ start: 0, end: 3 });
    expect(r.map((c) => t.slice(c.start, c.end)).join('')).toBe(t);
  });

  it('blank text has no chunk', () => {
    expect(chunkText('   ')).toEqual([]);
  });

  it('finds words', () => {
    expect(wordAt('ab  cd', 2)).toEqual({ start: 4, end: 6 });
    expect(wordStart('hello world', 8)).toBe(6);
  });
});

describe('language and voices', () => {
  const voices: SpeechVoice[] = [
    { id: 'en1', name: 'English A', lang: 'en-GB', local: true, default: false },
    { id: 'en2', name: 'English B', lang: 'en-US', local: false, default: true },
    { id: 'vi1', name: 'Tiếng Việt', lang: 'vi_VN', local: true, default: false },
  ];

  it('detects Vietnamese and English text', () => {
    expect(detectLanguage('Hôm nay trời đẹp quá')).toBe('vi');
    expect(detectLanguage('The weather is nice today')).toBe('en');
    expect(detectLanguage('12 34')).toBeNull();
    // Too short to tell Vietnamese without diacritics from English.
    expect(detectLanguage('Hai')).toBeNull();
    expect(detectLanguage('Ba Vi')).toBeNull();
    expect(detectLanguage('Привет мир')).toBeNull();
  });

  it('prefers what the text reads as over its tag', () => {
    expect(speechLanguage('vi', 'en-US', 'en-US')).toBe('vi-VN');
    expect(speechLanguage(null, 'en-GB', 'vi-VN')).toBe('en-GB');
    expect(speechLanguage(null, null, 'vi-VN')).toBe('vi-VN');
  });

  it('picks a voice of the language', () => {
    expect(pickVoice(voices, 'vi-VN')?.id).toBe('vi1');
    expect(pickVoice(voices, 'en-US')?.id).toBe('en2');
    // en-AU: no exact voice; the default English voice first.
    expect(pickVoice(voices, 'en-AU')?.id).toBe('en2');
    // A preferred voice of the same language wins.
    expect(pickVoice(voices, 'en-US', 'en1')?.id).toBe('en1');
    // A preferred voice of another language does not.
    expect(pickVoice(voices, 'vi-VN', 'en1')?.id).toBe('vi1');
    expect(pickVoice(voices.slice(0, 2), 'vi-VN')).toBeNull();
  });
});

class FakeUtteranceImpl implements UtteranceLike {
  lang = '';
  voice: unknown = null;
  rate = 1;
  pitch = 1;
  volume = 1;
  onstart: UtteranceLike['onstart'] = null;
  onend: UtteranceLike['onend'] = null;
  onerror: UtteranceLike['onerror'] = null;
  onboundary: UtteranceLike['onboundary'] = null;
  onpause: UtteranceLike['onpause'] = null;
  onresume: UtteranceLike['onresume'] = null;
  constructor(public text: string) {}
}

function synth(voices: VoiceLike[]) {
  const queue: FakeUtteranceImpl[] = [];
  const s: SynthesisLike & { queue: FakeUtteranceImpl[]; cancelled: number } = {
    queue,
    cancelled: 0,
    speak: (u) => queue.push(u as FakeUtteranceImpl),
    cancel: () => {
      s.cancelled++;
    },
    pause: () => {},
    resume: () => {},
    getVoices: () => voices,
  };
  return s;
}

describe('WebSpeechProvider', () => {
  const vv: VoiceLike[] = [
    { voiceURI: 'urn:vi', name: 'An', lang: 'vi-VN', localService: true, default: false },
  ];

  it('lists voices and passes options to the utterance', async () => {
    const s = synth(vv);
    const p = new WebSpeechProvider(s, (t) => new FakeUtteranceImpl(t));
    expect(await p.voices()).toEqual([
      { id: 'urn:vi', name: 'An', lang: 'vi-VN', local: true, default: false },
    ]);
    p.speak('xin chào', { voice: 'urn:vi', rate: 1.5, pitch: 0.8, volume: 0.5 }, () => {});
    const u = s.queue[0]!;
    expect(u.voice).toBe(vv[0]);
    expect(u.lang).toBe('vi-VN');
    expect([u.rate, u.pitch, u.volume]).toEqual([1.5, 0.8, 0.5]);
  });

  it('turns boundary events into word events (length from the text when missing)', () => {
    const s = synth(vv);
    const p = new WebSpeechProvider(s, (t) => new FakeUtteranceImpl(t));
    const events: SpeechEvent[] = [];
    const h = p.speak('xin chào bạn', {}, (e) => events.push(e));
    const u = s.queue[0]!;
    u.onstart?.({});
    u.onboundary?.({ name: 'word', charIndex: 0, charLength: 3 });
    u.onboundary?.({ name: 'word', charIndex: 4 }); // no charLength: "chào"
    u.onboundary?.({ name: 'sentence', charIndex: 0, charLength: 12 });
    u.onend?.({});
    expect(events).toEqual([
      { type: 'start' },
      { type: 'word', offset: 0, length: 3 },
      { type: 'word', offset: 4, length: 4 },
      { type: 'sentence', offset: 0, length: 12 },
      { type: 'end' },
    ]);
    // After cancel nothing more is reported.
    const ev2: SpeechEvent[] = [];
    const h2 = p.speak('a b', {}, (e) => ev2.push(e));
    h2.cancel();
    s.queue[1]!.onerror?.({ error: 'interrupted' });
    expect(ev2).toEqual([]);
    expect(s.cancelled).toBe(1);
    void h;
  });

  it('waits for a late voice list', async () => {
    const listeners: (() => void)[] = [];
    let list: VoiceLike[] = [];
    const s: SynthesisLike = {
      ...synth([]),
      getVoices: () => list,
      addEventListener: (_t, l) => listeners.push(l),
      removeEventListener: () => {},
    };
    const p = new WebSpeechProvider(s, (t) => new FakeUtteranceImpl(t), 1000);
    const got = p.voices();
    list = vv;
    listeners.forEach((l) => l());
    expect((await got).map((v) => v.id)).toEqual(['urn:vi']);
  });

  it('exists only with speechSynthesis', () => {
    expect(webSpeech({})).toBeUndefined();
    expect(
      webSpeech({ speechSynthesis: synth([]), SpeechSynthesisUtterance: FakeUtteranceImpl }),
    ).toBeInstanceOf(WebSpeechProvider);
  });
});

describe('FakeSpeechProvider', () => {
  it('reports words in order, holds while paused and ends', () => {
    const p = new FakeSpeechProvider();
    const events: SpeechEvent[] = [];
    const h = p.speak('một hai', {}, (e) => events.push(e));
    p.step();
    h.pause();
    expect(p.step()).toBe(false);
    h.resume();
    p.step();
    p.step();
    expect(events).toEqual([
      { type: 'start' },
      { type: 'word', offset: 0, length: 3 },
      { type: 'pause' },
      { type: 'resume' },
      { type: 'word', offset: 4, length: 3 },
      { type: 'end' },
    ]);
    expect(p.current).toBeUndefined();
  });
});
