// @vitest-environment happy-dom
import { CommandRegistry } from '@blueoffice/editor';
import { createI18n, I18nextProvider } from '@blueoffice/i18n';
import type {
  SlideCommand,
  SlideEditResult,
  SlideNotesPage,
  SlideOutlineSlide,
  SlidePageMasterInfo,
  SlideSection,
} from '@blueoffice/platform';
import { Ribbon } from '@blueoffice/ui';
import { act, cleanup, fireEvent, render, screen } from '@testing-library/react';
import type { ReactNode } from 'react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { NotesPageView, boxStyle } from '../src/notesPage';
import {
  demote,
  editsOf,
  enter,
  linesOf,
  newSlide,
  promote,
  removeEmpty,
  renumber,
  slideOfLine,
} from '../src/outlineModel';
import { OutlineView } from '../src/outlineView';
import {
  HandoutSlots,
  pageMasterTab,
  registerPageMasterCommands,
  type PageMasterState,
} from '../src/pageMasterView';
import {
  paneEntries,
  registerSectionCommands,
  sectionAt,
  sectionState,
  type SectionState,
} from '../src/sections';

afterEach(cleanup);

const i18n = createI18n('vi-VN');
const t = i18n.t.bind(i18n) as (key: string, o?: Record<string, unknown>) => string;
const wrap = (ui: ReactNode) => <I18nextProvider i18n={i18n}>{ui}</I18nextProvider>;

/** Hand-built deck: two slides, the second with a two-level body. */
const deck: SlideOutlineSlide[] = [
  { title: 'Mở đầu', bullets: [], hasTitle: true, hasBody: false },
  {
    title: 'Kế hoạch',
    bullets: [
      { text: 'Mục tiêu', level: 0 },
      { text: 'Doanh thu', level: 1 },
      { text: 'Chi phí', level: 0 },
    ],
    hasTitle: true,
    hasBody: true,
  },
];

describe('outline model', () => {
  it('maps slides to lines and back', () => {
    const lines = linesOf(deck);
    expect(lines.map((l) => [l.kind, l.level, l.text, l.slide])).toEqual([
      ['title', 0, 'Mở đầu', 0],
      ['title', 0, 'Kế hoạch', 1],
      ['bullet', 0, 'Mục tiêu', null],
      ['bullet', 1, 'Doanh thu', null],
      ['bullet', 0, 'Chi phí', null],
    ]);
    expect(editsOf(lines)).toEqual([
      { slide: 0, title: 'Mở đầu', bullets: [] },
      {
        slide: 1,
        title: 'Kế hoạch',
        bullets: [
          { text: 'Mục tiêu', level: 0 },
          { text: 'Doanh thu', level: 1 },
          { text: 'Chi phí', level: 0 },
        ],
      },
    ]);
    expect(slideOfLine(lines, 0)).toBe(0);
    expect(slideOfLine(lines, 3)).toBe(1);
  });

  it('Enter splits a line into a new bullet (a title gets its first bullet)', () => {
    const lines = linesOf(deck);
    const a = enter(lines, 0, 2);
    expect(a.focus).toBe(1);
    expect(editsOf(a.lines)[0]).toEqual({
      slide: 0,
      title: 'Mở',
      bullets: [{ text: ' đầu', level: 0 }],
    });
    const b = enter(lines, 3, 9);
    expect(b.lines[4]).toEqual({ kind: 'bullet', level: 1, text: '', slide: null });
  });

  it('Shift+Tab on a level-0 bullet makes a new slide; Tab on a title merges it back', () => {
    const lines = linesOf(deck);
    // "Doanh thu" level 1 → 0, then → title of a new slide; "Chi phí" follows it.
    const up = promote(promote(lines, 3)!.lines, 3)!;
    expect(editsOf(up.lines)).toEqual([
      { slide: 0, title: 'Mở đầu', bullets: [] },
      { slide: 1, title: 'Kế hoạch', bullets: [{ text: 'Mục tiêu', level: 0 }] },
      { title: 'Doanh thu', bullets: [{ text: 'Chi phí', level: 0 }] },
    ]);
    expect(promote(lines, 0)).toBeNull();
    // Demoting the title "Kế hoạch" puts it (and its bullets) under slide 1.
    const down = demote(lines, 1)!;
    expect(editsOf(down.lines)).toEqual([
      {
        slide: 0,
        title: 'Mở đầu',
        bullets: [
          { text: 'Kế hoạch', level: 0 },
          { text: 'Mục tiêu', level: 0 },
          { text: 'Doanh thu', level: 1 },
          { text: 'Chi phí', level: 0 },
        ],
      },
    ]);
    // The first title cannot be demoted; a bullet goes one level deeper (at most 8).
    expect(demote(lines, 0)).toBeNull();
    expect(demote(lines, 2)!.lines[2]!.level).toBe(1);
    const deep = linesOf([
      { title: 'x', bullets: [{ text: 'y', level: 8 }], hasTitle: true, hasBody: true },
    ]);
    expect(demote(deep, 1)).toBeNull();
  });

  it('Ctrl+Enter adds a slide after the current one; Backspace removes empty lines', () => {
    const lines = linesOf(deck);
    const n = newSlide(lines, 0);
    expect(n.focus).toBe(1);
    expect(editsOf(n.lines).map((e) => e.slide)).toEqual([0, undefined, 1]);
    // The deck after applying: slide k is the k-th title.
    expect(
      renumber(n.lines)
        .filter((l) => l.kind === 'title')
        .map((l) => l.slide),
    ).toEqual([0, 1, 2]);
    const r = removeEmpty(n.lines, 1)!;
    expect(r.focus).toBe(0);
    expect(r.caret).toBe('Mở đầu'.length);
    expect(editsOf(r.lines)).toEqual(editsOf(lines));
    expect(removeEmpty(lines, 0)).toBeNull();
    expect(removeEmpty(lines, 2)).toBeNull();
  });
});

