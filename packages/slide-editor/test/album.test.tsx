// @vitest-environment happy-dom
import { createI18n } from '@blueoffice/i18n';
import type {
  SlideBox,
  SlideCommand,
  SlideEditResult,
  SlideReuseSource,
  SlideShapeInfo,
} from '@blueoffice/platform';
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import {
  albumCells,
  albumLayoutIndex,
  buildAlbum,
  CAPTION_HEIGHT,
  captionOf,
  effectiveOptions,
  FRAME_LINE,
  perSlide,
  placePicture,
  type AlbumHost,
  type AlbumOptions,
  type AlbumPicture,
} from '../src/album/album';
import { AlbumDialog, moved, type AlbumRequest } from '../src/album/AlbumDialog';
import { OutlineDialog } from '../src/album/OutlineDialog';
import { ReusePane } from '../src/album/ReusePane';
import { deckFileName } from '../src/album/useAlbumTools';

/** SLIDE-ALBUM-001: photo album planner / builder / dialog, outline dialog, reuse pane. */

afterEach(cleanup);

const i18n = createI18n('vi-VN');
const t = (key: string, opts?: Record<string, unknown>) => i18n.t(key, opts ?? {}) as string;

const SLIDE = { width: 960, height: 540 };
const pic = (name: string): AlbumPicture => ({ name, bytes: new Uint8Array([1, 2, 3]) });
const opts = (o: Partial<AlbumOptions> = {}): AlbumOptions => ({
  layout: 'one',
  titles: false,
  captions: false,
  frame: 'rect',
  ...o,
});

describe('album planner', () => {
  it('counts pictures per slide and turns titles / captions off for fit-to-slide', () => {
    expect((['fit', 'one', 'two', 'four'] as const).map(perSlide)).toEqual([1, 1, 2, 4]);
    expect(effectiveOptions(opts({ layout: 'fit', titles: true, captions: true }))).toEqual(
      opts({ layout: 'fit' }),
    );
    expect(effectiveOptions(opts({ layout: 'two', titles: true }))).toEqual(
      opts({ layout: 'two', titles: true }),
    );
  });

  it('lays the cells out in a grid inside the margins, below the title', () => {
    expect(albumCells('fit', SLIDE)).toEqual([{ x: 0, y: 0, w: 960, h: 540 }]);
    // 5 % margin = 48, 3 % gap = 28.8.
    expect(albumCells('one', SLIDE)).toEqual([{ x: 48, y: 48, w: 864, h: 444 }]);
    const two = albumCells('two', SLIDE);
    expect(two).toHaveLength(2);
    expect(two[0]).toEqual({ x: 48, y: 48, w: 417.6, h: 444 });
    expect(two[1]!.x).toBeCloseTo(494.4);
    expect(two[1]!.x + two[1]!.w).toBeCloseTo(912);
    // Title ending at y = 120: the cells start half a gap below it.
    const four = albumCells('four', SLIDE, 120);
    expect(four).toHaveLength(4);
    expect(four[0]!.y).toBeCloseTo(134.4);
    expect(four[0]!.h).toBeCloseTo((540 - 134.4 - 48 - 28.8) / 2);
    expect(four[2]!.y).toBeCloseTo(four[0]!.y + four[0]!.h + 28.8);
    expect(four[3]!.x).toBeCloseTo(494.4);
    expect(four[3]!.y + four[3]!.h).toBeCloseTo(492);
  });

  it('fits a picture into its cell keeping its proportions, with room for a caption', () => {
    const cell = { x: 100, y: 50, w: 400, h: 300 };
    // Wide picture: limited by the width, centred vertically.
    expect(placePicture({ w: 800, h: 200 }, cell, false)).toEqual({
      picture: { x: 100, y: 150, w: 400, h: 100 },
      caption: null,
    });
    // Tall picture: limited by the height.
    expect(placePicture({ w: 100, h: 600 }, cell, false).picture).toEqual({
      x: 275,
      y: 50,
      w: 50,
      h: 300,
    });
    // With a caption the picture leaves the caption line (and a 4 pt gap) free below it.
    const withCaption = placePicture({ w: 100, h: 600 }, cell, true);
    const room = 300 - CAPTION_HEIGHT - 4;
    expect(withCaption.picture.h).toBeCloseTo(room);
    expect(withCaption.caption).toEqual({
      x: 100,
      y: withCaption.picture.y + room + 4,
      w: 400,
      h: CAPTION_HEIGHT,
    });
    expect(withCaption.caption!.y + CAPTION_HEIGHT).toBeLessThanOrEqual(350 + 1e-9);
  });

  it('names captions after the file and picks the slide layout', () => {
    expect(captionOf('Bãi biển.JPG')).toBe('Bãi biển');
    expect(captionOf('C:\\anh\\he.2024.png')).toBe('he.2024');
    expect(captionOf('.png')).toBe('.png');
    const layouts = [
      { index: 0, name: 'Tiêu đề', kind: 'title' },
      { index: 1, name: 'Nội dung', kind: 'obj' },
      { index: 2, name: 'Trống', kind: 'blank' },
    ];
    expect(albumLayoutIndex(layouts, false)).toBe(2);
    expect(albumLayoutIndex(layouts, true)).toBe(1);
    expect(
      albumLayoutIndex([...layouts, { index: 3, name: 'Chỉ tiêu đề', kind: 'titleOnly' }], true),
    ).toBe(3);
    expect(albumLayoutIndex([{ index: 0, name: 'x', kind: null }], true)).toBeUndefined();
    expect(deckFileName('Hè 2024: biển/núi')).toBe('Hè 2024 biển núi.pptx');
    expect(deckFileName('  ')).toBe('album.pptx');
  });
});

