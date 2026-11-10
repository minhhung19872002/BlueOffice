// @vitest-environment happy-dom
import { createI18n, I18nextProvider } from '@blueoffice/i18n';
import {
  FakeSpeechProvider,
  type EditCommand,
  type EditResult,
  type EngineHost,
  type SelRect,
  type SpeechVoice,
} from '@blueoffice/platform';
import { act, cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { useEffect } from 'react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { CommandRegistry } from '../src/commands';
import { docReadRequest, docUnits } from '../src/docReadAloud';
import {
  ReadAloudSession,
  type ReadAloudSettings,
  type ReadPosition,
  type ReadUnit,
} from '../src/readAloud';
import { useReadAloud, type ReadAloud, type ReadAloudRequest } from '../src/ReadAloudBar';
import { READ_ALOUD_SHORTCUT, registerReadAloudCommands } from '../src/readAloudCommands';

afterEach(() => {
  cleanup();
  localStorage.clear();
});

const VI: SpeechVoice = { id: 'vi', name: 'An', lang: 'vi-VN', local: true, default: false };
const EN: SpeechVoice = { id: 'en', name: 'Sam', lang: 'en-US', local: true, default: true };
const EN2: SpeechVoice = { id: 'en2', name: 'Kim', lang: 'en-GB', local: true, default: false };

const settings: ReadAloudSettings = {
  rate: 1,
  pitch: 1,
  volume: 1,
  voice: null,
  fallbackLang: 'vi-VN',
};

function session(units: ReadUnit[], voices = [VI, EN], extra: Partial<ReadAloudSettings> = {}) {
  const p = new FakeSpeechProvider(voices);
  const words: ReadPosition[] = [];
  const events: string[] = [];
  const s = new ReadAloudSession(
    p,
    units,
    { ...settings, ...extra },
    {
      onWord: (w) => words.push(w),
      onUnit: (u) => events.push(`unit:${u}`),
      onState: (st) => events.push(st),
      onNoVoice: (l) => events.push(`noVoice:${l}`),
      onError: (e) => events.push(`error:${e}`),
      onDone: () => events.push('done'),
    },
  );
  const said = (u: ReadPosition) => units[u.unit]!.text.slice(u.start, u.end);
  return { p, s, words, events, said };
}

describe('ReadAloudSession', () => {
  it('reads units in order and reports each word in its unit', async () => {
    const { p, s, words, events, said } = session([
      { text: 'Xin chào', lang: 'vi-VN' },
      { text: 'Hello world', lang: 'en-US' },
    ]);
    await s.start();
    p.run();
    expect(words.map(said)).toEqual(['Xin', 'chào', 'Hello', 'world']);
    expect(words.map((w) => w.unit)).toEqual([0, 0, 1, 1]);
    expect(events).toEqual(['unit:0', 'playing', 'unit:1', 'idle', 'done']);
    // The voice follows the language of each unit.
    expect(p.spoken.map((u) => u.options.voice)).toEqual(['vi', 'en']);
    expect(p.spoken.map((u) => u.options.lang)).toEqual(['vi-VN', 'en-US']);
  });

  it('maps word offsets with Vietnamese diacritics and emoji (UTF-16 units)', async () => {
    // "Tiếng 😀 Việt": T,i,ế,n,g = 5 units; space; 😀 = 2 units; space; V,i,ệ,t = 4 units.
    const text = 'Tiếng 😀 Việt';
    const { p, s, words } = session([{ text, lang: 'vi-VN' }]);
    await s.start();
    p.run();
    expect(words.map((w) => [w.start, w.end])).toEqual([
      [0, 5],
      [6, 8],
      [9, 13],
    ]);
  });

  it('speaks long paragraphs in chunks and keeps offsets in the paragraph', async () => {
    const text = 'Một hai ba. Bốn năm sáu. Bảy tám chín.';
    const { p, s, words, said } = session([{ text, lang: 'vi-VN' }], [VI], { maxChunk: 14 });
    await s.start();
    p.run();
    expect(p.spoken.map((u) => u.text)).toEqual(['Một hai ba. ', 'Bốn năm sáu. ', 'Bảy tám chín.']);
    expect(words.map(said)).toEqual(text.split(' '));
    expect(words[3]!.start).toBe(text.indexOf('Bốn'));
  });

  it('stops with a notice when no voice speaks the language', async () => {
    const { p, s, events } = session([{ text: 'Xin chào các bạn', lang: 'vi-VN' }], [EN]);
    await s.start();
    expect(p.spoken).toHaveLength(0);
    expect(events).toContain('noVoice:vi-VN');
    expect(s.current.state).toBe('idle');
  });

  it('a chosen voice speaks its language; other languages still pick by language', async () => {
    const { p, s } = session(
      [
        { text: 'Hello there', lang: 'en-US' },
        { text: 'Xin chào', lang: 'vi-VN' },
      ],
      [VI, EN, EN2],
      { voice: 'en2' },
    );
    await s.start();
    p.run();
    expect(p.spoken.map((u) => u.options.voice)).toEqual(['en2', 'vi']);
  });

  it('pauses and resumes', async () => {
    const { p, s, words, said } = session([{ text: 'một hai ba', lang: 'vi-VN' }]);
    await s.start();
    p.step();
    s.pause();
    expect(s.current.state).toBe('paused');
    expect(p.step()).toBe(false);
    s.resume();
    expect(s.current.state).toBe('playing');
    p.run();
    expect(words.map(said)).toEqual(['một', 'hai', 'ba']);
  });

  it('moves to the next and previous unit', async () => {
    const units = [
      { text: 'một hai', lang: 'vi-VN' },
      { text: 'ba bốn', lang: 'vi-VN' },
      { text: 'năm sáu', lang: 'vi-VN' },
    ];
    const { p, s, words, said } = session(units);
    await s.start();
    p.step(); // "một"
    s.next();
    p.step(); // "ba"
    s.next();
    p.step(); // "năm"
    s.previous();
    p.step(); // "ba" again
    expect(words.map(said)).toEqual(['một', 'ba', 'năm', 'ba']);
    // Earlier utterances were cancelled.
    expect(p.spoken.filter((u) => u.cancelled)).toHaveLength(3);
    s.next();
    s.next(); // past the last unit
    expect(s.current.state).toBe('idle');
  });

  it('starts at the word around an offset and reads only a range', async () => {
    const { p, s, words, said } = session([
      { text: 'đầu giữa cuối', lang: 'vi-VN' },
      { text: 'một hai ba', lang: 'vi-VN', range: { start: 0, end: 7 } },
    ]);
    await s.start(0, 6); // inside "giữa"
    p.run();
    expect(words.map(said)).toEqual(['giữa', 'cuối', 'một', 'hai']);
  });

  it('does not speak skipped text or object placeholders, offsets unchanged', async () => {
    const { p, s, words, said } = session([
      { text: 'ab￼cd ẩn ef', lang: 'vi-VN', skip: [{ start: 6, end: 9 }] },
    ]);
    await s.start();
    p.run();
    expect(p.spoken[0]!.text).toBe('ab cd    ef');
    expect(words.map(said)).toEqual(['ab', 'cd', 'ef']);
  });

  it('a speed change restarts the sentence at the current word', async () => {
    const { p, s, words, said } = session([{ text: 'một hai ba bốn', lang: 'vi-VN' }]);
    await s.start();
    p.step();
    p.step(); // "hai"
    s.update({ rate: 1.5 });
    p.run();
    expect(p.spoken.map((u) => [u.text, u.options.rate])).toEqual([
      ['một hai ba bốn', 1],
      ['hai ba bốn', 1.5],
    ]);
    expect(words.map(said)).toEqual(['một', 'hai', 'hai', 'ba', 'bốn']);
  });

  it('reports engine errors', async () => {
    const { p, s, events } = session([{ text: 'một hai', lang: 'vi-VN' }]);
    await s.start();
    p.fail('audio-busy');
    expect(events).toContain('error:audio-busy');
    expect(s.current.state).toBe('idle');
  });

  it('speaks with the platform voice when it lists none', async () => {
    const { p, s } = session([{ text: 'xin chào', lang: 'vi-VN' }], []);
    await s.start();
    expect(p.spoken[0]!.options).toMatchObject({ voice: null, lang: 'vi-VN' });
  });
});

describe('BlueDoc mapping', () => {
  it('turns byte offsets of the engine into UTF-16 units', () => {
    // "Việt" = V i ệ(3 bytes) t → 6 bytes, 4 units; "😀" = 4 bytes, 2 units.
    const paras = [
      { p: 0, text: 'Việt 😀 Nam', detected: 'vi-VN' },
      { p: 1, text: 'Hello world', lang: 'en-GB', skip: [[0, 6]] as [number, number][] },
      { p: 2, text: 'đi đâu', lang: 'en-US', detected: 'vi-VN' },
    ];
    const { units } = docUnits(paras, 'vi-VN');
    expect(units.map((u) => u.lang)).toEqual(['vi-VN', 'en-GB', 'vi-VN']);
    expect(units[1]!.skip).toEqual([{ start: 0, end: 6 }]);
    // Selection from byte 7 of paragraph 0 (after "Việt ") to byte 3 of paragraph 2 ("đi").
    const sel = docUnits(paras, 'vi-VN', { from: { p: 0, o: 7 }, to: { p: 2, o: 3 } });
    expect(sel.units.map((u) => u.range)).toEqual([
      { start: 5, end: 11 },
      undefined,
      { start: 0, end: 2 },
    ]);
  });

  function fakeEngine(
    paras: { p: number; text: string; detected?: string }[],
    caret: [number, number],
  ) {
    const commands: EditCommand[] = [];
    const engine = {
      command: vi.fn(async (c: EditCommand) => {
        commands.push(c);
        const base = {
          selection: { anchor: { p: caret[0], o: caret[1] }, focus: { p: caret[0], o: caret[1] } },
        } as Partial<EditResult>;
        if (c.cmd === 'speechText') return { ...base, speech: { paras } } as EditResult;
        if (c.cmd === 'speechRects') {
          const rects = c.ranges.map((r): SelRect[] => [
            { page: 0, x: r.start, y: r.p * 10, w: r.end - r.start, h: 10 },
          ]);
          return { ...base, speech: { rects } } as EditResult;
        }
        return base as EditResult;
      }),
    } as unknown as EngineHost;
    return { engine, commands };
  }

  it('starts at the caret and highlights the word by byte range', async () => {
    const text = 'Chào 😀 Việt Nam';
    const { engine, commands } = fakeEngine(
      [
        { p: 0, text: 'Trước', detected: 'vi-VN' },
        { p: 1, text, detected: 'vi-VN' },
      ],
      // Caret at byte 11: after "Chào 😀 " (C h à(2) o = 5 bytes, space, 😀 4 bytes, space).
      [1, 11],
    );
    const shown: [SelRect[], SelRect[]][] = [];
    const view = { setSpeechHighlights: (w: SelRect[], p: SelRect[]) => void shown.push([w, p]) };
    const req = await docReadRequest(engine, () => view, 'vi-VN');
    expect(req.unit).toBe(1);
    expect(req.offset).toBe(8); // UTF-16: "Chào " 5 + 😀 2 + space 1
    const provider = new FakeSpeechProvider([VI]);
    const s = new ReadAloudSession(provider, req.units, settings, {
      onWord: (w) => req.onWord?.(w),
    });
    await s.start(req.unit, req.offset);
    expect(provider.spoken[0]!.text).toBe('Việt Nam');
    provider.step(); // "Việt"
    await waitFor(() => expect(shown).toHaveLength(1));
    const rects = commands.find((c) => c.cmd === 'speechRects');
    // "Việt" = bytes 11..17 of the paragraph; the paragraph = 0..text bytes.
    expect(rects).toEqual({
      cmd: 'speechRects',
      ranges: [
        { p: 1, start: 11, end: 17 },
        { p: 1, start: 0, end: new TextEncoder().encode(text).length },
      ],
    });
    expect(shown[0]![0]).toEqual([{ page: 0, x: 11, y: 10, w: 6, h: 10 }]);
    req.onStop?.();
    expect(shown[shown.length - 1]).toEqual([[], []]);
  });
});

describe('read aloud bar and commands', () => {
  const i18n = createI18n('vi-VN');

  function Harness({
    provider,
    request,
    onReady,
  }: {
    provider: FakeSpeechProvider | undefined;
    request: ReadAloudRequest;
    onReady: (r: ReadAloud) => void;
  }) {
    const ra = useReadAloud(provider, 'paragraph');
    useEffect(() => {
      onReady(ra);
    });
    return (
      <>
        <button type="button" onClick={() => void ra.start(request)}>
          go
        </button>
        {ra.bar}
      </>
    );
  }

  function mount(provider: FakeSpeechProvider | undefined, units: ReadUnit[]) {
    let ra: ReadAloud | null = null;
    const words: ReadPosition[] = [];
    const stop = vi.fn();
    render(
      <I18nextProvider i18n={i18n}>
        <Harness
          provider={provider}
          request={{ units, onWord: (w) => words.push(w), onStop: stop }}
          onReady={(r) => (ra = r)}
        />
      </I18nextProvider>,
    );
    return { ra: () => ra, words, stop };
  }

  it('shows the bar, pauses, resumes and stops', async () => {
    const p = new FakeSpeechProvider([VI, EN]);
    const { words, stop } = mount(p, [
      { text: 'một hai', lang: 'vi-VN' },
      { text: 'ba', lang: 'vi-VN' },
    ]);
    fireEvent.click(screen.getByText('go'));
    const bar = await screen.findByTestId('read-aloud-bar');
    await waitFor(() => expect(bar.dataset.state).toBe('playing'));
    act(() => void p.step());
    expect(words).toHaveLength(1);
    fireEvent.click(screen.getByTestId('read-aloud-play'));
    expect(bar.dataset.state).toBe('paused');
    expect(screen.getByTestId('read-aloud-play').getAttribute('aria-label')).toBe('Tiếp tục đọc');
    fireEvent.click(screen.getByTestId('read-aloud-play'));
    expect(bar.dataset.state).toBe('playing');
    fireEvent.click(screen.getByTestId('read-aloud-next'));
    act(() => void p.step());
    expect(words[1]).toMatchObject({ unit: 1, start: 0, end: 2 });
    // Voices are listed for the user to choose.
    await waitFor(() =>
      expect(screen.getByTestId('read-aloud-voice').querySelectorAll('option')).toHaveLength(3),
    );
    fireEvent.change(screen.getByTestId('read-aloud-rate'), { target: { value: '1.5' } });
    expect(p.current?.options.rate).toBe(1.5);
    expect(JSON.parse(localStorage.getItem('bo.readAloud') ?? '{}')).toMatchObject({ rate: 1.5 });
    fireEvent.click(screen.getByTestId('read-aloud-stop'));
    expect(screen.queryByTestId('read-aloud-bar')).toBeNull();
    expect(stop).toHaveBeenCalled();
  });

  it('tells the user when no Vietnamese voice is installed', async () => {
    mount(new FakeSpeechProvider([EN]), [{ text: 'Xin chào các bạn', lang: 'vi-VN' }]);
    fireEvent.click(screen.getByText('go'));
    const notice = await screen.findByTestId('read-aloud-notice');
    expect(notice.textContent).toContain('tiếng Việt');
  });

  it('tells the user when the platform cannot speak', async () => {
    mount(undefined, [{ text: 'Xin chào', lang: 'vi-VN' }]);
    fireEvent.click(screen.getByText('go'));
    expect((await screen.findByTestId('read-aloud-notice')).textContent).toBe(
      i18n.t('readAloud.unavailable'),
    );
  });

  it('registers the commands with a shortcut', async () => {
    const r = new CommandRegistry<object>();
    let current: Partial<ReadAloud> | null = { active: false, state: 'idle' };
    const begin = vi.fn();
    registerReadAloudCommands(r, {
      enabled: () => true,
      begin,
      current: () => current as ReadAloud,
    });
    expect(
      r.match({ key: 'R', ctrlKey: true, shiftKey: true, altKey: true, metaKey: false })?.id,
    ).toBe('review.readAloud');
    expect(READ_ALOUD_SHORTCUT).toBe('Ctrl+Alt+Shift+R');
    await r.execute('review.readAloud', {});
    expect(begin).toHaveBeenCalledTimes(1);
    expect(r.isEnabled('review.readAloudNext', {})).toBe(false);
    const stop = vi.fn();
    const next = vi.fn();
    current = { active: true, state: 'playing', stop, next };
    expect(r.isChecked('review.readAloud', {})).toBe(true);
    await r.execute('review.readAloudNext', {});
    expect(next).toHaveBeenCalled();
    await r.execute('review.readAloud', {});
    expect(stop).toHaveBeenCalled();
  });
});
