// @vitest-environment happy-dom
import { createI18n, I18nextProvider } from '@blueoffice/i18n';
import type { EditCommand, EditResult } from '@blueoffice/platform';
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import type { ReactNode } from 'react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import {
  AuthoritiesDialog,
  ConvertNotesDialog,
  figuresInstr,
  IndexDialog,
  indexInstr,
  isReferenceDialog,
  MarkCitationDialog,
  MarkEntryDialog,
  TableOfFiguresDialog,
  toaInstr,
} from '../src/ReferenceDialogs';

afterEach(cleanup);

const i18n = createI18n('en-US');
const wrap = (n: ReactNode) => render(<I18nextProvider i18n={i18n}>{n}</I18nextProvider>);

/** A `run` that answers `copy` with `selected` and `crossReferenceTargets` with a bookmark;
 * the other commands are recorded. */
function engine(selected = '') {
  const sent: EditCommand[] = [];
  const run = vi.fn(async (c: EditCommand) => {
    if (c.cmd === 'copy') return { clip: { text: selected, html: '', internal: '' } } as EditResult;
    if (c.cmd === 'crossReferenceTargets')
      return {
        targets: [
          { id: 'dieu5', label: 'Điều 5' },
          { id: 'caption:3', label: 'Hình 1' },
        ],
      } as EditResult;
    sent.push(c);
    return {} as EditResult;
  });
  return { run, sent };
}

describe('field instructions (ECMA-376 Part 1, §17.16.5)', () => {
  it('table of figures: with or without label, page numbers, links', () => {
    const o = {
      label: 'Hình',
      includeLabel: true,
      pageNumbers: true,
      rightAlign: true,
      hyperlinks: true,
    };
    expect(figuresInstr(o)).toBe('TOC \\h \\z \\c "Hình"');
    expect(figuresInstr({ ...o, includeLabel: false, hyperlinks: false })).toBe('TOC \\a "Hình"');
    expect(figuresInstr({ ...o, pageNumbers: false })).toBe('TOC \\h \\z \\c "Hình" \\n');
    expect(figuresInstr({ ...o, rightAlign: false })).toBe('TOC \\h \\z \\c "Hình" \\p " "');
    // A quote cannot end the argument early.
    expect(figuresInstr({ ...o, label: 'Sơ "đồ"' })).toBe('TOC \\h \\z \\c "Sơ \'đồ\'"');
  });

  it('index: headings, columns, run-in, tab, language', () => {
    const o = {
      columns: 2,
      runIn: false,
      rightAlign: true,
      headings: true,
      language: 'vi' as const,
    };
    expect(indexInstr(o)).toBe('INDEX \\h "A" \\c "2" \\e "\t" \\z "1066"');
    // Run-in entries share a line: no tab separator.
    expect(indexInstr({ ...o, runIn: true, headings: false, columns: 9, language: 'en' })).toBe(
      'INDEX \\c "4" \\r \\z "1033"',
    );
  });

  it('table of authorities: category, passim, headings', () => {
    expect(toaInstr({ category: 0, passim: true, headings: true, rightAlign: true })).toBe(
      'TOA \\h \\c "0" \\p \\e "\t"',
    );
    expect(toaInstr({ category: 2, passim: false, headings: false, rightAlign: false })).toBe(
      'TOA \\c "2"',
    );
  });

  it('knows its dialogs', () => {
    expect(isReferenceDialog('insertIndex')).toBe(true);
    expect(isReferenceDialog('columns')).toBe(false);
    expect(isReferenceDialog(null)).toBe(false);
  });
});