/**
 * A stand-in for the engine: slides with shapes; inserted pictures are centred at 400 × 300
 * (like the engine centres a new picture); resizing by the bottom-right handle keeps the
 * top-left corner.
 */
function fakeEngine(options: { bad?: string[]; titleLayout?: boolean } = {}) {
  interface Shape extends SlideShapeInfo {
    line?: unknown;
    crop?: string;
    align?: string;
  }
  const slides: Shape[][] = [[]];
  let current = 0;
  let selected: number[] = [];
  let nextId = 10;
  const history: string[] = [];
  const log: SlideCommand[] = [];
  const box = (cx: number, cy: number, w: number, h: number): SlideBox => ({
    cx,
    cy,
    w,
    h,
    rot: 0,
  });
  const shape = (kind: Shape['kind'], rect: SlideBox, more: Partial<Shape> = {}): Shape => ({
    id: nextId++,
    name: '',
    kind,
    rect,
    text: null,
    placeholder: null,
    lockAspect: kind === 'picture',
    editableText: kind === 'shape',
    ...more,
  });
  const result = (more: Partial<SlideEditResult> = {}): SlideEditResult => {
    const sel = slides[current]!.filter((s) => selected.includes(s.id));
    return {
      slide: current,
      selection: sel,
      bounds: sel.length === 1 ? sel[0]!.rect : null,
      layouts: [
        { index: 0, name: 'Tiêu đề', kind: 'title' },
        { index: 1, name: 'Nội dung', kind: 'obj' },
        { index: 2, name: 'Trống', kind: 'blank' },
      ],
      history: history.length,
      ...more,
    } as SlideEditResult;
  };
  const host: AlbumHost = {
    run: async (cmd) => {
      log.push(cmd);
      const sel = () => slides[current]!.filter((s) => selected.includes(s.id));
      switch (cmd.cmd) {
        case 'historyLen':
        case 'goTo':
          if (cmd.cmd === 'goTo') current = cmd.slide;
          break;
        case 'addSlide': {
          const shapes: Shape[] = [];
          if (cmd.layout === 1) {
            shapes.push(shape('shape', box(480, 80, 864, 80), { placeholder: 'title' }));
            shapes.push(shape('shape', box(480, 320, 864, 340), { placeholder: 'body' }));
          }
          slides.splice(current + 1, 0, shapes);
          current++;
          selected = [];
          history.push('addSlide');
          break;
        }
        case 'selectAll':
          selected = slides[current]!.map((s) => s.id);
          break;
        case 'select':
          selected = cmd.ids;
          break;
        case 'delete':
          slides[current] = slides[current]!.filter((s) => !selected.includes(s.id));
          selected = [];
          history.push('delete');
          break;
        case 'transform': {
          const tr = cmd.transform;
          for (const s of sel()) {
            if (tr.kind === 'move')
              s.rect = { ...s.rect, cx: s.rect.cx + tr.dx, cy: s.rect.cy + tr.dy };
            else if (tr.kind === 'resize')
              s.rect = box(
                s.rect.cx + tr.dx / 2,
                s.rect.cy + tr.dy / 2,
                s.rect.w + tr.dx,
                s.rect.h + tr.dy,
              );
          }
          history.push('transform');
          break;
        }
        case 'pictureCropShape':
          for (const s of sel()) s.crop = cmd.preset;
          history.push('crop');
          break;
        case 'setLine':
          for (const s of sel()) s.line = { color: cmd.color, width: cmd.width };
          history.push('line');
          break;
        case 'insertTextBox': {
          const r = cmd.rect!;
          const s = shape('shape', box(r.x + r.w / 2, r.y + r.h / 2, r.w, r.h), {
            text: cmd.text ?? '',
          });
          slides[current]!.push(s);
          selected = [s.id];
          history.push('textBox');
          break;
        }
        case 'formatText':
          if (cmd.format.kind === 'align') for (const s of sel()) s.align = cmd.format.align;
          history.push('format');
          break;
        case 'mergeHistory':
          if (history.length > cmd.since + 1) history.splice(cmd.since, history.length, 'album');
          break;
        case 'undo': {
          const last = history.pop();
          // The only thing ever undone here: an album of one empty slide.
          if (last) {
            slides.splice(current, 1);
            current = Math.max(0, current - 1);
          }
          break;
        }
        default:
          throw new Error(`unexpected command ${cmd.cmd}`);
      }
      return result();
    },
    insertPicture: async (_bytes, name) => {
      if (options.bad?.includes(name)) return result({ error: 'badImage' });
      const s = shape('picture', box(480, 270, 400, 300), { name });
      slides[current]!.push(s);
      selected = [s.id];
      history.push('picture');
      return result();
    },
  };
  return { host, slides, log, history, current: () => current };
}

