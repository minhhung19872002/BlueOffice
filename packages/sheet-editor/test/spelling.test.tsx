// @vitest-environment happy-dom
import { createI18n, I18nextProvider } from '@blueoffice/i18n';
import type { CellIssue, SheetCommand, SheetHost, TextIssue } from '@blueoffice/platform';
import { act, cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { createRef, type ReactNode } from 'react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { SheetEditSpelling, SheetSpellDialog, loadSheetLang, saveSheetLang } from '../src/spelling';

afterEach(cleanup);
beforeEach(() => localStorage.clear());

const i18n = createI18n('en-US');
const wrap = (n: ReactNode) => render(<I18nextProvider i18n={i18n}>{n}</I18nextProvider>);

const utf8 = (s: string) => new TextEncoder().encode(s).length;

/** An issue of `text` at the first occurrence of `word` (offsets worked out from the strings). */
function issueOf(text: string, word: string, suggestions: string[]): TextIssue {
  const from = text.indexOf(word);
  const start = utf8(text.slice(0, from));
  return {
    start,
    end: start + utf8(word),
    from,
    to: from + word.length,
    word,
    suggestions,
    before: text.slice(0, from),
    after: text.slice(from + word.length),
  };
}

/** A fake sheet host holding cells; `spellReplace` applies corrections to them. */
function fakeHost(cells: Map<string, string>) {
  const sent: SheetCommand[] = [];
  const check = (lang?: string): CellIssue[] => {
    const out: CellIssue[] = [];
    for (const [k, text] of cells) {
      const [row, col] = k.split(':').map(Number) as [number, number];
      for (const w of ['documnet', 'thág']) {
        if (text.includes(w) && (w !== 'thág' || lang !== 'en-US'))
          out.push({
            ...issueOf(text, w, w === 'documnet' ? ['document'] : ['tháng']),
            sheet: 0,
            sheetName: 'Sheet1',
            row,
            col,
          });
      }
    }
    return out;
  };
  const exec = vi.fn(async (c: SheetCommand): Promise<unknown> => {
    sent.push(c);
    switch (c.cmd) {
      case 'spellCheck':
        return { issues: check(c.lang), unchecked: [], languages: ['vi', 'en'] };
      case 'spellText':
        return {
          issues: c.text.includes('documnet') ? [issueOf(c.text, 'documnet', ['document'])] : [],
        };
      case 'spellReplace':
        for (const e of [...c.edits].sort((a, b) => b.start - a.start)) {
          const k = `${e.row}:${e.col}`;
          const bytes = new TextEncoder().encode(cells.get(k) ?? '');
          const dec = new TextDecoder();
          cells.set(
            k,
            dec.decode(bytes.slice(0, e.start)) + e.text + dec.decode(bytes.slice(e.end)),
          );
        }
        return { changed: c.edits.length };
      default:
        return { ok: true };
    }
  });
  return { host: { exec } as unknown as SheetHost, sent };
}

describe('BlueSheet spelling', () => {
  it('keeps the proofing language per workbook', () => {
    expect(loadSheetLang('a.xlsx', 'vi-VN')).toBe('vi-VN');
    saveSheetLang('a.xlsx', 'en-US');
    expect(loadSheetLang('a.xlsx', 'vi-VN')).toBe('en-US');
    expect(loadSheetLang('b.xlsx', 'vi-VN')).toBe('vi-VN');
  });

  it('walks the text cells, replaces one and replaces all in one command', async () => {
    const cells = new Map([
      ['0:0', 'Doanh thu thág'],
      ['1:0', 'the documnet'],
      ['2:1', 'documnet again'],
    ]);
    const { host, sent } = fakeHost(cells);
    const onShow = vi.fn();
    const onChanged = vi.fn();
    wrap(
      <SheetSpellDialog
        host={host}
        lang="vi-VN"
        setLang={() => {}}
        onClose={() => {}}
        onShow={onShow}
        onChanged={onChanged}
      />,
    );
    await waitFor(() => expect(screen.getByTestId('spellcheck-word').textContent).toBe('thág'));
    expect(screen.getByTestId('spellcheck-where').textContent).toBe('Sheet Sheet1, cell A1');
    expect(onShow).toHaveBeenCalledWith(0, 0, 0);
    fireEvent.click(screen.getByTestId('spellcheck-change'));
    await waitFor(() => expect(cells.get('0:0')).toBe('Doanh thu tháng'));
    await waitFor(() => expect(screen.getByTestId('spellcheck-word').textContent).toBe('documnet'));
    expect(screen.getByTestId('spellcheck-where').textContent).toBe('Sheet Sheet1, cell A2');
    fireEvent.click(screen.getByTestId('spellcheck-change-all'));
    await waitFor(() => expect(screen.getByTestId('spellcheck-done')).toBeTruthy());
    expect(cells.get('1:0')).toBe('the document');
    expect(cells.get('2:1')).toBe('document again');
    // Replace all is one engine edit (one undo step) covering both cells.
    const replaces = sent.filter((c) => c.cmd === 'spellReplace');
    expect(replaces).toHaveLength(2);
    expect(replaces[1]).toMatchObject({
      edits: [
        { sheet: 0, row: 1, col: 0, start: 4, end: 12, text: 'document', word: 'documnet' },
        { sheet: 0, row: 2, col: 1, start: 0, end: 8, text: 'document', word: 'documnet' },
      ],
    });
    expect(onChanged).toHaveBeenCalledTimes(2);
  });

  it('checks again in the language picked in the dialog', async () => {
    const { host, sent } = fakeHost(new Map([['0:0', 'Doanh thu thág']]));
    const setLang = vi.fn();
    const view = wrap(
      <SheetSpellDialog
        host={host}
        lang="vi-VN"
        setLang={setLang}
        onClose={() => {}}
        onShow={() => {}}
        onChanged={() => {}}
      />,
    );
    await waitFor(() => screen.getByTestId('spellcheck-word'));
    fireEvent.change(screen.getByTestId('spellcheck-sheet-language'), {
      target: { value: 'en-US' },
    });
    expect(setLang).toHaveBeenCalledWith('en-US');
    view.rerender(
      <I18nextProvider i18n={i18n}>
        <SheetSpellDialog
          host={host}
          lang="en-US"
          setLang={setLang}
          onClose={() => {}}
          onShow={() => {}}
          onChanged={() => {}}
        />
      </I18nextProvider>,
    );
    await waitFor(() => expect(screen.getByTestId('spellcheck-done')).toBeTruthy());
    expect(sent.filter((c) => c.cmd === 'spellCheck').map((c) => 'lang' in c && c.lang)).toEqual([
      'vi-VN',
      'en-US',
    ]);
  });

  it('underlines the typed text and corrects it from the menu', async () => {
    const { host, sent } = fakeHost(new Map());
    const ref = createRef<HTMLInputElement>();
    const typed: string[] = [];
    function Harness({ text }: { text: string }) {
      return (
        <>
          <input ref={ref} defaultValue={text} onInput={(e) => typed.push(e.currentTarget.value)} />
          <SheetEditSpelling host={host} inputs={[ref]} text={text} lang="en-US" enabled />
        </>
      );
    }
    vi.useFakeTimers({ shouldAdvanceTime: true });
    try {
      wrap(<Harness text="the documnet" />);
      // happy-dom lays nothing out: give the input and the mark a box.
      const box = { left: 10, top: 10, right: 200, bottom: 30, width: 190, height: 20 };
      ref.current!.getBoundingClientRect = () => ({ ...box, x: 10, y: 10 }) as DOMRect;
      await act(async () => {
        await vi.advanceTimersByTimeAsync(250);
      });
      const mark = await screen.findByTestId('cell-spell-mark');
      expect(mark.textContent).toBe('documnet');
      expect(sent).toContainEqual({ cmd: 'spellText', text: 'the documnet', lang: 'en-US' });
      mark.getBoundingClientRect = () =>
        ({
          left: 40,
          top: 12,
          right: 100,
          bottom: 28,
          width: 60,
          height: 16,
          x: 40,
          y: 12,
        }) as DOMRect;
      fireEvent.contextMenu(ref.current!, { clientX: 60, clientY: 20 });
      const menu = await screen.findByTestId('spell-menu');
      fireEvent.click(menu.querySelector('button')!);
      expect(typed).toEqual(['the document']);
    } finally {
      vi.useRealTimers();
    }
  });
});