describe('OutlineView', () => {
  it('sends Enter, Shift+Tab and text edits as applyOutline', async () => {
    const sent: SlideCommand[] = [];
    const runResult = vi.fn(async (c: SlideCommand) => {
      sent.push(c);
      return { outline: deck } as unknown as SlideEditResult;
    });
    const onSelect = vi.fn();
    render(
      wrap(<OutlineView runResult={runResult} revision={1} current={0} onSelect={onSelect} />),
    );
    const first = await screen.findByTestId('outline-line-0');
    expect((first as HTMLInputElement).value).toBe('Mở đầu');
    expect(screen.getByTestId('outline-slide-2')).toBeTruthy();
    expect(screen.getByTestId('outline-line-3').getAttribute('data-level')).toBe('1');
    // Shift+Tab twice on "Doanh thu": a new slide.
    const line3 = screen.getByTestId('outline-line-3');
    await act(async () => {
      fireEvent.keyDown(line3, { key: 'Tab', shiftKey: true });
    });
    await act(async () => {
      fireEvent.keyDown(screen.getByTestId('outline-line-3'), { key: 'Tab', shiftKey: true });
    });
    const applies = sent.filter((c) => c.cmd === 'applyOutline');
    expect(applies).toHaveLength(2);
    expect(applies[1]).toEqual({
      cmd: 'applyOutline',
      current: 2,
      slides: [
        { slide: 0, title: 'Mở đầu', bullets: [] },
        { slide: 1, title: 'Kế hoạch', bullets: [{ text: 'Mục tiêu', level: 0 }] },
        { title: 'Doanh thu', bullets: [{ text: 'Chi phí', level: 0 }] },
      ],
    });
    expect(screen.getByTestId('outline-line-3').getAttribute('data-kind')).toBe('title');
    // A text edit is sent when the line loses focus; the new slide now has index 2.
    const title = screen.getByTestId('outline-line-3');
    fireEvent.focus(title);
    expect(onSelect).toHaveBeenCalledWith(2);
    fireEvent.change(title, { target: { value: 'Doanh thu năm' } });
    await act(async () => {
      fireEvent.blur(title);
    });
    const last = sent.filter((c) => c.cmd === 'applyOutline').at(-1);
    expect(last).toMatchObject({
      slides: [{ slide: 0 }, { slide: 1 }, { slide: 2, title: 'Doanh thu năm' }],
    });
    // Enter at the end of the first title adds an empty first bullet under it.
    const top = screen.getByTestId('outline-line-0') as HTMLInputElement;
    top.setSelectionRange(top.value.length, top.value.length);
    await act(async () => {
      fireEvent.keyDown(top, { key: 'Enter' });
    });
    expect(sent.filter((c) => c.cmd === 'applyOutline').at(-1)).toMatchObject({
      current: 0,
      slides: [{ slide: 0, title: 'Mở đầu', bullets: [{ text: '', level: 0 }] }, {}, {}],
    });
  });
});