describe('album builder', () => {
  it('puts two pictures per slide into their cells; the whole album is one undo step', async () => {
    const e = fakeEngine();
    e.history.push('earlier edit');
    const r = await buildAlbum(
      e.host,
      [pic('a.png'), pic('b.png'), pic('c.png')],
      opts({ layout: 'two' }),
      SLIDE,
    );
    expect(r).toEqual({ slides: 2, pictures: 3, skipped: [], first: 1 });
    expect(e.slides.map((s) => s.length)).toEqual([0, 2, 1]);
    // Cells of 417.6 × 444 at x = 48 and 494.4; a 400 × 300 picture grows to 417.6 × 313.2.
    const [a, b] = e.slides[1]!;
    expect(a!.rect.w).toBeCloseTo(417.6);
    expect(a!.rect.h).toBeCloseTo(313.2);
    expect(a!.rect.cx).toBeCloseTo(48 + 417.6 / 2);
    expect(a!.rect.cy).toBeCloseTo(270);
    expect(b!.rect.cx).toBeCloseTo(494.4 + 417.6 / 2);
    expect(e.slides[2]![0]!.rect.cx).toBeCloseTo(48 + 417.6 / 2);
    // Blank layout, no snapping, and the steps joined after the earlier edit.
    expect(e.log.filter((c) => c.cmd === 'addSlide')).toEqual([
      { cmd: 'addSlide', layout: 2 },
      { cmd: 'addSlide', layout: 2 },
    ]);
    for (const c of e.log) if (c.cmd === 'transform') expect(c.snap).toBe(false);
    expect(e.history).toEqual(['earlier edit', 'album']);
    // It ends on the album's first slide.
    expect(e.log.at(-1)).toEqual({ cmd: 'goTo', slide: 1 });
    expect(e.current()).toBe(1);
  });

  it('keeps the slide title, drops other placeholders, adds captions and frames', async () => {
    const e = fakeEngine();
    const r = await buildAlbum(
      e.host,
      [pic('Bãi biển.jpg')],
      opts({ titles: true, captions: true, frame: 'outline' }),
      SLIDE,
    );
    expect(r.slides).toBe(1);
    const shapes = e.slides[1]!;
    // Title kept, body placeholder removed, picture and caption added.
    expect(shapes.map((s) => s.placeholder ?? s.kind)).toEqual(['title', 'picture', 'shape']);
    const [title, picture, caption] = shapes as [
      (typeof shapes)[0],
      (typeof shapes)[0],
      (typeof shapes)[0],
    ];
    const titleBottom = title.rect.cy + title.rect.h / 2;
    expect(titleBottom).toBe(120);
    // Below the title, inside the bottom margin, caption right under the picture.
    expect(picture.rect.cy - picture.rect.h / 2).toBeGreaterThanOrEqual(titleBottom);
    expect(picture.rect.w / picture.rect.h).toBeCloseTo(400 / 300);
    expect(caption.text).toBe('Bãi biển');
    expect(caption.rect.cy - caption.rect.h / 2).toBeCloseTo(
      picture.rect.cy + picture.rect.h / 2 + 4,
    );
    expect(caption.rect.cy + caption.rect.h / 2).toBeLessThanOrEqual(540 - 48 + 1e-6);
    expect((caption as { align?: string }).align).toBe('center');
    expect((picture as { line?: unknown }).line).toEqual(FRAME_LINE);
    expect(e.history).toEqual(['album']);
  });

  it('fills the slide for fit-to-slide and rounds corners on request', async () => {
    const e = fakeEngine();
    await buildAlbum(
      e.host,
      [pic('a.png')],
      opts({ layout: 'fit', titles: true, captions: true, frame: 'rounded' }),
      SLIDE,
    );
    const [p] = e.slides[1]!;
    // 400 × 300 in 960 × 540: limited by the height → 720 × 540, centred.
    expect(p!.rect).toEqual({ cx: 480, cy: 270, w: 720, h: 540, rot: 0 });
    expect((p as { crop?: string }).crop).toBe('roundRect');
    expect(e.slides[1]).toHaveLength(1);
    expect(e.log.some((c) => c.cmd === 'insertTextBox')).toBe(false);
  });

  it('skips files that are not pictures and leaves nothing behind when none is', async () => {
    const e = fakeEngine({ bad: ['hong.png', 'x.png'] });
    const r = await buildAlbum(e.host, [pic('hong.png'), pic('ok.png')], opts(), SLIDE);
    expect(r).toEqual({ slides: 1, pictures: 1, skipped: ['hong.png'], first: 1 });
    expect(e.slides[1]).toHaveLength(1);
    const none = fakeEngine({ bad: ['x.png'] });
    const r2 = await buildAlbum(none.host, [pic('x.png')], opts(), SLIDE);
    expect(r2).toEqual({ slides: 0, pictures: 0, skipped: ['x.png'], first: null });
    expect(none.slides).toHaveLength(1);
    expect(none.history).toEqual([]);
  });
});

