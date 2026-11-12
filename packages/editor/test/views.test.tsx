// @vitest-environment happy-dom
import { createI18n, I18nextProvider } from '@blueoffice/i18n';
import type { EditCommand, EditResult, OutlineItem } from '@blueoffice/platform';
import { act, cleanup, fireEvent, render, screen } from '@testing-library/react';
import type { ReactNode } from 'react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { CommandRegistry } from '../src/commands';
import { keyToCommand } from '../src/keys';
import { NavigationPane, type NavigationPaneProps } from '../src/NavigationPane';
import { OutlineView } from '../src/OutlineView';
import {
  demoted,
  hasChildren,
  headingAt,
  headingsOf,
  promoted,
  visibleItems,
} from '../src/outlineModel';
import { ReadMode, readColumn, spreadSize, spreadStart } from '../src/ReadMode';

afterEach(cleanup);

const i18n = createI18n('en-US');
const wrap = (n: ReactNode) => render(<I18nextProvider i18n={i18n}>{n}</I18nextProvider>);

const item = (para: number, level: number | null, text: string, page = 0): OutlineItem => ({
  para,
  level,
  text,
  section: 0,
  page,
});

/** H1 Intro, body, H2 Scope, body, H1 Results, body. */
const outline: OutlineItem[] = [
  item(0, 0, 'Intro'),
  item(1, null, 'Some words'),
  item(2, 1, 'Scope'),
  item(3, null, 'More words'),
  item(4, 0, 'Results', 1),
  item(5, null, 'Numbers', 1),
];

const runner = () => {
  const calls: EditCommand[] = [];
  const run = vi.fn((c: EditCommand) => {
    calls.push(c);
    return Promise.resolve(null as EditResult | null);
  });
  return { run, calls };
};

describe('outline model', () => {
  it('collapses headings and filters by level', () => {
    expect(hasChildren(outline, 0)).toBe(true);
    expect(hasChildren(outline, 1)).toBe(false);
    expect(visibleItems(outline, new Set([0]), null).map((i) => i.para)).toEqual([0, 4, 5]);
    expect(visibleItems(outline, new Set(), 1).map((i) => i.para)).toEqual([0, 4]);
    expect(visibleItems(outline, new Set(), 2).map((i) => i.para)).toEqual([0, 2, 4]);
    expect(headingsOf(outline).map((i) => i.text)).toEqual(['Intro', 'Scope', 'Results']);
    expect(headingAt(outline, 3)?.text).toBe('Scope');
  });

  it('computes promoted and demoted levels', () => {
    expect(promoted(outline, 2)).toBe(0);
    expect(promoted(outline, 0)).toBeUndefined();
    // Body text is promoted to the level of the heading above it.
    expect(promoted(outline, 3)).toBe(1);
    expect(demoted(outline, 0)).toBe(1);
    expect(demoted(outline, 1)).toBeUndefined();
    expect(demoted([item(0, 8, 'deep')], 0)).toBeNull();
  });

  it('keeps Alt+Shift+arrows for the outline commands', () => {
    expect(keyToCommand({ key: 'ArrowUp', ctrl: false, shift: true, alt: true })).toBeNull();
    expect(keyToCommand({ key: 'ArrowUp', ctrl: false, shift: true, alt: false })).toEqual({
      cmd: 'move',
      motion: 'up',
      extend: true,
    });
    const r = new CommandRegistry<object>();
    r.register({ id: 'outline.moveUp', shortcut: 'Alt+Shift+ArrowUp', run: () => {} });
    r.register({ id: 'view.formattingMarks', shortcut: 'Ctrl+Shift+8', run: () => {} });
    const base = { ctrlKey: false, metaKey: false, shiftKey: true, altKey: true };
    expect(r.match({ ...base, key: 'ArrowUp' })?.id).toBe('outline.moveUp');
    // Ctrl+Shift+8 types "*" on most layouts: matched by the physical key.
    expect(
      r.match({
        key: '*',
        code: 'Digit8',
        ctrlKey: true,
        metaKey: false,
        shiftKey: true,
        altKey: false,
      })?.id,
    ).toBe('view.formattingMarks');
  });
});