describe('OutlineView undo', () => {
  it('Ctrl+Z / Ctrl+Y in a line run the engine history after sending the typing', async () => {
    const sent: SlideCommand[] = [];
    let current = deck;
    const runResult = vi.fn(async (c: SlideCommand) => {
      sent.push(c);
      if (c.cmd === 'undo') current = [{ ...deck[0]!, title: 'Đã hoàn tác' }, deck[1]!];
      return { outline: current } as unknown as SlideEditResult;
    });
    render(wrap(<OutlineView runResult={runResult} revision={1} current={0} onSelect={vi.fn()} />));
    const first = (await screen.findByTestId('outline-line-0')) as HTMLInputElement;
    fireEvent.change(first, { target: { value: 'Mở đầu mới' } });
    let ev: KeyboardEvent | undefined;
    await act(async () => {
      ev = new KeyboardEvent('keydown', {
        key: 'z',
        ctrlKey: true,
        bubbles: true,
        cancelable: true,
      });
      first.dispatchEvent(ev);
    });
    // The text box's own undo is not used.
    expect(ev?.defaultPrevented).toBe(true);
    const cmds = sent.map((c) => c.cmd);
    // The typed text becomes a step first, then the engine undoes, then the outline is read again.
    const apply = cmds.indexOf('applyOutline');
    const undo = cmds.indexOf('undo');
    expect(apply).toBeGreaterThanOrEqual(0);
    expect(undo).toBeGreaterThan(apply);
    expect(cmds.slice(undo + 1)).toContain('outline');
    expect(((await screen.findByTestId('outline-line-0')) as HTMLInputElement).value).toBe(
      'Đã hoàn tác',
    );
    // Ctrl+Y and Ctrl+Shift+Z redo.
    await act(async () => {
      fireEvent.keyDown(screen.getByTestId('outline-line-0'), { key: 'y', ctrlKey: true });
    });
    await act(async () => {
      fireEvent.keyDown(screen.getByTestId('outline-line-0'), {
        key: 'Z',
        ctrlKey: true,
        shiftKey: true,
      });
    });
    expect(sent.filter((c) => c.cmd === 'redo')).toHaveLength(2);
    // Nothing typed: Ctrl+Z only undoes.
    const before = sent.filter((c) => c.cmd === 'applyOutline').length;
    await act(async () => {
      fireEvent.keyDown(screen.getByTestId('outline-line-0'), { key: 'z', ctrlKey: true });
    });
    expect(sent.filter((c) => c.cmd === 'applyOutline')).toHaveLength(before);
    expect(sent.filter((c) => c.cmd === 'undo')).toHaveLength(2);
  });
});

describe('NotesPageView', () => {
  const page: SlideNotesPage = {
    width: 540,
    height: 720,
    image: { x: 54, y: 90, w: 432, h: 243 },
    body: { x: 54, y: 346.5, w: 432, h: 283.5 },
    text: 'Ghi chú',
    size: 12,
    hf: [
      {
        kind: 'number',
        rect: { x: 270, y: 684, w: 270, h: 36 },
        text: '1',
        align: 'right',
        size: 12,
        bottom: true,
      },
    ],
  };

  it('scales the page boxes', () => {
    // 540 pt page shown 270 px wide: 0.5 px per point.
    expect(boxStyle(page.body, 0.5)).toEqual({ left: 27, top: 173.25, width: 216, height: 141.75 });
  });

  it('lays out the notes page and saves notes edits with setNotes', async () => {
    const run = vi.fn();
    const runResult = vi.fn(
      async () => ({ notesPages: [page, { ...page, text: '' }] }) as unknown as SlideEditResult,
    );
    const canvas = vi.fn((i: number, w: number) => <span data-testid={`pic-${i}`}>{w}</span>);
    render(
      wrap(
        <NotesPageView
          runResult={runResult}
          run={run}
          revision={0}
          current={0}
          onSelect={() => undefined}
          canvas={canvas}
          pageWidth={270}
        />,
      ),
    );
    const text = (await screen.findByTestId('notes-page-text-1')) as HTMLTextAreaElement;
    expect(text.value).toBe('Ghi chú');
    expect(text.style.top).toBe('173.25px');
    expect(text.style.fontSize).toBe('6px');
    // The slide picture: 432 pt × 0.5.
    expect(screen.getByTestId('pic-0').textContent).toBe('216');
    // The notes master's page number, in its box at the page scale.
    const num = screen.getByTestId('notes-page-hf-number-1');
    expect(num.textContent).toBe('1');
    expect(num.style.top).toBe('342px');
    expect(num.style.left).toBe('135px');
    expect(num.style.textAlign).toBe('right');
    expect(num.getAttribute('data-bottom')).toBe('true');
    expect(screen.getByTestId('notes-page-2').style.height).toBe('360px');
    fireEvent.change(text, { target: { value: 'Ghi chú mới' } });
    fireEvent.blur(text);
    expect(run).toHaveBeenCalledWith({ cmd: 'setNotes', slide: 0, text: 'Ghi chú mới' });
    // Unchanged text sends nothing.
    fireEvent.focus(screen.getByTestId('notes-page-text-2'));
    fireEvent.blur(screen.getByTestId('notes-page-text-2'));
    expect(run).toHaveBeenCalledTimes(1);
  });
});