describe('photo album dialog', () => {
  const open = (files: AlbumPicture[][], more: { canAppend?: boolean; unsaved?: boolean } = {}) => {
    const onCreate = vi.fn<(r: AlbumRequest) => void>();
    const onClose = vi.fn();
    const pick = vi.fn(async () => files.shift() ?? []);
    render(
      <AlbumDialog
        t={t}
        pick={pick}
        canAppend={more.canAppend ?? true}
        unsaved={more.unsaved ?? false}
        onClose={onClose}
        onCreate={onCreate}
      />,
    );
    return { onCreate, onClose, pick };
  };
  const names = () =>
    screen.queryAllByTestId(/^album-item-\d+$/).map((el) => el.textContent?.replace(/[↑↓×]/g, ''));

  it('moves a list item', () => {
    expect(moved(['a', 'b', 'c'], 0, 2)).toEqual(['b', 'c', 'a']);
    expect(moved(['a', 'b', 'c'], 2, 1)).toEqual(['a', 'c', 'b']);
    const same = ['a'];
    expect(moved(same, 0, 5)).toBe(same);
  });

  it('adds pictures, reorders and removes them, then creates the album', async () => {
    const { onCreate, onClose, pick } = open([[pic('a.png'), pic('b.png')], [pic('c.png')]]);
    expect(screen.getByRole('dialog', { name: 'Album ảnh' })).toBeTruthy();
    const create = screen.getByTestId('album-create') as HTMLButtonElement;
    expect(create.disabled).toBe(true);
    expect(screen.getByTestId('album-summary').textContent).toBe('Chưa có ảnh nào.');
    fireEvent.click(screen.getByTestId('album-add'));
    await waitFor(() => expect(names()).toEqual(['1. a.png', '2. b.png']));
    fireEvent.click(screen.getByTestId('album-add'));
    await waitFor(() => expect(names()).toEqual(['1. a.png', '2. b.png', '3. c.png']));
    expect(pick).toHaveBeenCalledTimes(2);
    expect(screen.getByTestId('album-summary').textContent).toBe('3 ảnh trên 3 slide');
    // The first cannot go up, the last cannot go down.
    expect(
      (screen.getByRole('button', { name: 'Đưa a.png lên trên' }) as HTMLButtonElement).disabled,
    ).toBe(true);
    expect(
      (screen.getByRole('button', { name: 'Đưa c.png xuống dưới' }) as HTMLButtonElement).disabled,
    ).toBe(true);
    fireEvent.click(screen.getByRole('button', { name: 'Đưa c.png lên trên' }));
    expect(names()).toEqual(['1. a.png', '2. c.png', '3. b.png']);
    // The moved row keeps the focus, so it can be moved again from the keyboard.
    await waitFor(() =>
      expect(document.activeElement?.getAttribute('aria-label')).toBe('Đưa c.png lên trên'),
    );
    // Alt+↓ on a row moves it down.
    fireEvent.keyDown(screen.getByRole('button', { name: 'Đưa a.png xuống dưới' }), {
      key: 'ArrowDown',
      altKey: true,
    });
    expect(names()).toEqual(['1. c.png', '2. a.png', '3. b.png']);
    fireEvent.click(screen.getByRole('button', { name: 'Bỏ a.png' }));
    expect(names()).toEqual(['1. c.png', '2. b.png']);
    fireEvent.change(screen.getByTestId('album-layout'), { target: { value: 'two' } });
    expect(screen.getByTestId('album-summary').textContent).toBe('2 ảnh trên 1 slide');
    fireEvent.change(screen.getByTestId('album-frame'), { target: { value: 'rounded' } });
    fireEvent.click(screen.getByTestId('album-captions'));
    fireEvent.click(create);
    expect(onCreate).toHaveBeenCalledTimes(1);
    const req = onCreate.mock.calls[0]![0];
    expect(req.pictures.map((p) => p.name)).toEqual(['c.png', 'b.png']);
    expect(req.options).toEqual({ layout: 'two', titles: true, captions: true, frame: 'rounded' });
    expect(req.target).toBe('append');
    expect(onClose).toHaveBeenCalled();
  });

  it('turns titles and captions off for fit-to-slide', async () => {
    const { onCreate } = open([[pic('a.png')]]);
    fireEvent.click(screen.getByTestId('album-add'));
    await waitFor(() => expect(names()).toHaveLength(1));
    fireEvent.click(screen.getByTestId('album-captions'));
    fireEvent.change(screen.getByTestId('album-layout'), { target: { value: 'fit' } });
    const titles = screen.getByTestId('album-titles') as HTMLInputElement;
    const captions = screen.getByTestId('album-captions') as HTMLInputElement;
    expect([titles.disabled, titles.checked, captions.disabled, captions.checked]).toEqual([
      true,
      false,
      true,
      false,
    ]);
    fireEvent.click(screen.getByTestId('album-create'));
    expect(onCreate.mock.calls[0]![0].options).toEqual({
      layout: 'fit',
      titles: false,
      captions: false,
      frame: 'rect',
    });
  });

  it('starts a new presentation when none is open, and warns about unsaved changes', async () => {
    const first = open([[pic('a.png')]], { canAppend: false });
    expect((screen.getByTestId('album-target-append') as HTMLInputElement).disabled).toBe(true);
    expect((screen.getByTestId('album-target-new') as HTMLInputElement).checked).toBe(true);
    expect((screen.getByTestId('album-title') as HTMLInputElement).value).toBe('Album ảnh');
    expect(screen.queryByRole('note')).toBeNull();
    fireEvent.click(screen.getByTestId('album-add'));
    await waitFor(() => expect(names()).toHaveLength(1));
    fireEvent.change(screen.getByTestId('album-title'), { target: { value: ' Hè 2024 ' } });
    fireEvent.click(screen.getByTestId('album-create'));
    expect(first.onCreate.mock.calls[0]![0]).toMatchObject({ target: 'new', title: 'Hè 2024' });
    cleanup();
    open([], { canAppend: true, unsaved: true });
    expect(screen.queryByTestId('album-title')).toBeNull();
    fireEvent.click(screen.getByTestId('album-target-new'));
    expect(screen.getByRole('note').textContent).toContain('chưa lưu');
  });

  it('closes on Cancel and Escape without creating anything', () => {
    const { onCreate, onClose } = open([]);
    fireEvent.keyDown(screen.getByRole('dialog'), { key: 'Escape' });
    expect(onClose).toHaveBeenCalledTimes(1);
    fireEvent.click(screen.getByRole('button', { name: 'Huỷ' }));
    expect(onClose).toHaveBeenCalledTimes(2);
    expect(onCreate).not.toHaveBeenCalled();
  });
});

