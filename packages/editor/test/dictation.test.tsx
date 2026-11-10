// @vitest-environment happy-dom
import { createI18n, I18nextProvider } from '@blueoffice/i18n';
import {
  FakeDictationProvider,
  byteToUtf16,
  utf16ToByte,
  type EditCommand,
  type EditResult,
  type EngineHost,
  type Pos,
} from '@blueoffice/platform';
import { act, cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { useEffect } from 'react';
import { afterEach, describe, expect, it } from 'vitest';
import { CommandRegistry } from '../src/commands';
import {
  DictationSession,
  composeDictation,
  parseDictation,
  type ComposeOptions,
  type DictationEdit,
  type DictationState,
  type DictationTarget,
} from '../src/dictation';
import { useDictation, type Dictation } from '../src/DictationBar';
import { DICTATE_SHORTCUT, registerDictationCommands } from '../src/dictationCommands';
import { DOC_LINE_BREAK, docDictationTarget } from '../src/docDictation';

afterEach(() => {
  cleanup();
  localStorage.clear();
});

const VI: ComposeOptions = { lang: 'vi-VN', lineBreak: DOC_LINE_BREAK, paragraph: '\n' };
const EN: ComposeOptions = { ...VI, lang: 'en-US' };

/** Text after dictating `phrases` one after the other into an empty paragraph. */
function dictate(phrases: string[], o: ComposeOptions, start = ''): string {
  let s = start;
  for (const p of phrases) {
    const e = composeDictation(s, '', p, o);
    s = s.slice(0, s.length - e.remove) + e.text;
  }
  return s;
}

describe('voice commands', () => {
  it.each([
    ['xin chào chấm', 'Xin chào.'],
    ['xin chào dấu chấm', 'Xin chào.'],
    ['xin chào dấu phẩy tôi là an chấm hỏi', 'Xin chào, tôi là an?'],
    ['xin chào phẩy bạn', 'Xin chào, bạn'],
    ['tuyệt vời chấm than', 'Tuyệt vời!'],
    ['ghi chú hai chấm một', 'Ghi chú: một'],
    ['thứ nhất chấm phẩy thứ hai', 'Thứ nhất; thứ hai'],
    ['đợi đã ba chấm', 'Đợi đã…'],
    ['dòng một xuống dòng dòng hai', `Dòng một${DOC_LINE_BREAK}Dòng hai`],
    ['hết đoạn đoạn mới mở đầu', 'Hết đoạn\nMở đầu'],
    ['anh ấy mở ngoặc kỹ sư đóng ngoặc đến', 'Anh ấy (kỹ sư) đến'],
    ['cô nói mở ngoặc kép xin chào đóng ngoặc kép', 'Cô nói “xin chào”'],
    ['một hai ba xoá từ vừa rồi bốn', 'Một hai bốn'],
    // The other placement of the tone mark, and upper-case commands.
    ['một hai xóa từ vừa rồi', 'Một'],
    ['XIN CHÀO Chấm', 'XIN CHÀO.'],
    // The recogniser's own punctuation; a spoken mark it also wrote counts once.
    ['xin chào, bạn khỏe không?', 'Xin chào, bạn khỏe không?'],
    ['xong chấm.', 'Xong.'],
    ['câu một chấm bạn khỏe không chấm hỏi', 'Câu một. Bạn khỏe không?'],
    // Left to right: "hai chấm" is a colon.
    ['câu hai chấm hỏi', 'Câu: hỏi'],
  ])('vi: %s', (spoken, written) => {
    expect(dictate([spoken], VI)).toBe(written);
  });

  it.each([
    ['hello comma world period', 'Hello, world.'],
    ['hello world full stop', 'Hello world.'],
    ['is it question mark yes exclamation point', 'Is it? Yes!'],
    ['wow exclamation mark', 'Wow!'],
    ['note colon one semicolon two', 'Note: one; two'],
    ['first new line second new paragraph third', `First${DOC_LINE_BREAK}Second\nThird`],
    [
      'say open quote hi close quote and open parenthesis maybe close parenthesis',
      'Say “hi” and (maybe)',
    ],
    ['one two delete last word three', 'One three'],
    ['wait ellipsis', 'Wait…'],
    ['a dot dot dot', 'A…'],
  ])('en: %s', (spoken, written) => {
    expect(dictate([spoken], EN)).toBe(written);
  });

  it('only the language being dictated has commands', () => {
    expect(dictate(['hello period'], VI)).toBe('Hello period');
    expect(dictate(['xin chào chấm'], EN)).toBe('Xin chào chấm');
    expect(parseDictation('một, hai', 'vi-VN')).toEqual([
      { kind: 'word', text: 'một' },
      { kind: 'punct', text: ',' },
      { kind: 'word', text: 'hai' },
    ]);
  });
});

describe('spacing and capitals', () => {
  it.each<[string, string, string, string, DictationEdit]>([
    [
      'after a sentence and a space',
      'Xin chào. ',
      '',
      'tôi là an',
      { remove: 0, text: 'Tôi là an' },
    ],
    ['after a sentence end', 'Xin chào.', '', 'tôi', { remove: 0, text: ' Tôi' }],
    ['mid-sentence', 'Xin chào', '', 'bạn', { remove: 0, text: ' bạn' }],
    ['a mark after a space', 'Xin chào ', '', 'chấm', { remove: 1, text: '.' }],
    ['a word deleted', 'Một hai', '', 'xoá từ vừa rồi', { remove: 4, text: '' }],
    ['a mark deleted', 'Một hai.', '', 'xoá từ vừa rồi', { remove: 1, text: '' }],
    ['a word before text', '', 'bạn ơi', 'chào', { remove: 0, text: 'Chào ' }],
    ['after a line break', `abc${DOC_LINE_BREAK}`, '', 'x', { remove: 0, text: 'X' }],
    [
      'after an opening bracket at a sentence start',
      '(',
      '',
      'ví dụ',
      { remove: 0, text: 'Ví dụ' },
    ],
    [
      'after a closing quote ending a sentence',
      'Anh nói “Chào.”',
      '',
      'rồi đi',
      { remove: 0, text: ' Rồi đi' },
    ],
    ['a Vietnamese capital', '', '', 'đà nẵng', { remove: 0, text: 'Đà nẵng' }],
    [
      'nothing to delete across a line',
      `abc${DOC_LINE_BREAK}`,
      '',
      'xoá từ vừa rồi',
      { remove: 0, text: '' },
    ],
  ])('%s', (_name, before, after, spoken, edit) => {
    expect(composeDictation(before, after, spoken, VI)).toEqual(edit);
  });

  it('capitals can be turned off; emoji are never split', () => {
    expect(composeDictation('', '', 'xin chào', { ...VI, capitalize: false }).text).toBe(
      'xin chào',
    );
    expect(composeDictation('Vui 😀', '', 'xoá từ vừa rồi', VI)).toEqual({ remove: 3, text: '' });
  });

  it('builds a text over several phrases', () => {
    expect(dictate(['hôm nay trời đẹp chấm', 'chúng ta đi chơi nhé chấm hỏi', 'đi thôi'], VI)).toBe(
      'Hôm nay trời đẹp. Chúng ta đi chơi nhé? Đi thôi',
    );
  });
});

/** A target over a plain string with the caret at the end. */
class TextTarget implements DictationTarget {
  lineBreak = '\v';
  paragraph = '\n';
  text = '';
  previews: (string | null)[] = [];
  applied: DictationEdit[] = [];
  missing = false;
  async context() {
    return this.missing ? null : { before: this.text, after: '' };
  }
  async apply(e: DictationEdit) {
    this.applied.push(e);
    this.text = this.text.slice(0, this.text.length - e.remove) + e.text;
  }
  preview(t: string | null) {
    this.previews.push(t);
  }
}

describe('DictationSession', () => {
  const setup = (lang = 'vi-VN') => {
    const p = new FakeDictationProvider();
    const tg = new TextTarget();
    const states: DictationState[] = [];
    const errors: string[] = [];
    const interims: string[] = [];
    const s = new DictationSession(p, tg, lang, {
      onState: (st) => states.push(st),
      onError: (c) => errors.push(c),
      onInterim: (t) => interims.push(t),
    });
    return { p, tg, s, states, errors, interims };
  };

  it('previews interim results without writing; writes each final phrase once', async () => {
    const { p, tg, s, states, interims } = setup();
    await s.start();
    expect(states).toEqual(['starting', 'listening']);
    expect(p.current?.lang).toBe('vi-VN');
    p.interim('xin');
    p.interim('xin chào chấm');
    expect(tg.text).toBe('');
    expect(tg.previews).toEqual(['Xin', 'Xin chào.']);
    expect(interims).toEqual(['Xin', 'Xin chào.']);
    p.final('xin chào chấm');
    await s.written();
    expect(tg.text).toBe('Xin chào.');
    expect(tg.previews.at(-1)).toBeNull();
    // The next interim composes against what was written.
    p.interim('tôi');
    expect(tg.previews.at(-1)).toBe(' Tôi');
    p.final('tôi là an');
    await s.written();
    expect(tg.text).toBe('Xin chào. Tôi là an');
    expect(tg.applied).toHaveLength(2);
  });

  it('stop writes what the recogniser still had; pause and resume start a new listen', async () => {
    const { p, tg, s, states } = setup();
    await s.start();
    s.pause();
    expect(s.state).toBe('paused');
    expect(p.sessions[0]!.stopped).toBe(true);
    await s.resume();
    expect(p.sessions).toHaveLength(2);
    expect(s.state).toBe('listening');
    p.pendingOnStop = 'cuối cùng';
    s.stop();
    await s.written();
    expect(tg.text).toBe('Cuối cùng');
    expect(states.at(-1)).toBe('idle');
  });

  it('reports errors and a missing target', async () => {
    const { p, s, errors } = setup();
    await s.start();
    p.fail('noPermission');
    expect(s.state).toBe('error');
    expect(errors).toEqual(['noPermission']);
    const other = setup();
    other.tg.missing = true;
    await other.s.start();
    expect(other.errors).toEqual(['noTarget']);
    expect(other.p.sessions).toHaveLength(0);
  });

  it('changes language by listening again', async () => {
    const { p, s } = setup();
    await s.start();
    await s.setLanguage('en-US');
    expect(p.sessions.map((x) => x.lang)).toEqual(['vi-VN', 'en-US']);
    expect(p.sessions[0]!.stopped).toBe(true);
    p.final('hello period');
    await s.written();
  });
});

/** A one-paragraph-list document with an undo stack (`insertText` / `deleteBackward` = one step). */
class FakeDoc {
  paras = [''];
  sel: { anchor: Pos; focus: Pos } = { anchor: { p: 0, o: 0 }, focus: { p: 0, o: 0 } };
  undo: { paras: string[]; sel: FakeDoc['sel'] }[] = [];
  commands: EditCommand[] = [];

  private range(): [Pos, Pos] {
    const { anchor: a, focus: b } = this.sel;
    return a.p < b.p || (a.p === b.p && a.o <= b.o) ? [a, b] : [b, a];
  }

  private replace(text: string) {
    this.undo.push({ paras: [...this.paras], sel: this.sel });
    const [a, b] = this.range();
    const pa = this.paras[a.p]!;
    const pb = this.paras[b.p]!;
    const head = pa.slice(0, byteToUtf16(pa, a.o));
    const tail = pb.slice(byteToUtf16(pb, b.o));
    const lines = (head + text).split('\n');
    const last = lines.length - 1;
    const caret = { p: a.p + last, o: utf16ToByte(lines[last]! + tail, lines[last]!.length) };
    lines[last] += tail;
    this.paras.splice(a.p, b.p - a.p + 1, ...lines);
    this.sel = { anchor: caret, focus: caret };
  }

  command(cmd: EditCommand): EditResult {
    this.commands.push(cmd);
    const out = { selection: this.sel } as unknown as EditResult;
    switch (cmd.cmd) {
      case 'speechText': {
        const from = cmd.from ?? 0;
        const to = cmd.to ?? this.paras.length - 1;
        const paras = this.paras.slice(from, to + 1).map((text, i) => ({ p: from + i, text }));
        return { ...out, speech: { paras } };
      }
      case 'setSelection':
        this.sel = cmd.selection;
        break;
      case 'insertText':
        this.replace(cmd.text);
        break;
      case 'deleteBackward': {
        const [a, b] = this.range();
        if (a.p === b.p && a.o === b.o) throw new Error('only selections here');
        this.replace('');
        break;
      }
      case 'undo': {
        const u = this.undo.pop();
        if (u) {
          this.paras = u.paras;
          this.sel = u.sel;
        }
        break;
      }
      default:
        break;
    }
    return { ...out, selection: this.sel };
  }
}

describe('BlueDoc target', () => {
  it('one undo step per final phrase, deleted words included', async () => {
    const doc = new FakeDoc();
    const engine = { command: async (c: EditCommand) => doc.command(c) } as unknown as EngineHost;
    const previews: (string | null)[] = [];
    const tg = docDictationTarget(
      engine,
      async (c) => doc.command(c),
      () => ({
        setDictationPreview: (t) => previews.push(t),
      }),
    );
    const p = new FakeDictationProvider();
    const s = new DictationSession(p, tg, 'vi-VN');
    await s.start();
    p.interim('hôm nay');
    expect(previews).toEqual(['Hôm nay']);
    expect(doc.paras).toEqual(['']);
    for (const phrase of [
      'hôm nay trời đẹp chấm',
      'chúng ta đi chơi xuống dòng tạm biệt',
      'đoạn mới bạn nhé',
      'xoá từ vừa rồi',
    ])
      p.final(phrase);
    await s.written();
    expect(doc.paras).toEqual([
      `Hôm nay trời đẹp. Chúng ta đi chơi${DOC_LINE_BREAK}Tạm biệt`,
      'Bạn',
    ]);
    const edits = doc.commands.filter((c) => c.cmd === 'insertText' || c.cmd === 'deleteBackward');
    expect(edits.map((c) => c.cmd)).toEqual([
      'insertText',
      'insertText',
      'insertText',
      'deleteBackward',
    ]);
    expect(doc.undo).toHaveLength(4);
    // Undo takes back one phrase at a time.
    doc.command({ cmd: 'undo' });
    expect(doc.paras).toEqual([
      `Hôm nay trời đẹp. Chúng ta đi chơi${DOC_LINE_BREAK}Tạm biệt`,
      'Bạn nhé',
    ]);
    doc.command({ cmd: 'undo' });
    expect(doc.paras).toEqual([`Hôm nay trời đẹp. Chúng ta đi chơi${DOC_LINE_BREAK}Tạm biệt`]);
  });

  it('replaces the selection, spacing from the text around it', async () => {
    const doc = new FakeDoc();
    doc.paras = ['Xin X bạn'];
    doc.sel = { anchor: { p: 0, o: 4 }, focus: { p: 0, o: 5 } };
    const engine = { command: async (c: EditCommand) => doc.command(c) } as unknown as EngineHost;
    const tg = docDictationTarget(
      engine,
      async (c) => doc.command(c),
      () => null,
    );
    expect(await tg.context()).toEqual({ before: 'Xin ', after: ' bạn' });
    await tg.apply(composeDictation('Xin ', ' bạn', 'chào', VI));
    expect(doc.paras).toEqual(['Xin chào bạn']);
  });
});

describe('dictation bar and commands', () => {
  const i18n = createI18n('vi-VN');

  function Harness({
    provider,
    target,
    onReady,
  }: {
    provider: FakeDictationProvider | undefined;
    target: DictationTarget | null;
    onReady: (d: Dictation) => void;
  }) {
    const d = useDictation(provider, () => target);
    useEffect(() => {
      onReady(d);
    });
    return <>{d.bar}</>;
  }

  function mount(provider: FakeDictationProvider | undefined, target: DictationTarget | null) {
    let d: Dictation | null = null;
    render(
      <I18nextProvider i18n={i18n}>
        <Harness provider={provider} target={target} onReady={(x) => (d = x)} />
      </I18nextProvider>,
    );
    const r = new CommandRegistry<object>();
    registerDictationCommands(r, { enabled: () => true, current: () => d });
    return { d: () => d!, r };
  }

  it('toggles from the command, shows status and interim text, stops', async () => {
    const p = new FakeDictationProvider();
    const tg = new TextTarget();
    const { d, r } = mount(p, tg);
    expect(r.get('edit.dictate')?.shortcut).toBe(DICTATE_SHORTCUT);
    await act(async () => {
      await r.execute('edit.dictate', {});
    });
    expect(screen.getByTestId('dictation-status').textContent).toBe('Đang nghe…');
    expect(r.isChecked('edit.dictate', {})).toBe(true);
    act(() => {
      p.interim('xin chào');
    });
    expect(screen.getByTestId('dictation-interim').textContent).toBe('Xin chào');
    await act(async () => {
      p.final('xin chào chấm');
      await d().written();
    });
    expect(tg.text).toBe('Xin chào.');
    expect(screen.queryByTestId('dictation-interim')).toBeNull();
    await act(async () => {
      await r.execute('edit.dictatePause', {});
    });
    expect(screen.getByTestId('dictation-status').textContent).toBe('Đã tạm dừng');
    await act(async () => {
      await r.execute('edit.dictate', {});
    });
    expect(screen.queryByTestId('dictation-bar')).toBeNull();
  });

  it('keeps the language picked and listens in it', async () => {
    const p = new FakeDictationProvider();
    const { d } = mount(p, new TextTarget());
    await act(async () => {
      await d().start();
    });
    fireEvent.change(screen.getByTestId('dictation-lang'), { target: { value: 'en-US' } });
    await waitFor(() => expect(p.current?.lang).toBe('en-US'));
    expect(localStorage.getItem('bo.dictation.lang')).toBe('en-US');
  });

  it('says when dictation is unavailable, refused or has no target', async () => {
    const first = mount(undefined, new TextTarget());
    await act(async () => {
      await first.d().start();
    });
    expect(screen.getByTestId('dictation-status').textContent).toContain('chưa dùng được');
    cleanup();
    const p = new FakeDictationProvider();
    p.startError = 'noPermission';
    const second = mount(p, new TextTarget());
    await act(async () => {
      await second.d().start();
    });
    await waitFor(() =>
      expect(screen.getByTestId('dictation-status').textContent).toContain('micrô'),
    );
    cleanup();
    const third = mount(new FakeDictationProvider(), null);
    await act(async () => {
      await third.d().start();
    });
    expect(screen.getByTestId('dictation-status').textContent).toContain('con trỏ');
  });
});