describe('dialogs', () => {
  it('inserts a table of figures without labels and with dashes', async () => {
    const { run, sent } = engine();
    const onClose = vi.fn();
    wrap(<TableOfFiguresDialog run={run} onClose={onClose} />);
    fireEvent.change(screen.getByTestId('figures-label'), { target: { value: 'Table' } });
    fireEvent.click(screen.getByTestId('figures-include-label'));
    fireEvent.change(screen.getByTestId('figures-leader'), { target: { value: 'hyphen' } });
    fireEvent.click(screen.getByTestId('figures-ok'));
    await waitFor(() => expect(sent).toHaveLength(1));
    expect(sent[0]).toEqual({
      cmd: 'insertReferenceTable',
      instr: 'TOC \\h \\z \\a "Table"',
      leader: 'hyphen',
    });
    expect(onClose).toHaveBeenCalled();
  });

  it('marks the selected text as a sub-entry with a bold page number, everywhere', async () => {
    const { run, sent } = engine('Hợp đồng');
    wrap(<MarkEntryDialog run={run} onClose={() => {}} />);
    await waitFor(() =>
      expect((screen.getByTestId('entry-main') as HTMLInputElement).value).toBe('Hợp đồng'),
    );
    fireEvent.change(screen.getByTestId('entry-sub'), { target: { value: 'mua bán' } });
    fireEvent.click(screen.getByTestId('entry-bold'));
    fireEvent.click(screen.getByTestId('mark-entry-all'));
    await waitFor(() => expect(sent).toHaveLength(1));
    expect(sent[0]).toEqual({
      cmd: 'markIndexEntry',
      mark: {
        main: 'Hợp đồng',
        sub: 'mua bán',
        crossRef: '',
        bookmark: '',
        bold: true,
        italic: false,
      },
      all: true,
    });
  });

  it('marks a cross-reference or a bookmarked page range', async () => {
    const { run, sent } = engine('Giao kết');
    wrap(<MarkEntryDialog run={run} onClose={() => {}} />);
    await waitFor(() =>
      expect((screen.getByTestId('entry-main') as HTMLInputElement).value).toBe('Giao kết'),
    );
    fireEvent.click(screen.getByTestId('entry-kind-crossRef'));
    fireEvent.change(screen.getByTestId('entry-see'), { target: { value: 'See Contract' } });
    fireEvent.click(screen.getByTestId('mark-entry-ok'));
    await waitFor(() => expect(sent).toHaveLength(1));
    expect(sent[0]).toMatchObject({ mark: { crossRef: 'See Contract', bookmark: '' }, all: false });
    cleanup();

    wrap(<MarkEntryDialog run={run} onClose={() => {}} />);
    // Captions are not bookmarks: only `dieu5` is offered.
    await waitFor(() =>
      expect(screen.getByTestId('entry-bookmark').querySelectorAll('option')).toHaveLength(1),
    );
    fireEvent.click(screen.getByTestId('entry-kind-range'));
    fireEvent.click(screen.getByTestId('mark-entry-ok'));
    await waitFor(() => expect(sent).toHaveLength(2));
    expect(sent[1]).toMatchObject({ mark: { main: 'Giao kết', bookmark: 'dieu5', crossRef: '' } });
  });

  it('does not mark an empty entry', async () => {
    const { run, sent } = engine('');
    wrap(<MarkEntryDialog run={run} onClose={() => {}} />);
    fireEvent.click(screen.getByTestId('mark-entry-ok'));
    await new Promise((r) => setTimeout(r, 10));
    expect(sent).toHaveLength(0);
  });

  it('inserts a run-in index in two columns sorted in English', async () => {
    const { run, sent } = engine();
    wrap(<IndexDialog run={run} language="vi-VN" onClose={() => {}} />);
    expect((screen.getByTestId('index-language') as HTMLSelectElement).value).toBe('vi');
    fireEvent.change(screen.getByTestId('index-type'), { target: { value: 'runIn' } });
    fireEvent.change(screen.getByTestId('index-language'), { target: { value: 'en' } });
    fireEvent.click(screen.getByTestId('index-ok'));
    await waitFor(() => expect(sent).toHaveLength(1));
    expect(sent[0]).toEqual({
      cmd: 'insertReferenceTable',
      instr: 'INDEX \\h "A" \\c "2" \\r \\z "1033"',
      leader: undefined,
      columns: 2,
    });
  });

  it('marks a citation in a category', async () => {
    const { run, sent } = engine('Bộ luật Dân sự 2015');
    wrap(<MarkCitationDialog run={run} onClose={() => {}} />);
    await waitFor(() =>
      expect((screen.getByTestId('citation-long') as HTMLInputElement).value).toBe(
        'Bộ luật Dân sự 2015',
      ),
    );
    const cat = screen.getByTestId('citation-category') as HTMLSelectElement;
    // 16 categories, the first seven named.
    expect(cat.querySelectorAll('option')).toHaveLength(16);
    expect(cat.querySelectorAll('option')[1]!.textContent).toBe('Statutes and codes');
    expect(cat.querySelectorAll('option')[8]!.textContent).toBe('Category 9');
    fireEvent.change(screen.getByTestId('citation-short'), { target: { value: 'BLDS' } });
    fireEvent.change(cat, { target: { value: '2' } });
    fireEvent.click(screen.getByTestId('mark-citation-all'));
    await waitFor(() => expect(sent).toHaveLength(1));
    expect(sent[0]).toEqual({
      cmd: 'markCitation',
      mark: { long: 'Bộ luật Dân sự 2015', short: 'BLDS', category: 2 },
      all: true,
    });
  });

  it('inserts a table of authorities for one category without passim', async () => {
    const { run, sent } = engine();
    wrap(<AuthoritiesDialog run={run} onClose={() => {}} />);
    fireEvent.change(screen.getByTestId('toa-category'), { target: { value: '1' } });
    fireEvent.click(screen.getByTestId('toa-passim'));
    fireEvent.change(screen.getByTestId('toa-leader'), { target: { value: 'none' } });
    fireEvent.click(screen.getByTestId('toa-ok'));
    await waitFor(() => expect(sent).toHaveLength(1));
    expect(sent[0]).toEqual({
      cmd: 'insertReferenceTable',
      instr: 'TOA \\h \\c "1" \\e "\t"',
      leader: 'none',
    });
  });

  it('converts the endnotes of the selection to footnotes', async () => {
    const { run, sent } = engine();
    wrap(<ConvertNotesDialog run={run} onClose={() => {}} />);
    fireEvent.click(screen.getByTestId('convert-toFootnotes'));
    fireEvent.change(screen.getByTestId('convert-scope'), { target: { value: 'selection' } });
    fireEvent.click(screen.getByTestId('convert-ok'));
    await waitFor(() => expect(sent).toHaveLength(1));
    expect(sent[0]).toEqual({ cmd: 'convertNotes', mode: 'toFootnotes', selection: true });
  });
});