describe('slides from outline dialog', () => {
  it('previews the slides and adds them', () => {
    const onInsert = vi.fn();
    const onClose = vi.fn();
    const entries = [
      { level: 0, text: 'Tổng quan' },
      { level: 1, text: 'Phạm vi' },
      { level: 2, text: 'Chi tiết' },
      { level: 0, text: 'Thiết kế' },
    ];
    render(
      <OutlineDialog
        t={t}
        name="dan-y.docx"
        entries={entries}
        onClose={onClose}
        onInsert={onInsert}
      />,
    );
    expect(screen.getByRole('dialog', { name: 'Slide từ dàn ý' })).toBeTruthy();
    expect(screen.getByText('Tệp: dan-y.docx')).toBeTruthy();
    const preview = screen.getByTestId('outline-import-preview');
    expect(preview.querySelectorAll('li')).toHaveLength(2);
    expect(preview.textContent).toContain('1. Tổng quan');
    expect(preview.textContent).toContain('• Chi tiết');
    const insert = screen.getByTestId('outline-import-insert');
    expect(insert.textContent).toBe('Thêm 2 slide');
    fireEvent.click(insert);
    expect(onInsert).toHaveBeenCalledWith(entries);
    expect(onClose).toHaveBeenCalled();
  });

  it('says so when the file is unreadable or empty', () => {
    const { unmount } = render(
      <OutlineDialog t={t} name="x.docx" entries={null} onClose={vi.fn()} onInsert={vi.fn()} />,
    );
    expect(screen.getByRole('alert').textContent).toBe('Không đọc được tệp này dưới dạng văn bản.');
    expect((screen.getByTestId('outline-import-insert') as HTMLButtonElement).disabled).toBe(true);
    unmount();
    render(<OutlineDialog t={t} name="x.txt" entries={[]} onClose={vi.fn()} onInsert={vi.fn()} />);
    expect(screen.getByRole('status').textContent).toBe('Tệp này không có nội dung để tạo slide.');
    expect((screen.getByTestId('outline-import-insert') as HTMLButtonElement).disabled).toBe(true);
  });
});