describe('OutlineView', () => {
  it('lists paragraphs by level, collapses, filters and selects', () => {
    const { run, calls } = runner();
    const onCommand = vi.fn();
    const onClose = vi.fn();
    wrap(
      <OutlineView
        items={outline}
        caretPara={2}
        run={run}
        onCommand={onCommand}
        onClose={onClose}
      />,
    );
    const rows = () => screen.getAllByTestId('outline-item');
    expect(rows()).toHaveLength(6);
    expect(rows()[2]?.getAttribute('aria-selected')).toBe('true');
    expect(rows()[0]?.getAttribute('data-level')).toBe('0');
    expect(rows()[1]?.getAttribute('data-level')).toBe('body');
    // Collapse "Intro": its subordinate rows go.
    fireEvent.click(screen.getAllByRole('button', { name: 'Hide subheadings' })[0]!);
    expect(rows()).toHaveLength(3);
    fireEvent.click(screen.getByRole('button', { name: 'Show subheadings' }));
    // Show level 1 only.
    fireEvent.change(screen.getByTestId('outline-show-level'), { target: { value: '1' } });
    expect(rows().map((r) => r.textContent)).toEqual(['⊕Intro', '⊕Results']);
    fireEvent.click(rows()[1]!);
    expect(calls.at(-1)).toEqual({
      cmd: 'setSelection',
      selection: { anchor: { p: 4, o: 0 }, focus: { p: 4, o: 0 } },
    });
    // Tab lowers, Shift+Tab raises the level (editor commands).
    const list = screen.getByTestId('outline-list');
    fireEvent.keyDown(list, { key: 'Tab' });
    fireEvent.keyDown(list, { key: 'Tab', shiftKey: true });
    expect(onCommand.mock.calls.map((c) => c[0])).toEqual(['outline.demote', 'outline.promote']);
    fireEvent.click(screen.getByTestId('outline.moveUp'));
    expect(onCommand).toHaveBeenLastCalledWith('outline.moveUp');
    fireEvent.doubleClick(rows()[0]!);
    expect(onClose).toHaveBeenCalledWith(0);
  });

  it('moves a heading with its content by drag and drop', () => {
    const { run, calls } = runner();
    wrap(
      <OutlineView
        items={outline}
        caretPara={0}
        run={run}
        onCommand={() => {}}
        onClose={() => {}}
      />,
    );
    const data = new Map<string, string>();
    const dataTransfer = {
      setData: (k: string, v: string) => data.set(k, v),
      getData: (k: string) => data.get(k) ?? '',
      effectAllowed: '',
    };
    const rows = screen.getAllByTestId('outline-item');
    fireEvent.dragStart(rows[4]!, { dataTransfer });
    fireEvent.dragOver(rows[0]!, { dataTransfer });
    fireEvent.drop(rows[0]!, { dataTransfer });
    expect(calls.at(-1)).toEqual({ cmd: 'moveOutlineTo', para: 4, before: 0 });
    fireEvent.dragStart(rows[0]!, { dataTransfer });
    fireEvent.drop(screen.getByTestId('outline-drop-end'), { dataTransfer });
    expect(calls.at(-1)).toEqual({ cmd: 'moveOutlineTo', para: 0, before: null });
  });
});