describe('sections', () => {
  const sections: SlideSection[] = [
    { name: 'A', id: '{A}', first: 0, count: 2 },
    { name: 'B', id: '{B}', first: 2, count: 0 },
    { name: 'C', id: '{C}', first: 2, count: 1 },
  ];

  it('places headers before their slides and hides collapsed ones', () => {
    expect(paneEntries(3, undefined, new Set()).map((e) => e.kind)).toEqual([
      'slide',
      'slide',
      'slide',
    ]);
    const e = paneEntries(3, sections, new Set(['{A}']));
    expect(e.map((x) => `${x.kind}${x.index}`)).toEqual([
      'section0',
      'section1',
      'section2',
      'slide2',
    ]);
    expect(sectionAt(sections, 1)).toBe(0);
    expect(sectionAt(sections, 2)).toBe(2);
    // An empty section picked from its header.
    expect(sectionAt(sections, 2, 1)).toBe(1);
    expect(sectionAt(undefined, 0)).toBeNull();
    expect(sectionState(sections, 2, null)).toEqual({
      sectionCount: 3,
      section: 2,
      sectionIds: ['{A}', '{B}', '{C}'],
    });
  });

  it('maps section commands onto engine commands', async () => {
    const sent: SlideCommand[] = [];
    const reg = new CommandRegistry<SectionState>();
    const openRename = vi.fn();
    const collapse = vi.fn();
    registerSectionCommands(reg, {
      run: (c) => sent.push(c),
      openRename,
      names: () => ({ section: 'Phần mới', first: 'Phần đầu' }),
      collapse,
    });
    const none: SectionState = { hasDeck: true, current: 1, sectionCount: 0, section: null };
    const s: SectionState = {
      hasDeck: true,
      current: 1,
      sectionCount: 3,
      section: 0,
      sectionIds: ['{A}', '{B}', '{C}'],
    };
    expect(reg.isEnabled('section.rename', none)).toBe(false);
    expect(reg.isEnabled('section.moveUp', s)).toBe(false);
    expect(reg.isEnabled('section.moveDown', s)).toBe(true);
    await reg.execute('section.add', none);
    await reg.execute('section.moveDown', s);
    await reg.execute('section.remove', { ...s, section: 2 });
    await reg.execute('section.removeWithSlides', s);
    await reg.execute('section.removeAll', s);
    await reg.execute('section.rename', s);
    await reg.execute('section.collapseAll', s);
    await reg.execute('section.expandAll', s);
    expect(sent).toEqual([
      { cmd: 'addSection', name: 'Phần mới', firstName: 'Phần đầu', slide: 1 },
      { cmd: 'moveSection', index: 0, to: 1 },
      { cmd: 'removeSection', index: 2 },
      { cmd: 'removeSection', index: 0, slides: true },
      { cmd: 'removeAllSections' },
    ]);
    expect(openRename).toHaveBeenCalledOnce();
    expect(collapse.mock.calls).toEqual([[['{A}', '{B}', '{C}']], [[]]]);
  });
});