describe('reuse slides pane', () => {
  const source: SlideReuseSource = {
    name: 'nguon.pptx',
    slides: [{ title: 'Mở đầu', png: 'AAAA' }, { title: '' }, { title: 'Kết luận', png: 'BBBB' }],
  };
  const open = (more: Partial<Parameters<typeof ReusePane>[0]> = {}) => {
    const onInsert = vi.fn();
    const onOpen = vi.fn();
    const onClose = vi.fn();
    render(
      <ReusePane
        t={t}
        source={source}
        busy={false}
        error={false}
        canInsert
        onOpen={onOpen}
        onInsert={onInsert}
        onClose={onClose}
        {...more}
      />,
    );
    return { onInsert, onOpen, onClose };
  };
  const selected = () =>
    screen.getAllByRole('option').map((o) => o.getAttribute('aria-selected') === 'true');

  it('asks for a presentation first', () => {
    const { onOpen } = open({ source: null });
    expect(screen.getByRole('complementary', { name: 'Dùng lại slide' })).toBeTruthy();
    expect(screen.getByText('Mở một bản trình chiếu khác để chọn slide từ đó.')).toBeTruthy();
    expect(screen.queryByRole('listbox')).toBeNull();
    fireEvent.click(screen.getByTestId('reuse-open'));
    expect(onOpen).toHaveBeenCalled();
  });

  it('shows reading and error states', () => {
    open({ source: null, busy: true });
    expect(screen.getByRole('status').textContent).toBe('Đang đọc bản trình chiếu…');
    expect((screen.getByTestId('reuse-open') as HTMLButtonElement).disabled).toBe(true);
    cleanup();
    open({ source: null, error: true });
    expect(screen.getByRole('alert').textContent).toBe(
      'Không đọc được tệp này dưới dạng bản trình chiếu.',
    );
  });

  it('lists the slides with thumbnails and inserts the selected ones in deck order', () => {
    const { onInsert } = open();
    expect(screen.getByTestId('reuse-source').textContent).toBe('nguon.pptx — 3 slide');
    const options = screen.getAllByRole('option');
    expect(options).toHaveLength(3);
    expect(options[0]!.textContent).toBe('Slide 1Mở đầu');
    expect(options[0]!.querySelector('img')?.getAttribute('src')).toBe(
      'data:image/png;base64,AAAA',
    );
    expect(options[1]!.querySelector('img')).toBeNull();
    const insert = screen.getByTestId('reuse-insert') as HTMLButtonElement;
    expect(insert.disabled).toBe(true);
    fireEvent.click(options[2]!);
    fireEvent.click(options[0]!);
    expect(selected()).toEqual([true, false, true]);
    expect(insert.textContent).toBe('Chèn slide đã chọn (2)');
    fireEvent.click(insert);
    expect(onInsert).toHaveBeenLastCalledWith([0, 2], false);
    // The selection is cleared after inserting.
    expect(selected()).toEqual([false, false, false]);
    // Keep source formatting, all slides.
    fireEvent.click(screen.getByTestId('reuse-keep-source'));
    fireEvent.click(screen.getByTestId('reuse-insert-all'));
    expect(onInsert).toHaveBeenLastCalledWith([0, 1, 2], true);
    // A double click inserts that slide.
    fireEvent.doubleClick(options[1]!);
    expect(onInsert).toHaveBeenLastCalledWith([1], true);
  });

  it('works from the keyboard: arrows, Space, Ctrl+A, Enter, Escape', () => {
    const { onInsert, onClose } = open();
    const list = screen.getByRole('listbox', { name: 'Slide của bản trình chiếu kia' });
    const options = screen.getAllByRole('option');
    // One tab stop: the focused slide.
    expect(options.map((o) => o.tabIndex)).toEqual([0, -1, -1]);
    options[0]!.focus();
    fireEvent.keyDown(list, { key: 'ArrowDown' });
    expect(document.activeElement).toBe(options[1]);
    expect(options.map((o) => o.tabIndex)).toEqual([-1, 0, -1]);
    fireEvent.keyDown(list, { key: ' ' });
    expect(selected()).toEqual([false, true, false]);
    fireEvent.keyDown(list, { key: 'End' });
    expect(document.activeElement).toBe(options[2]);
    fireEvent.keyDown(list, { key: 'ArrowDown' });
    expect(document.activeElement).toBe(options[2]);
    fireEvent.keyDown(list, { key: ' ' });
    fireEvent.keyDown(list, { key: 'Enter' });
    expect(onInsert).toHaveBeenLastCalledWith([1, 2], false);
    // Nothing selected: Enter inserts the focused slide.
    fireEvent.keyDown(list, { key: 'Home' });
    fireEvent.keyDown(list, { key: 'Enter' });
    expect(onInsert).toHaveBeenLastCalledWith([0], false);
    fireEvent.keyDown(list, { key: 'a', ctrlKey: true });
    expect(selected()).toEqual([true, true, true]);
    fireEvent.keyDown(list, { key: 'Escape' });
    expect(onClose).toHaveBeenCalled();
  });

  it('keeps plain keys inside but lets shortcuts (save, undo) reach the editor', () => {
    open();
    const seen: string[] = [];
    const onKey = (e: KeyboardEvent) => seen.push(`${e.ctrlKey ? 'Ctrl+' : ''}${e.key}`);
    window.addEventListener('keydown', onKey);
    try {
      const list = screen.getByRole('listbox');
      fireEvent.keyDown(list, { key: 'Delete' });
      fireEvent.keyDown(list, { key: 'ArrowDown' });
      fireEvent.keyDown(list, { key: 'a', ctrlKey: true });
      fireEvent.keyDown(list, { key: 's', ctrlKey: true });
      fireEvent.keyDown(screen.getByTestId('reuse-open'), { key: 'z', ctrlKey: true });
      expect(seen).toEqual(['Ctrl+s', 'Ctrl+z']);
    } finally {
      window.removeEventListener('keydown', onKey);
    }
  });

  it('cannot insert without an open presentation', () => {
    const { onInsert } = open({ canInsert: false });
    fireEvent.click(screen.getAllByRole('option')[0]!);
    expect((screen.getByTestId('reuse-insert') as HTMLButtonElement).disabled).toBe(true);
    expect((screen.getByTestId('reuse-insert-all') as HTMLButtonElement).disabled).toBe(true);
    fireEvent.doubleClick(screen.getAllByRole('option')[0]!);
    expect(onInsert).not.toHaveBeenCalled();
  });
});