describe('NavigationPane', () => {
  const props = (over: Partial<NavigationPaneProps> = {}): NavigationPaneProps => ({
    tab: 'headings',
    onTab: () => {},
    outline,
    caretPara: 3,
    page: 1,
    pageCount: 3,
    layoutVersion: 0,
    renderThumbnail: () => Promise.resolve(),
    goToPage: () => {},
    run: runner().run,
    onClearHighlights: () => {},
    onClose: () => {},
    ...over,
  });

  it('shows the heading tree and goes to a heading', () => {
    const { run, calls } = runner();
    wrap(<NavigationPane {...props({ run })} />);
    const heads = screen.getAllByTestId('nav-heading');
    expect(heads.map((h) => h.textContent)).toEqual(['▾Intro', 'Scope', 'Results']);
    // The caret is under "Scope".
    expect(heads[1]?.getAttribute('aria-selected')).toBe('true');
    fireEvent.click(screen.getByRole('button', { name: 'Results' }));
    expect(calls.at(-1)).toEqual({
      cmd: 'setSelection',
      selection: { anchor: { p: 4, o: 0 }, focus: { p: 4, o: 0 } },
    });
    fireEvent.click(screen.getByRole('button', { name: 'Hide subheadings' }));
    expect(screen.getAllByTestId('nav-heading')).toHaveLength(2);
  });

  it('draws page thumbnails and goes to a page', () => {
    // Without IntersectionObserver every thumbnail is drawn at once.
    vi.stubGlobal('IntersectionObserver', undefined);
    const renderThumbnail = vi.fn(() => Promise.resolve());
    const goToPage = vi.fn();
    const onTab = vi.fn();
    wrap(<NavigationPane {...props({ tab: 'pages', renderThumbnail, goToPage, onTab })} />);
    const thumbs = screen.getAllByTestId('nav-thumbnail');
    expect(thumbs).toHaveLength(3);
    expect(thumbs[1]?.getAttribute('aria-current')).toBe('page');
    expect(renderThumbnail).toHaveBeenCalledTimes(3);
    fireEvent.click(thumbs[2]!);
    expect(goToPage).toHaveBeenCalledWith(2);
    fireEvent.click(screen.getByTestId('nav-tab-results'));
    expect(onTab).toHaveBeenCalledWith('results');
    vi.unstubAllGlobals();
  });

  it('lists find results with context', async () => {
    vi.useFakeTimers();
    const calls: EditCommand[] = [];
    const run = vi.fn((c: EditCommand) => {
      calls.push(c);
      return Promise.resolve(
        c.cmd === 'findResults'
          ? ({
              results: [
                {
                  match: { p: 3, start: 5, end: 10 },
                  before: 'More ',
                  text: 'words',
                  after: '',
                  page: 0,
                },
                {
                  match: { p: 5, start: 0, end: 5 },
                  before: '',
                  text: 'Numb',
                  after: 'ers',
                  page: 1,
                },
              ],
            } as unknown as EditResult)
          : null,
      );
    });
    const goToPage = vi.fn();
    wrap(<NavigationPane {...props({ tab: 'results', run, goToPage })} />);
    fireEvent.change(screen.getByTestId('nav-search'), { target: { value: 'wor' } });
    await act(async () => {
      await vi.advanceTimersByTimeAsync(250);
    });
    vi.useRealTimers();
    expect(calls[0]).toEqual({ cmd: 'findResults', query: 'wor' });
    const res = screen.getAllByTestId('nav-result');
    expect(res).toHaveLength(2);
    expect(res[0]?.querySelector('mark')?.textContent).toBe('words');
    expect(screen.getByTestId('nav-result-count').textContent).toBe('2 matches');
    fireEvent.click(res[1]!);
    expect(goToPage).toHaveBeenCalledWith(1);
    expect(calls.at(-1)).toEqual({ cmd: 'selectMatch', match: { p: 5, start: 0, end: 5 } });
  });
});

describe('ReadMode', () => {
  it('pairs pages on wide screens and turns them with the keyboard', () => {
    const a4 = { width: 595, height: 842 };
    expect(spreadSize(1600, 900, a4)).toBe(2);
    expect(spreadSize(700, 900, a4)).toBe(1);
    expect(spreadStart(5, 2)).toBe(4);
    const onExit = vi.fn();
    const renderPage = vi.fn(() => Promise.resolve());
    wrap(
      <ReadMode
        name="a.docx"
        pages={[a4, a4, a4]}
        start={0}
        renderPage={renderPage}
        onExit={onExit}
      />,
    );
    const pos = () => screen.getByTestId('read-position').textContent;
    const first = pos();
    fireEvent.keyDown(window, { key: 'PageDown' });
    expect(pos()).not.toBe(first);
    fireEvent.keyDown(window, { key: 'Home' });
    expect(pos()).toBe(first);
    fireEvent.keyDown(window, { key: 'Escape' });
    expect(onExit).toHaveBeenCalledWith(0);
    expect(renderPage).toHaveBeenCalled();
  });

  it('reflows the text into screen columns and opens on the caret page', async () => {
    // Two columns on a wide screen (at most 720 px each), one on a narrow one; points.
    expect(readColumn(1600, 900)).toEqual({ width: 540, height: 639 });
    expect(readColumn(600, 900).width).toBe(414);
    const col = { width: 540, height: 639 };
    const reflow = vi.fn(() => Promise.resolve(2));
    wrap(
      <ReadMode
        name="a.docx"
        pages={[col, col, col, col]}
        start={0}
        renderPage={() => Promise.resolve()}
        onExit={() => {}}
        reflow={reflow}
      />,
    );
    expect(reflow).toHaveBeenCalled();
    const [w, h] = reflow.mock.calls.at(-1) as unknown as [number, number];
    expect(w).toBeGreaterThan(100);
    expect(h).toBeGreaterThan(100);
    // The caret's page (3 of 4) is on the screen.
    await screen.findByText(/3/, { selector: '[data-testid="read-position"]' });
  });
});