describe('notes / handout master views', () => {
  const handout: SlidePageMasterInfo = {
    kind: 'handout',
    header: true,
    date: false,
    footer: true,
    number: true,
    slideImage: false,
    body: false,
    portrait: true,
    perPage: 1,
    slots: [{ x: 36, y: 228.375, w: 468, h: 263.25 }],
  };

  it('maps the view commands onto engine commands', async () => {
    const sent: SlideCommand[] = [];
    const reg = new CommandRegistry<PageMasterState>();
    registerPageMasterCommands(reg, (c) => sent.push(c));
    const normal: PageMasterState = { hasDeck: true, pageMaster: null };
    const inView: PageMasterState = { hasDeck: true, pageMaster: handout };
    expect(reg.isEnabled('view.handoutMaster', { hasDeck: true, masterView: true })).toBe(false);
    expect(reg.isEnabled('pageMaster.close', normal)).toBe(false);
    // Slide image / notes body toggles belong to the notes master.
    expect(reg.isEnabled('pageMaster.slideImage.on', inView)).toBe(false);
    expect(
      reg.isEnabled('pageMaster.slideImage.on', {
        ...inView,
        pageMaster: { ...handout, kind: 'notes' },
      }),
    ).toBe(true);
    await reg.execute('view.handoutMaster', normal);
    await reg.execute('view.notesMaster', normal);
    await reg.execute('pageMaster.date.on', inView);
    await reg.execute('pageMaster.header.off', inView);
    await reg.execute('pageMaster.landscape', inView);
    await reg.execute('pageMaster.perPage.4', inView);
    await reg.execute('pageMaster.close', inView);
    expect(sent).toEqual([
      { cmd: 'pageMasterView', kind: 'handout', on: true },
      { cmd: 'pageMasterView', kind: 'notes', on: true },
      { cmd: 'pageMasterElements', date: true },
      { cmd: 'pageMasterElements', header: false },
      { cmd: 'pageOrientation', portrait: false },
      { cmd: 'handoutSlides', perPage: 4 },
      { cmd: 'pageMasterView', on: false },
    ]);
  });

  it('builds the contextual tab and the handout preview boxes', () => {
    const reg = new CommandRegistry<PageMasterState>();
    const sent: SlideCommand[] = [];
    registerPageMasterCommands(reg, (c) => sent.push(c));
    const state: PageMasterState = { hasDeck: true, pageMaster: handout };
    const item = (
      id: string,
      label: string,
      icon: ReactNode,
      size: 'large' | 'small' = 'large',
      checked?: boolean,
    ) => ({
      id,
      label,
      icon,
      size,
      enabled: reg.isEnabled(id, state),
      checked,
      onSelect: () => void reg.execute(id, state),
    });
    const tab = pageMasterTab(t, item, handout);
    expect(tab.label).toBe('Tài liệu phát tay cái');
    const ids = tab.groups.flatMap((g) => g.items.map((i) => i.id));
    // Present placeholders switch off, absent ones on; no notes-only toggles.
    expect(ids).toContain('pageMaster.header.off');
    expect(ids).toContain('pageMaster.date.on');
    expect(ids).not.toContain('pageMaster.slideImage.on');
    expect(ids).toContain('pageMaster.perPage.9');
    render(wrap(<Ribbon tabs={[tab]} value="pageMaster" />));
    fireEvent.click(screen.getByRole('button', { name: 'Ngày' }));
    expect(sent).toEqual([{ cmd: 'pageMasterElements', date: true }]);
    cleanup();
    // 0.5 px per point.
    render(<HandoutSlots info={handout} scale={0.5} />);
    const slot = screen.getByTestId('handout-slot-1');
    expect(slot.style.left).toBe('18px');
    expect(slot.style.top).toBe('114.1875px');
    expect(slot.style.width).toBe('234px');
  });
});

describe('outlineKey', () => {
  it('treats a body of empty paragraphs as no bullets and ignores slide indexes', async () => {
    const { outlineKey } = await import('../src/outlineModel');
    expect(outlineKey([{ title: 'A', slide: 3, bullets: [{ text: '', level: 0 }] }])).toBe(
      outlineKey([{ title: 'A', bullets: [] }]),
    );
    expect(outlineKey([{ title: 'A', bullets: [{ text: 'x', level: 0 }] }])).not.toBe(
      outlineKey([{ title: 'A', bullets: [] }]),
    );
  });
});
