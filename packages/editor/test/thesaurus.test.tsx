// @vitest-environment happy-dom
import { createI18n, I18nextProvider } from '@blueoffice/i18n';
import {
  LazyThesaurus,
  type EditCommand,
  type EditResult,
  type ThesaurusData,
} from '@blueoffice/platform';
import type { MenuEntry } from '@blueoffice/ui';
import { act, cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { useEffect } from 'react';
import { afterEach, describe, expect, it } from 'vitest';
import { docReplaceWord, docWordAtCaret } from '../src/docThesaurus';
import {
  thesaurusMenuEntry,
  useThesaurus,
  type Thesaurus,
  type ThesaurusOpen,
} from '../src/ThesaurusPane';

afterEach(() => cleanup());

const DATA: ThesaurusData = {
  format: 'bo-thesaurus/1',
  lang: 'en',
  source: 'test',
  version: '1',
  license: 'CC0-1.0',
  words: ['glad', 'joyful', 'unhappy', 'sprint', 'dash', 'race'],
  synsets: [
    ['a', 'feeling joy', [0, 1]],
    ['v', 'move fast on foot', [3, 4]],
    ['n', 'a contest of speed', [5]],
  ],
  entries: { happy: [[0, [2]]], run: [[1], [2]] },
  forms: { ran: 'run' },
};

const provider = () =>
  new LazyThesaurus({ en: async () => new TextEncoder().encode(JSON.stringify(DATA)) });

/** A fake BlueDoc engine: paragraphs of text, a selection, `insertText` over it. */
function fakeDoc(paras: string[]) {
  const enc = new TextEncoder();
  const dec = new TextDecoder();
  let selection = { anchor: { p: 0, o: 0 }, focus: { p: 0, o: 0 } };
  const log: EditCommand[] = [];
  const doc = [...paras];
  const run = async (c: EditCommand): Promise<EditResult> => {
    log.push(c);
    const r = { selection } as unknown as EditResult;
    if (c.cmd === 'setSelection') {
      selection = c.selection;
      r.selection = selection;
    } else if (c.cmd === 'speechText')
      r.speech = {
        paras: doc
          .map((text, p) => ({ p, text }))
          .filter((x) => x.p >= (c.from ?? 0) && x.p <= (c.to ?? doc.length)),
      };
    else if (c.cmd === 'insertText') {
      const p = selection.anchor.p;
      const b = enc.encode(doc[p]!);
      const lo = Math.min(selection.anchor.o, selection.focus.o);
      const hi = Math.max(selection.anchor.o, selection.focus.o);
      doc[p] = dec.decode(b.slice(0, lo)) + c.text + dec.decode(b.slice(hi));
      const o = lo + enc.encode(c.text).length;
      selection = { anchor: { p, o }, focus: { p, o } };
    }
    return r;
  };
  /** Caret before `needle` + `delta` UTF-16 units. */
  const caret = (p: number, needle: string, delta = 0) => {
    const t = doc[p]!;
    const o = enc.encode(t.slice(0, t.indexOf(needle) + delta)).length;
    selection = { anchor: { p, o }, focus: { p, o } };
  };
  return { run, log, doc, caret, sel: () => selection };
}

describe('BlueDoc word at the caret', () => {
  it('finds the word at the caret and replaces it with one insertText', async () => {
    const d = fakeDoc(['Tôi rất vui. She is Happy today.']);
    // Bytes: 'Tôi rất vui. She is ' is 23 bytes (ô 2, ấ 3).
    d.caret(0, 'Happy', 2);
    expect(await docWordAtCaret(d.run)).toEqual({ p: 0, start: 23, end: 28, word: 'Happy' });
    expect(await docReplaceWord(d.run, 'Happy', 'Glad')).toBe(true);
    expect(d.doc[0]).toBe('Tôi rất vui. She is Glad today.');
    expect(d.log.filter((c) => c.cmd === 'insertText')).toEqual([
      { cmd: 'insertText', text: 'Glad' },
    ]);
    // The caret is after the new word: it is the word for a next replacement.
    expect((await docWordAtCaret(d.run))?.word).toBe('Glad');
    // A word that is no longer there is not replaced.
    expect(await docReplaceWord(d.run, 'Happy', 'Joyful')).toBe(false);
  });

  it('takes a one-word selection, not a longer one', async () => {
    const d = fakeDoc(['big red car']);
    await d.run({
      cmd: 'setSelection',
      selection: { anchor: { p: 0, o: 4 }, focus: { p: 0, o: 7 } },
    });
    expect((await docWordAtCaret(d.run))?.word).toBe('red');
    await d.run({
      cmd: 'setSelection',
      selection: { anchor: { p: 0, o: 0 }, focus: { p: 0, o: 7 } },
    });
    expect(await docWordAtCaret(d.run)).toBeNull();
    d.caret(0, ' red');
    expect((await docWordAtCaret(d.run))?.word).toBe('big');
  });
});

describe('thesaurus menu entry', () => {
  const t = (k: string) => k;
  it('lists the top synonyms in the capitalisation of the word, then the pane', async () => {
    const r = await provider().lookup('Ran');
    const picked: [string, string][] = [];
    let opened = 0;
    const e = thesaurusMenuEntry(
      { word: 'Ran', result: r },
      t,
      (text, word) => picked.push([text, word]),
      () => opened++,
    )!;
    expect(e.label).toBe('thesaurus.menu');
    const items = (e.submenu ?? []).filter((l): l is MenuEntry => 'onSelect' in l);
    expect(items.map((x) => x.label)).toEqual(['Sprint', 'Dash', 'Race', 'thesaurus.openPane']);
    items[0]!.onSelect();
    items[3]!.onSelect();
    expect(picked).toEqual([['Sprint', 'Ran']]);
    expect(opened).toBe(1);
  });

  it('says it is searching, or that there is nothing', () => {
    expect(
      thesaurusMenuEntry(
        null,
        t,
        () => {},
        () => {},
      ),
    ).toBeNull();
    const busy = thesaurusMenuEntry(
      { word: 'x', result: undefined },
      t,
      () => {},
      () => {},
    );
    expect(busy?.submenu?.[0]).toEqual({ heading: 'thesaurus.searching' });
    const none = thesaurusMenuEntry(
      { word: 'x', result: null },
      t,
      () => {},
      () => {},
    );
    expect(none?.submenu?.[0]).toMatchObject({ heading: 'thesaurus.noSynonyms' });
  });
});

describe('thesaurus pane', () => {
  const i18n = createI18n('vi-VN');

  function Harness({
    p,
    onReady,
  }: {
    p: LazyThesaurus | undefined;
    onReady: (t: Thesaurus) => void;
  }) {
    const th = useThesaurus(p);
    useEffect(() => {
      onReady(th);
    });
    return <>{th.content}</>;
  }

  function mount(p: LazyThesaurus | undefined) {
    let th: Thesaurus | null = null;
    render(
      <I18nextProvider i18n={i18n}>
        <Harness p={p} onReady={(x) => (th = x)} />
      </I18nextProvider>,
    );
    return { open: (o: ThesaurusOpen) => act(() => th!.open(o)) };
  }

  it('groups senses by part of speech and replaces in the capitalisation of the word', async () => {
    const replaced: [string, string][] = [];
    const m = mount(provider());
    m.open({
      target: {
        word: 'Ran',
        replace: (text, word) => {
          replaced.push([text, word]);
          return true;
        },
      },
    });
    await screen.findByTestId('thesaurus-results');
    const groups = screen.getAllByTestId('thesaurus-group');
    expect(groups.map((g) => g.dataset.pos)).toEqual(['n', 'v']);
    expect(groups.map((g) => g.querySelector('h3')?.textContent)).toEqual(['Danh từ', 'Động từ']);
    expect(screen.getByTestId('thesaurus-lemma').textContent).toBe('Dạng gốc: run');
    expect((screen.getByTestId('thesaurus-search') as HTMLInputElement).value).toBe('Ran');
    expect(screen.getByTestId('thesaurus-credit').textContent).toContain('Open English WordNet');
    const replace = screen.getByTestId('thesaurus-replace') as HTMLButtonElement;
    expect(replace.disabled).toBe(true);
    fireEvent.click(screen.getByText('dash'));
    expect(replace.disabled).toBe(false);
    fireEvent.click(replace);
    await waitFor(() => expect(replaced).toEqual([['Dash', 'Ran']]));
    expect(screen.getByTestId('thesaurus-notice').textContent).toBe('Đã thay “Ran” bằng “Dash”.');
    // The next replacement applies to the new word.
    fireEvent.click(screen.getByText('sprint'));
    fireEvent.click(replace);
    await waitFor(() => expect(replaced.at(-1)).toEqual(['Sprint', 'Dash']));
  });

  it('searches another word, shows antonyms and says when nothing is found', async () => {
    const m = mount(provider());
    m.open({ target: { word: '' } });
    expect(screen.queryByTestId('thesaurus-replace')).toBeNull();
    const box = screen.getByTestId('thesaurus-search');
    fireEvent.change(box, { target: { value: 'happy' } });
    fireEvent.submit(box.closest('form')!);
    await screen.findByTestId('thesaurus-results');
    expect(screen.getAllByTestId('thesaurus-synonym').map((b) => b.textContent)).toEqual([
      'glad',
      'joyful',
    ]);
    expect(screen.getAllByTestId('thesaurus-antonym').map((b) => b.textContent)).toEqual([
      'unhappy',
    ]);
    // Looking up the chosen word.
    fireEvent.click(screen.getByText('glad'));
    fireEvent.click(screen.getByTestId('thesaurus-lookup-chosen'));
    const none = await screen.findByTestId('thesaurus-none');
    expect(none.textContent).toContain('“glad”');
    expect((box as HTMLInputElement).value).toBe('glad');
  });

  it('says so when the platform has no thesaurus', () => {
    const m = mount(undefined);
    m.open({ target: { word: 'happy' } });
    expect(screen.getByTestId('thesaurus-unavailable').textContent).toBe(
      'Nền tảng này không có từ điển đồng nghĩa.',
    );
  });
});
