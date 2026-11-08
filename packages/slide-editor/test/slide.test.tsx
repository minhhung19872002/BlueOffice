// @vitest-environment happy-dom
import { act, cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import type {
  OpenedFile,
  Platform,
  SlideAnimInfo,
  SlideCommand,
  SlideEditResult,
  SlideMediaInfo,
  SlideShapeInfo,
  SlideShowInfo,
  SlideTransitionInfo,
} from '@blueoffice/platform';
import {
  advance,
  AudienceLink,
  BlueSlideEditor,
  crc32,
  effectFrames,
  effectsAt,
  fit,
  fromGrid,
  parseValue,
  stepDuration,
  transitionPlan,
  visibility,
  zipStore,
  type ShowPos,
  formatElapsed,
  fromEditorText,
  hexOf,
  nudgeCommand,
  showAction,
  SlideController,
  step,
  toEditorText,
} from '../src';

afterEach(cleanup);

const TRANSITION: SlideTransitionInfo = {
  kind: 'fade',
  known: true,
  dir: null,
  durMs: 500,
  advanceOnClick: true,
  advanceAfterMs: null,
};

describe('show logic', () => {
  it('maps keys and steps through the deck', () => {
    expect(showAction('ArrowRight')).toBe('next');
    expect(showAction(' ')).toBe('next');
    expect(showAction('PageUp')).toBe('prev');
    expect(showAction('Escape')).toBe('exit');
    expect(showAction('b')).toBe('black');
    expect(showAction('x')).toBeNull();
    expect(step(2, 3, 'next')).toBe(3); // end-of-show screen
    expect(step(3, 3, 'next')).toBe(3);
    expect(step(0, 3, 'prev')).toBe(0);
    expect(step(1, 3, 'last')).toBe(2);
  });

  it('formats time and fits slides', () => {
    expect(formatElapsed(65_000)).toBe('01:05');
    expect(formatElapsed(3_723_000)).toBe('1:02:03');
    expect(fit({ width: 960, height: 540 }, { width: 800, height: 800 })).toEqual({
      width: 800,
      height: 450,
    });
    expect(fit({ width: 0, height: 0 }, { width: 1, height: 1 })).toEqual({ width: 0, height: 0 });
  });
});

describe('editing helpers', () => {
  it('converts line breaks between the engine and the text editor', () => {
    expect(toEditorText('a\u000bb\nc')).toBe('a b\nc');
    expect(fromEditorText('a b\r\nc')).toBe('a\u000bb\nc');
  });

  it('nudges with arrows and parses colours', () => {
    expect(nudgeCommand('ArrowLeft', false)).toEqual({
      cmd: 'transform',
      transform: { kind: 'move', dx: -1, dy: 0 },
      snap: false,
    });
    expect(nudgeCommand('ArrowDown', true)).toMatchObject({ transform: { dy: 10 } });
    expect(nudgeCommand('a', false)).toBeNull();
    expect(hexOf('#c00000')).toBe('C00000');
    expect(hexOf('red')).toBeNull();
  });

  it('coalesces drag previews and serialises commands', async () => {
    const seen: SlideCommand[] = [];
    let release: () => void = () => {};
    const engine = {
      slideCommand: vi.fn(async (c: SlideCommand) => {
        seen.push(c);
        if (seen.length === 1) await new Promise<void>((r) => (release = r));
        return { slide: 0 } as SlideEditResult;
      }),
    };
    const results: SlideEditResult[] = [];
    const ctrl = new SlideController(
      engine as unknown as Platform['engine'],
      (r) => results.push(r),
      () => {},
    );
    ctrl.preview({ kind: 'move', dx: 1, dy: 0 }, true, 4);
    await waitFor(() => expect(seen).toHaveLength(1));
    for (const dx of [2, 3, 4]) ctrl.preview({ kind: 'move', dx, dy: 0 }, true, 4);
    release();
    await waitFor(() => expect(results).toHaveLength(2));
    // First preview, then only the latest one.
    expect(seen.map((c) => (c.cmd === 'transform' ? c.transform : null))).toEqual([
      { kind: 'move', dx: 1, dy: 0 },
      { kind: 'move', dx: 4, dy: 0 },
    ]);
    expect(results).toHaveLength(2);
  });
});

function fakePlatform() {
  let openHandler: ((f: OpenedFile) => void) | null = null;
  const saved: string[] = [];
  const st = {
    slide: 0,
    sel: [] as number[],
    notes: ['Ghi chú 1', 'Ghi chú 2', 'Ghi chú 3'],
    text: 'Tiêu đề',
    modified: false,
    editing: null as number | null,
    ts: [0, 0] as [number, number],
    clip: undefined as 'shapes' | 'slides' | undefined,
    emptyHit: false,
    clipboard: undefined as { text: string; png?: string } | undefined,
    transition: null as SlideTransitionInfo | null,
    animations: [] as SlideAnimInfo[],
    chart: false,
    media: [] as SlideMediaInfo[],
    /** The show data of slide 1 has one click step (an entrance). */
    showAnims: false,
  };
  const info = (name = 'Báo cáo.pptx') => ({
    name,
    pageCount: 3,
    pages: [0, 1, 2].map(() => ({ width: 960, height: 540 })),
    wordCount: 0,
    kind: 'slide' as const,
  });
  const shape = (): SlideShapeInfo => ({
    id: 7,
    name: 'Title 1',
    kind: st.chart ? 'chart' : 'shape',
    rect: { cx: 480, cy: 100, w: 600, h: 80, rot: 0 },
    text: st.text,
    placeholder: 'title',
    lockAspect: false,
    editableText: !st.chart,
  });
  const anim = (i: number, a: Partial<SlideAnimInfo>): SlideAnimInfo => ({
    index: i,
    shapeId: 7,
    shapeName: 'Title 1',
    effect: 'fade',
    class: 'entr',
    direction: null,
    trigger: 'onClick',
    delayMs: 0,
    durMs: 500,
    click: i + 1,
    startMs: 0,
    endMs: 500,
    rect: [180, 60, 600, 80],
    ...a,
  });
  const result = (): SlideEditResult => ({
    info: info(),
    slide: st.slide,
    selection: st.sel.map(shape),
    bounds: st.sel.length ? shape().rect : null,
    handles: st.sel.length
      ? [
          { kind: 'se', x: 780, y: 140 },
          { kind: 'rot', x: 480, y: 42 },
        ]
      : [],
    placeholders: [],
    dirty: [],
    slidesChanged: false,
    modified: st.modified,
    canUndo: st.modified,
    canRedo: false,
    format: {
      hasText: st.sel.length > 0,
      bold: false,
      italic: false,
      underline: false,
      size: 24,
      color: null,
      align: 'left',
      bullets: false,
      strike: false,
      highlight: null,
      font: null,
      numbering: false,
      level: 0,
      lineSpacing: null,
    },
    notes: st.notes[st.slide] ?? '',
    layouts: [],
    clip: st.clip,
    clipboard: st.clipboard,
    adjust: st.sel.length ? [{ index: 0, x: 300, y: 60 }] : [],
    textEdit:
      st.editing === null
        ? undefined
        : {
            id: st.editing,
            start: st.ts[0],
            end: st.ts[1],
            caret: st.ts[0] === st.ts[1] ? [200, 70, 200, 90] : null,
            rects: st.ts[0] === st.ts[1] ? [] : [{ cx: 220, cy: 80, w: 40, h: 20, rot: 0 }],
            rect: shape().rect,
            text: st.text,
          },
    transition: st.transition,
    animations: st.animations,
    media: st.media,
    chart:
      st.chart && st.sel.length
        ? {
            id: 7,
            kind: 'column',
            title: 'Doanh thu',
            legend: 'b',
            data: { categories: ['Q1', 'Q2'], series: [{ name: '2026', values: [1, 2] }] },
            embedded: true,
          }
        : undefined,
  });
  const showInfo = (slide: number): SlideShowInfo => ({
    slide,
    width: 960,
    height: 540,
    transition: slide === 1 ? { ...TRANSITION, kind: 'push', dir: 'u' } : null,
    layers: [
      { ids: [2], backdrop: true, page: { width: 960, height: 540, commands: [] } },
      { ids: [7], backdrop: false, page: { width: 960, height: 540, commands: [] } },
    ],
    animations: slide === 0 && st.showAnims ? [{ ...anim(0, {}), layer: 1 }] : [],
    clicks: slide === 0 && st.showAnims ? 1 : 0,
    media: [],
  });
  const engine = {
    backend: 'wasm',
    init: async () => {},
    open: vi.fn(async (_b: Uint8Array, name: string) => info(name)),
    getPage: async () => ({ width: 960, height: 540, commands: [] }),
    getGlyphOutlines: async () => [],
    getMedia: async () => new Uint8Array(),
    getSlideNotes: vi.fn(async (i: number) => st.notes[i] ?? ''),
    savePptx: vi.fn(async () => new Uint8Array([80, 75])),
    markSaved: vi.fn(async () => {
      st.modified = false;
    }),
    exportPdf: async () => new Uint8Array([37]),
    slideCommand: vi.fn(async (c: SlideCommand) => {
      const keep = ['state', 'setText', 'formatText', 'textSelect', 'textHit', 'undo', 'redo'];
      if (!keep.includes(c.cmd)) st.editing = null;
      st.clipboard = undefined;
      if (c.cmd === 'hitTest') return { ...result(), hits: st.emptyHit ? [] : [7] };
      if (c.cmd === 'show') return { ...result(), show: showInfo(c.slide) };
      switch (c.cmd) {
        case 'copy':
        case 'copySlides':
          st.clip = c.cmd === 'copy' ? 'shapes' : 'slides';
          st.clipboard = { text: 'Tiêu đề', png: 'iVBORw0KGgo=' };
          break;
        case 'textSelect':
          st.editing = c.id ?? 7;
          st.ts = [c.start, c.end];
          break;
        case 'textHit':
          st.ts = [2, 2];
          break;
        case 'goTo':
          st.slide = c.slide;
          st.sel = [];
          break;
        case 'click':
          st.sel = [7];
          break;
        case 'select':
          st.sel = c.ids;
          break;
        case 'setText':
          st.text = c.text;
          if (c.sel) st.ts = c.sel;
          st.modified = true;
          break;
        case 'setNotes':
          st.notes[c.slide ?? st.slide] = c.text;
          st.modified = true;
          break;
        case 'delete':
          st.sel = [];
          st.modified = true;
          break;
        case 'state':
          break;
        case 'setTransition':
          st.transition =
            c.transition.kind === 'none'
              ? null
              : {
                  ...TRANSITION,
                  ...st.transition,
                  ...(c.transition.kind === 'keep' ? {} : { kind: c.transition.kind }),
                  ...(c.transition.durMs ? { durMs: c.transition.durMs } : {}),
                };
          st.modified = true;
          break;
        case 'addAnimation':
          st.animations = [
            ...st.animations,
            anim(st.animations.length, { effect: c.effect ?? 'fade', class: c.class ?? 'entr' }),
          ];
          st.modified = true;
          break;
        case 'setAnimation':
          st.animations = st.animations.map((a) =>
            a.index === c.index
              ? { ...a, trigger: c.trigger ?? a.trigger, durMs: c.durMs ?? a.durMs }
              : a,
          );
          st.modified = true;
          break;
        case 'removeAnimation':
          st.animations = st.animations
            .filter((a) => a.index !== c.index)
            .map((a, i) => ({ ...a, index: i }));
          st.modified = true;
          break;
        case 'insertChart':
          st.chart = true;
          st.sel = [7];
          st.modified = true;
          break;
        default:
          st.modified = true;
      }
      return result();
    }),
    slideInsertPicture: vi.fn(async () => result()),
  };
  const p = {
    kind: 'web',
    engine,
    files: {
      openFile: async () => ({ name: 'Báo cáo.pptx', bytes: new Uint8Array([1]) }),
      saveFile: async (_b: Uint8Array, n: string) => {
        saved.push(n);
        return n;
      },
    },
    print: { printPdf: async () => {} },
    clipboard: {
      writeItems: vi.fn(async () => {}),
      writeHtml: vi.fn(async () => {}),
      read: vi.fn(async () => ({})),
    },
    window: { setTitle: vi.fn(), openNew: async () => {} },
    shell: {
      recentFiles: async () => [],
      addRecentFile: async () => {},
      onOpenRequest: (h: (f: OpenedFile) => void) => {
        openHandler = h;
        return () => {};
      },
    },
  } as unknown as Platform;
  const cmds = () => engine.slideCommand.mock.calls.map((c) => c[0]);
  return { p, engine, saved, cmds, st, open: (f: OpenedFile) => openHandler?.(f) };
}

async function openDeck(p: Platform, label = 'Mở') {
  fireEvent.click(screen.getByText(label));
  await screen.findByDisplayValue('Ghi chú 1');
}

describe('BlueSlideEditor', () => {
  it('opens a deck, shows thumbnails and notes, navigates and saves', async () => {
    const { p, engine, saved } = fakePlatform();
    render(<BlueSlideEditor platform={p} language="vi-VN" />);
    await openDeck(p);
    expect(engine.open).toHaveBeenCalledWith(expect.any(Uint8Array), 'Báo cáo.pptx');
    expect(screen.getAllByRole('img', { name: /Slide \d \/ 3/ }).length).toBeGreaterThanOrEqual(3);
    fireEvent.keyDown(window, { key: 'ArrowDown' });
    await screen.findByDisplayValue('Ghi chú 2');
    expect(screen.getByText('Slide 2 / 3')).toBeTruthy();
    fireEvent.keyDown(window, { key: 's', ctrlKey: true });
    await waitFor(() => expect(saved).toEqual(['Báo cáo.pptx']));
    expect(engine.markSaved).toHaveBeenCalled();
  });

  it('runs the presenter view with next slide, notes and timer, Esc exits', async () => {
    const { p } = fakePlatform();
    render(<BlueSlideEditor platform={p} language="en-US" />);
    await openDeck(p, 'Open');
    fireEvent.click(screen.getByText('Presenter view'));
    await screen.findByText('Next slide', { selector: 'h3' });
    expect(screen.getByText('00:00')).toBeTruthy();
    await screen.findByText('Ghi chú 1');
    fireEvent.keyDown(window, { key: 'ArrowRight' });
    await screen.findByText('Ghi chú 2');
    fireEvent.keyDown(window, { key: 'Escape' });
    await waitFor(() => expect(screen.queryByText('Next slide', { selector: 'h3' })).toBeNull());
    await screen.findByDisplayValue('Ghi chú 2');
    expect(screen.getByText('Slide 2 of 3')).toBeTruthy();
  });

  it('switches to the slide sorter and back', async () => {
    const { p } = fakePlatform();
    render(<BlueSlideEditor platform={p} language="en-US" />);
    await openDeck(p, 'Open');
    fireEvent.mouseDown(screen.getByRole('tab', { name: 'View' }));
    fireEvent.click(await screen.findByText('Slide sorter'));
    const items = await screen.findAllByRole('img', { name: /Slide \d of 3/ });
    fireEvent.doubleClick(items[2]!.parentElement!);
    await screen.findByDisplayValue('Ghi chú 3');
  });

  it('selects, edits text with an IME composition, and runs arrange commands', async () => {
    const { p, cmds } = fakePlatform();
    render(<BlueSlideEditor platform={p} language="vi-VN" />);
    await openDeck(p);
    const overlay = screen.getByTestId('slide-overlay');
    fireEvent.pointerDown(overlay, { button: 0, clientX: 10, clientY: 10, pointerId: 1 });
    fireEvent.pointerUp(overlay, { button: 0, clientX: 10, clientY: 10, pointerId: 1 });
    await screen.findByTestId('selection-box');
    expect(screen.getByTestId('handle-se')).toBeTruthy();

    // Arrow nudges the selection by 1 pt, Shift by 10 pt.
    fireEvent.keyDown(window, { key: 'ArrowRight' });
    await waitFor(() =>
      expect(cmds()).toContainEqual({
        cmd: 'transform',
        transform: { kind: 'move', dx: 1, dy: 0 },
        snap: false,
      }),
    );
    fireEvent.keyDown(window, { key: 'd', ctrlKey: true });
    await waitFor(() => expect(cmds()).toContainEqual({ cmd: 'duplicate' }));

    // Double-click edits the text in place: caret on the canvas, IME composition,
    // typing sent live, shortcuts and Tab inside the text, Escape leaves.
    fireEvent.doubleClick(overlay);
    const ta = (await screen.findByTestId('slide-text-editor')) as HTMLTextAreaElement;
    expect(ta.value).toBe('Tiêu đề');
    await waitFor(() =>
      expect(cmds()).toContainEqual({ cmd: 'textSelect', id: 7, start: 7, end: 7 }),
    );
    await screen.findByTestId('text-caret');
    fireEvent.compositionStart(ta);
    fireEvent.keyDown(ta, { key: 'Escape', isComposing: true });
    expect(screen.getByTestId('slide-text-editor')).toBeTruthy();
    ta.value = 'Xin chào Việt Nam';
    fireEvent.compositionEnd(ta, { data: 'Nam' });
    await waitFor(() =>
      expect(cmds()).toContainEqual(
        expect.objectContaining({ cmd: 'setText', id: 7, text: 'Xin chào Việt Nam', merge: true }),
      ),
    );
    fireEvent.keyDown(ta, { key: 'b', ctrlKey: true });
    await waitFor(() =>
      expect(cmds()).toContainEqual({ cmd: 'formatText', format: { kind: 'bold' } }),
    );
    fireEvent.keyDown(ta, { key: 'Tab' });
    await waitFor(() =>
      expect(cmds()).toContainEqual({ cmd: 'formatText', format: { kind: 'level', delta: 1 } }),
    );
    // A click inside the shape moves the caret through the engine.
    fireEvent.pointerDown(overlay, { button: 0, clientX: 0, clientY: 0, pointerId: 2 });
    fireEvent.pointerUp(overlay, { button: 0, clientX: 0, clientY: 0, pointerId: 2 });
    fireEvent.keyDown(ta, { key: 'Escape' });
    await waitFor(() => expect(cmds()).toContainEqual({ cmd: 'textExit' }));
    expect(screen.queryByTestId('slide-text-editor')).toBeNull();
    await screen.findByTestId('status-modified');

    // Arrange tab: bring to front; Delete removes the selection.
    fireEvent.mouseDown(screen.getByRole('tab', { name: 'Sắp xếp' }));
    fireEvent.click(await screen.findByText('Đưa lên trên cùng'));
    await waitFor(() => expect(cmds()).toContainEqual({ cmd: 'zOrder', to: 'front' }));
    fireEvent.keyDown(window, { key: 'Delete' });
    await waitFor(() => expect(cmds()).toContainEqual({ cmd: 'delete' }));
    await waitFor(() => expect(screen.queryByTestId('selection-box')).toBeNull());
  });

  it('copies to the system clipboard and pastes internal copies, pictures and text', async () => {
    const { p, engine, cmds } = fakePlatform();
    render(<BlueSlideEditor platform={p} language="vi-VN" />);
    await openDeck(p);
    const overlay = screen.getByTestId('slide-overlay');
    fireEvent.pointerDown(overlay, { button: 0, clientX: 10, clientY: 10, pointerId: 1 });
    fireEvent.pointerUp(overlay, { button: 0, clientX: 10, clientY: 10, pointerId: 1 });
    await screen.findByTestId('selection-box');
    fireEvent.keyDown(window, { key: 'c', ctrlKey: true });
    await waitFor(() => expect(cmds()).toContainEqual({ cmd: 'copy' }));
    const write = p.clipboard.writeItems as unknown as ReturnType<typeof vi.fn>;
    await waitFor(() => expect(write).toHaveBeenCalled());
    const items = write.mock.calls[0]![0] as { text: string; html: string; png: Uint8Array };
    expect(items.text).toBe('Tiêu đề');
    expect(items.png).toBeInstanceOf(Uint8Array);
    const paste = (data: Record<string, string>, file?: File) => {
      const e = new Event('paste', { bubbles: true }) as Event & { clipboardData: unknown };
      e.clipboardData = {
        getData: (t: string) => data[t] ?? '',
        items: file ? [{ type: file.type, getAsFile: () => file }] : [],
      };
      act(() => {
        window.dispatchEvent(e);
      });
    };
    // Our own copy: the internal clipboard (full shapes).
    paste({ 'text/html': items.html, 'text/plain': items.text });
    await waitFor(() => expect(cmds()).toContainEqual({ cmd: 'paste' }));
    // Text from another program: a text box.
    paste({ 'text/plain': 'Từ chương trình khác' });
    await waitFor(() =>
      expect(cmds()).toContainEqual({ cmd: 'insertTextBox', text: 'Từ chương trình khác' }),
    );
    // A picture: inserted as a picture.
    paste({}, new File([new Uint8Array([137, 80, 78, 71])], 'a.png', { type: 'image/png' }));
    await waitFor(() => expect(engine.slideInsertPicture).toHaveBeenCalled());
    // In the slide panel Ctrl+C copies the slide.
    screen.getByTestId('thumb-1').focus();
    fireEvent.keyDown(screen.getByTestId('thumb-1'), { key: 'c', ctrlKey: true });
    await waitFor(() => expect(cmds()).toContainEqual({ cmd: 'copySlides', indices: [0] }));
  });

  it('selects with a marquee and cycles shapes with Tab', async () => {
    const { p, cmds, st } = fakePlatform();
    render(<BlueSlideEditor platform={p} language="vi-VN" />);
    await openDeck(p);
    const overlay = screen.getByTestId('slide-overlay');
    overlay.getBoundingClientRect = () =>
      ({ left: 0, top: 0, width: 960, height: 540, right: 960, bottom: 540 }) as DOMRect;
    st.emptyHit = true;
    fireEvent.pointerDown(overlay, { button: 0, clientX: 700, clientY: 400, pointerId: 1 });
    await waitFor(() => expect(cmds()).toContainEqual({ cmd: 'select', ids: [] }));
    fireEvent.pointerMove(overlay, { clientX: 900, clientY: 500, pointerId: 1 });
    await screen.findByTestId('marquee');
    fireEvent.pointerUp(overlay, { button: 0, clientX: 900, clientY: 500, pointerId: 1 });
    await waitFor(() =>
      expect(cmds()).toContainEqual({
        cmd: 'selectRect',
        rect: { x: 700, y: 400, w: 200, h: 100 },
        extend: false,
      }),
    );
    expect(screen.queryByTestId('marquee')).toBeNull();
    fireEvent.keyDown(window, { key: 'Tab' });
    await waitFor(() => expect(cmds()).toContainEqual({ cmd: 'selectNext', back: false }));
    // Yellow adjust handle: dragging sends live adjust commands.
    st.emptyHit = false;
    fireEvent.pointerDown(overlay, { button: 0, clientX: 480, clientY: 100, pointerId: 3 });
    fireEvent.pointerUp(overlay, { button: 0, clientX: 480, clientY: 100, pointerId: 3 });
    const adj = await screen.findByTestId('adjust-0');
    fireEvent.pointerDown(adj, { button: 0, clientX: 300, clientY: 60, pointerId: 4 });
    fireEvent.pointerMove(overlay, { clientX: 330, clientY: 60, pointerId: 4 });
    fireEvent.pointerUp(overlay, { clientX: 340, clientY: 60, pointerId: 4 });
    await waitFor(() =>
      expect(cmds()).toContainEqual({ cmd: 'adjust', index: 0, x: 330, y: 60, merge: true }),
    );
    fireEvent.keyDown(window, { key: 'Tab', shiftKey: true });
    await waitFor(() => expect(cmds()).toContainEqual({ cmd: 'selectNext', back: true }));
  });

  it('edits notes, inserts shapes and adds slides', async () => {
    const { p, cmds } = fakePlatform();
    render(<BlueSlideEditor platform={p} language="vi-VN" />);
    await openDeck(p);
    const notes = screen.getByTestId('notes-editor');
    fireEvent.change(notes, { target: { value: 'Ghi chú đã sửa' } });
    fireEvent.blur(notes);
    await waitFor(() =>
      expect(cmds()).toContainEqual({ cmd: 'setNotes', slide: 0, text: 'Ghi chú đã sửa' }),
    );
    fireEvent.mouseDown(screen.getByRole('tab', { name: 'Chèn' }));
    fireEvent.click(await screen.findByText('Hình elip'));
    await waitFor(() => expect(cmds()).toContainEqual({ cmd: 'insertShape', preset: 'ellipse' }));
    fireEvent.click(screen.getByText('+ Thêm slide'));
    await waitFor(() => expect(cmds()).toContainEqual({ cmd: 'addSlide' }));
    // Drag a thumbnail onto another to reorder.
    const t1 = screen.getByTestId('thumb-1');
    const t3 = screen.getByTestId('thumb-3');
    await act(async () => {
      fireEvent.dragStart(t1);
    });
    await act(async () => {
      fireEvent.dragOver(t3);
    });
    await act(async () => {
      fireEvent.drop(t3);
    });
    await waitFor(() => expect(cmds()).toContainEqual({ cmd: 'moveSlide', from: 0, to: 2 }));
    fireEvent.keyDown(window, { key: 'z', ctrlKey: true });
    await waitFor(() => expect(cmds()).toContainEqual({ cmd: 'undo' }));
  });
});

/** In-memory window channels: every bus of a name hears the others. */
function memoryChannels() {
  const subs = new Map<string, Set<(m: unknown) => void>>();
  const sent: unknown[] = [];
  const channel = (name: string) => {
    const mine = new Set<(m: unknown) => void>();
    let set = subs.get(name);
    if (!set) subs.set(name, (set = new Set()));
    const all = set;
    return {
      post: (m: unknown) => {
        sent.push(m);
        for (const f of all) if (!mine.has(f)) f(m);
      },
      onMessage: (f: (m: unknown) => void) => {
        mine.add(f);
        all.add(f);
        return () => {
          mine.delete(f);
          all.delete(f);
        };
      },
      close: () => {
        for (const f of mine) all.delete(f);
      },
    };
  };
  return { channel, sent };
}

describe('audience screen', () => {
  it('opens a second window and keeps it on the presenter slide', async () => {
    const { channel, sent } = memoryChannels();
    const opened: string[] = [];
    const presenter = {
      channel,
      window: { openNew: async (u: string) => void opened.push(u), setTitle: () => {} },
    } as unknown as Platform;
    const link = new AudienceLink(presenter, async () => ({
      name: 'a.pptx',
      bytes: new Uint8Array([80, 75, 3]),
    }));
    await link.start(1);
    expect(opened[0]).toMatch(/^\?app=slide&audience=audience-/);
    const id = decodeURIComponent(opened[0]!.split('audience=')[1]!);

    // The audience window (same editor with `audience`) asks for the deck.
    const { p, engine } = fakePlatform();
    const audiencePlatform = { ...p, channel } as Platform;
    render(<BlueSlideEditor platform={audiencePlatform} language="vi-VN" audience={id} />);
    await waitFor(() =>
      expect(engine.open).toHaveBeenCalledWith(new Uint8Array([80, 75, 3]), 'a.pptx'),
    );
    expect(sent).toContainEqual({ type: 'hello' });
    await screen.findByRole('img', { name: 'Slide 2 / 3' });
    act(() => link.update(2, false, false));
    await screen.findByRole('img', { name: 'Slide 3 / 3' });
    act(() => link.update(2, true, false));
    await waitFor(() => expect(screen.queryByRole('img', { name: /Slide/ })).toBeNull());
    act(() => link.update(2, false, true));
    await screen.findByText('Kết thúc trình chiếu');
    link.stop();
  });
});

describe('slide show playback', () => {
  const W = 960;
  const H = 540;
  const a = (x: Partial<SlideAnimInfo>): SlideAnimInfo => ({
    index: 0,
    shapeId: 4,
    shapeName: 'Shape',
    effect: 'fade',
    class: 'entr',
    direction: null,
    trigger: 'onClick',
    delayMs: 0,
    durMs: 500,
    click: 1,
    startMs: 0,
    endMs: 500,
    rect: [96, 54, 192, 108],
    layer: 1,
    ...x,
  });
  const show = (animations: SlideAnimInfo[]): SlideShowInfo => ({
    slide: 0,
    width: W,
    height: H,
    transition: null,
    layers: [0, 1, 2].map((i) => ({
      ids: [i],
      backdrop: i === 0,
      page: { width: W, height: H, commands: [] },
    })),
    animations,
    clicks: Math.max(0, ...animations.map((x) => x.click)),
    media: [],
  });

  it('hides entering shapes until their click and exiting ones after it', () => {
    const s = show([
      a({ layer: 1 }),
      a({ layer: 2, class: 'exit', click: 2, index: 1 }),
      a({ layer: 1, class: 'emph', click: 2, index: 2 }),
    ]);
    expect(visibility(s, -1)).toEqual([true, false, true]);
    expect(visibility(s, 0)).toEqual([true, false, true]);
    expect(visibility(s, 1)).toEqual([true, true, true]);
    expect(visibility(s, 2)).toEqual([true, true, false]);
    expect(effectsAt(s, 2).map((e) => [e.layer, e.visibleAfter])).toEqual([
      [2, false],
      [1, null],
    ]);
    expect(stepDuration(s, 1)).toBe(500);
  });

  it('builds keyframes for fly-in, wipe, appear, exits and unknown effects', () => {
    expect(effectFrames(a({ effect: 'flyIn', direction: 'l' }), W, H)[0]).toMatchObject({
      transform: 'translate(-30%, 0)',
    });
    expect(effectFrames(a({ effect: 'flyIn', direction: 'b' }), W, H)[0]).toMatchObject({
      transform: 'translate(0, 90%)',
    });
    // Wipe from the top: the shape box opens downwards.
    const w = effectFrames(a({ effect: 'wipe', direction: 't' }), W, H);
    expect(w[0]).toMatchObject({ clipPath: 'inset(10% 70% 90% 10%)' });
    expect(w[1]).toMatchObject({ clipPath: 'inset(10% 70% 70% 10%)' });
    expect(effectFrames(a({ effect: 'appear' }), W, H).at(-1)).toMatchObject({ opacity: 1 });
    const out = effectFrames(a({ effect: 'fade', class: 'exit' }), W, H);
    expect(out[0]).toMatchObject({ opacity: 1 });
    expect(out[1]).toMatchObject({ opacity: 0 });
    // Not modelled: entrance plays as a fade, emphasis as a pulse.
    expect(effectFrames(a({ effect: 'other' }), W, H)).toEqual([{ opacity: 0 }, { opacity: 1 }]);
    expect(effectFrames(a({ class: 'emph', effect: 'other' }), W, H)).toHaveLength(3);
  });

  it('plans transitions and steps through clicks, slides and the end screen', () => {
    const t = (kind: string, dir: string | null = null): SlideTransitionInfo => ({
      kind,
      known: true,
      dir,
      durMs: 700,
      advanceOnClick: true,
      advanceAfterMs: null,
    });
    const push = transitionPlan(t('push', 'u'));
    expect(push.incoming?.[0]).toEqual({ transform: 'translate(0%, 100%)' });
    expect(push.outgoing?.[1]).toEqual({ transform: 'translate(0%, -100%)' });
    expect(transitionPlan(t('wipe', 'r')).incoming?.[0]).toEqual({
      clipPath: 'inset(0% 100% 0% 0%)',
    });
    expect(transitionPlan(t('pull', 'l')).outgoingOnTop).toBe(true);
    expect(transitionPlan(t('cut')).duration).toBe(0);
    expect(transitionPlan({ ...t('dissolve'), known: false }).incoming).toEqual([
      { opacity: 0 },
      { opacity: 1 },
    ]);
    const clicks = (i: number) => [2, 0, 1][i] ?? 0;
    let pos: ShowPos = { slide: 0, click: 0, enter: 'transition' };
    const seen: string[] = [];
    for (let i = 0; i < 6; i++) {
      pos = advance(pos, 'next', 3, clicks);
      seen.push(`${pos.slide}.${pos.click}.${pos.enter}`);
    }
    expect(seen).toEqual([
      '0.1.click',
      '0.2.click',
      '1.0.transition',
      '2.0.transition',
      '2.1.click',
      '3.0.transition',
    ]);
    expect(advance(pos, 'prev', 3, clicks)).toEqual({ slide: 2, click: 1, enter: 'jump' });
    expect(advance({ slide: 2, click: 0, enter: 'jump' }, 'prev', 3, clicks)).toEqual({
      slide: 1,
      click: 0,
      enter: 'jump',
    });
  });

  it('writes a stored zip and reads chart numbers', () => {
    expect(crc32(new TextEncoder().encode('123456789'))).toBe(0xcbf43926);
    const z = zipStore([
      { name: 'a.png', data: new Uint8Array([1, 2, 3]) },
      { name: 'ảnh.png', data: new Uint8Array([4]) },
    ]);
    const v = new DataView(z.buffer);
    expect(v.getUint32(0, true)).toBe(0x04034b50);
    expect(v.getUint32(z.length - 22, true)).toBe(0x06054b50);
    expect(v.getUint16(z.length - 22 + 10, true)).toBe(2);
    expect(parseValue('1.234,5')).toBe(1234.5);
    expect(parseValue('12.5')).toBe(12.5);
    expect(parseValue('')).toBeNull();
    expect(parseValue('abc')).toBeNull();
    expect(
      fromGrid({ categories: ['A'], names: ['S'], cells: [['3,5']] }).series[0]!.values,
    ).toEqual([3.5]);
  });
});

describe('BlueSlideEditor transitions, effects, charts and pictures', () => {
  it('sets a transition from the ribbon and edits effects in the pane', async () => {
    const { p, cmds } = fakePlatform();
    render(<BlueSlideEditor platform={p} language="vi-VN" />);
    await openDeck(p);
    fireEvent.mouseDown(screen.getByRole('tab', { name: 'Chuyển trang' }));
    fireEvent.click(await screen.findByRole('button', { name: 'Đẩy' }));
    await waitFor(() =>
      expect(cmds()).toContainEqual({ cmd: 'setTransition', transition: { kind: 'push' } }),
    );
    // The pane opens with the tab and shows the slide's transition.
    await screen.findByTestId('anim-pane');
    await waitFor(() =>
      expect((screen.getByTestId('transition-kind') as HTMLSelectElement).value).toBe('push'),
    );
    fireEvent.click(screen.getByText('Áp dụng cho mọi slide'));
    await waitFor(() =>
      expect(cmds()).toContainEqual(expect.objectContaining({ cmd: 'setTransition', all: true })),
    );

    // Select the title and add a fly-in, then edit its trigger and duration.
    const overlay = screen.getByTestId('slide-overlay');
    fireEvent.pointerDown(overlay, { button: 0, clientX: 10, clientY: 10, pointerId: 1 });
    fireEvent.pointerUp(overlay, { button: 0, clientX: 10, clientY: 10, pointerId: 1 });
    await screen.findByTestId('selection-box');
    fireEvent.mouseDown(screen.getByRole('tab', { name: 'Hiệu ứng' }));
    fireEvent.click(await screen.findByText('Bay vào'));
    await screen.findByTestId('anim-item-0');
    fireEvent.change(screen.getByTestId('anim-trigger-0'), { target: { value: 'afterPrevious' } });
    await waitFor(() =>
      expect(cmds()).toContainEqual({ cmd: 'setAnimation', index: 0, trigger: 'afterPrevious' }),
    );
    const dur = screen.getByTestId('anim-duration-0');
    fireEvent.change(dur, { target: { value: '1,5' } });
    fireEvent.blur(dur);
    await waitFor(() =>
      expect(cmds()).toContainEqual({ cmd: 'setAnimation', index: 0, durMs: 1500 }),
    );
    // Preview plays the slide in the pane.
    fireEvent.click(screen.getByTestId('anim-preview'));
    await waitFor(() => expect(cmds()).toContainEqual({ cmd: 'show', slide: 0 }));
    expect(await screen.findAllByTestId('show-stage')).toHaveLength(1);
    fireEvent.click(screen.getByTestId('anim-remove-0'));
    await waitFor(() => expect(cmds()).toContainEqual({ cmd: 'removeAnimation', index: 0 }));
  });

  it('plays click steps in the show before moving to the next slide', async () => {
    const { p, cmds, st } = fakePlatform();
    st.showAnims = true;
    render(<BlueSlideEditor platform={p} language="en-US" />);
    await openDeck(p, 'Open');
    fireEvent.keyDown(window, { key: 'F5' });
    await screen.findByTestId('show-stage');
    await waitFor(() => expect(screen.getAllByTestId('show-layer')).toHaveLength(2));
    expect(screen.getByTestId('show-stage').dataset.slide).toBe('0');
    // The animated title layer starts hidden (entrance on click 1).
    await waitFor(() =>
      expect(screen.getAllByTestId('show-layer')[1]!.dataset.visible).toBe('false'),
    );
    fireEvent.keyDown(window, { key: 'ArrowRight' });
    await waitFor(() => expect(screen.getByTestId('show-stage').dataset.click).toBe('1'));
    await waitFor(() =>
      expect(screen.getAllByTestId('show-layer')[1]!.dataset.visible).toBe('true'),
    );
    fireEvent.keyDown(window, { key: 'ArrowRight' });
    await waitFor(() => expect(screen.getByTestId('show-stage').dataset.slide).toBe('1'));
    await waitFor(() => expect(screen.getByTestId('show-stage').dataset.transition).toBe('push'));
    expect(cmds()).toContainEqual({ cmd: 'show', slide: 1 });
    // Back: the previous slide with its effects done.
    fireEvent.keyDown(window, { key: 'ArrowLeft' });
    await waitFor(() => expect(screen.getByTestId('show-stage').dataset.slide).toBe('0'));
    expect(screen.getByTestId('show-stage').dataset.click).toBe('1');
    fireEvent.keyDown(window, { key: 'Escape' });
    await screen.findByTestId('slide-overlay');
  });

  it('inserts a chart from the dialog and edits the selected chart', async () => {
    const { p, cmds } = fakePlatform();
    render(<BlueSlideEditor platform={p} language="vi-VN" />);
    await openDeck(p);
    fireEvent.mouseDown(screen.getByRole('tab', { name: 'Chèn' }));
    fireEvent.click(await screen.findByText('Biểu đồ'));
    await screen.findByTestId('chart-dialog');
    fireEvent.change(screen.getByTestId('chart-kind'), { target: { value: 'line' } });
    fireEvent.change(screen.getByTestId('chart-title'), { target: { value: 'Doanh thu' } });
    fireEvent.change(screen.getByTestId('chart-cell-0-0'), { target: { value: '7,5' } });
    fireEvent.click(screen.getByTestId('chart-add-row'));
    fireEvent.click(screen.getByTestId('chart-ok'));
    await waitFor(() =>
      expect(cmds()).toContainEqual(
        expect.objectContaining({ cmd: 'insertChart', kind: 'line', title: 'Doanh thu' }),
      ),
    );
    const ins = cmds().find((c) => c.cmd === 'insertChart') as Extract<
      SlideCommand,
      { cmd: 'insertChart' }
    >;
    expect(ins.data?.categories).toEqual(['Mục 1', 'Mục 2', 'Mục 3', 'Mục 4', '5']);
    expect(ins.data?.series[0]!.values[0]).toBe(7.5);
    expect(ins.data?.series[0]!.values[4]).toBeNull();
    // The chart tab edits the selected chart.
    fireEvent.mouseDown(await screen.findByRole('tab', { name: 'Biểu đồ' }));
    fireEvent.click(await screen.findByText('Sửa biểu đồ'));
    await screen.findByTestId('chart-dialog');
    expect((screen.getByTestId('chart-title') as HTMLInputElement).value).toBe('Doanh thu');
    fireEvent.change(screen.getByTestId('chart-legend'), { target: { value: 'none' } });
    fireEvent.click(screen.getByTestId('chart-ok'));
    await waitFor(() =>
      expect(cmds()).toContainEqual(expect.objectContaining({ cmd: 'setChart', legend: 'none' })),
    );
  });

  it('exports the current slide and all slides as PNG', async () => {
    const proto = HTMLCanvasElement.prototype as unknown as {
      getContext: unknown;
      toBlob: unknown;
    };
    const saveCtx = proto.getContext;
    const saveBlob = proto.toBlob;
    const ctx = new Proxy({}, { get: () => () => ({ addColorStop: () => {} }) });
    proto.getContext = () => ctx;
    proto.toBlob = (cb: (b: Blob) => void) =>
      cb(new Blob([new Uint8Array([137, 80, 78, 71])], { type: 'image/png' }));
    try {
      const { p, saved } = fakePlatform();
      render(<BlueSlideEditor platform={p} language="vi-VN" />);
      await openDeck(p);
      fireEvent.click(screen.getByText('Xuất ảnh PNG'));
      await screen.findByTestId('export-png-dialog');
      fireEvent.click(screen.getByTestId('export-png-ok'));
      await waitFor(() => expect(saved).toContain('Báo cáo-1.png'));
      fireEvent.click(screen.getByText('Xuất ảnh PNG'));
      fireEvent.click(await screen.findByTestId('export-png-all'));
      fireEvent.change(screen.getByTestId('export-png-scale'), { target: { value: '1' } });
      fireEvent.click(screen.getByTestId('export-png-ok'));
      await waitFor(() => expect(saved).toContain('Báo cáo-png.zip'));
    } finally {
      proto.getContext = saveCtx;
      proto.toBlob = saveBlob;
    }
  });

  it('reorders slides by dragging in the slide sorter and plays slides in reading view', async () => {
    const { p, cmds, st } = fakePlatform();
    st.showAnims = true;
    render(<BlueSlideEditor platform={p} language="en-US" />);
    await openDeck(p, 'Open');
    fireEvent.mouseDown(screen.getByRole('tab', { name: 'View' }));
    fireEvent.click(await screen.findByText('Slide sorter'));
    const first = await screen.findByTestId('sorter-1');
    const third = screen.getByTestId('sorter-3');
    fireEvent.dragStart(first);
    fireEvent.dragOver(third);
    fireEvent.drop(third);
    await waitFor(() => expect(cmds()).toContainEqual({ cmd: 'moveSlide', from: 0, to: 2 }));
    fireEvent.click(screen.getByText('Reading view'));
    await screen.findByTestId('reading-view');
    expect(screen.getByTestId('reading-status').textContent).toBe('Slide 1 of 3');
    // Slide 1 has one click step: the first "next" plays it, the second changes slide.
    await waitFor(() => expect(cmds()).toContainEqual({ cmd: 'show', slide: 0 }));
    fireEvent.click(screen.getByTestId('reading-next'));
    fireEvent.click(screen.getByTestId('reading-next'));
    await waitFor(() =>
      expect(screen.getByTestId('reading-status').textContent).toBe('Slide 2 of 3'),
    );
  });
});
