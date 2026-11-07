// @vitest-environment happy-dom
import { act, cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import type {
  OpenedFile,
  Platform,
  SlideCommand,
  SlideEditResult,
  SlideShapeInfo,
} from '@blueoffice/platform';
import {
  AudienceLink,
  BlueSlideEditor,
  fit,
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
    kind: 'shape',
    rect: { cx: 480, cy: 100, w: 600, h: 80, rot: 0 },
    text: st.text,
    placeholder: 'title',
    lockAspect: false,
    editableText: true,
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
