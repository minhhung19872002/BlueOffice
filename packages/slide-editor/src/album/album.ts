/**
 * Photo album (SLIDE-ALBUM-001): several pictures become slides — one, two or
 * four pictures per slide, or one picture filling the slide — with optional
 * slide titles, captions from the file names and a frame shape.
 *
 * The album is built with the ordinary slide commands (new slide, insert
 * picture, resize / move, crop to shape, outline, text box), so every picture
 * is a real picture with embedded media and the edits are shared while
 * collaborating; the steps are then joined into one undo step (`mergeHistory`).
 */
import type {
  SlideBox,
  SlideCommand,
  SlideEditResult,
  SlideRect,
  SlideShapeInfo,
} from '@blueoffice/platform';

/** Pictures per slide; `fit`: one picture as large as the slide allows, no margins. */
export type AlbumLayout = 'fit' | 'one' | 'two' | 'four';
/** Frame of the pictures: plain rectangle, rounded corners, or a thin outline. */
export type AlbumFrame = 'rect' | 'rounded' | 'outline';

export interface AlbumPicture {
  name: string;
  bytes: Uint8Array;
}

export interface AlbumOptions {
  layout: AlbumLayout;
  /** Each slide keeps an (empty) title placeholder above the pictures. */
  titles: boolean;
  /** A caption under each picture: its file name without the extension. */
  captions: boolean;
  frame: AlbumFrame;
}

/** Picture file types an album takes. */
export const ALBUM_FILES = ['.png', '.jpg', '.jpeg'];

export const ALBUM_LAYOUTS: AlbumLayout[] = ['fit', 'one', 'two', 'four'];
export const ALBUM_FRAMES: AlbumFrame[] = ['rect', 'rounded', 'outline'];

/** Pictures on one slide of a layout. */
export function perSlide(layout: AlbumLayout): number {
  return layout === 'two' ? 2 : layout === 'four' ? 4 : 1;
}

/** `fit` has neither titles nor captions: the picture takes the whole slide. */
export function effectiveOptions(o: AlbumOptions): AlbumOptions {
  return o.layout === 'fit' ? { ...o, titles: false, captions: false } : o;
}

/** Caption of a picture: its file name without folder and extension. */
export function captionOf(name: string): string {
  const file = name.split(/[\\/]/).pop() ?? name;
  return file.replace(/\.[^.]+$/, '') || file;
}

/** Height of a caption line (points) and its gap to the picture. */
export const CAPTION_HEIGHT = 30;
const CAPTION_GAP = 4;
/** Outline of the `outline` frame. */
export const FRAME_LINE = { color: '404040', width: 2.25 };

/**
 * Cells of the pictures of one slide (points): the area below `top` (the
 * bottom of the title, when there is one) inside a 5 % margin, split into the
 * layout's grid with a 3 % gap. `fit`: the whole slide.
 */
export function albumCells(
  layout: AlbumLayout,
  slide: { width: number; height: number },
  top = 0,
): SlideRect[] {
  const { width: w, height: h } = slide;
  if (layout === 'fit') return [{ x: 0, y: 0, w, h }];
  const m = w * 0.05;
  const gap = w * 0.03;
  const y0 = Math.max(m, top + gap / 2);
  const area = { x: m, y: y0, w: w - 2 * m, h: Math.max(1, h - y0 - m) };
  const cols = layout === 'one' ? 1 : 2;
  const rows = layout === 'four' ? 2 : 1;
  const cw = (area.w - gap * (cols - 1)) / cols;
  const ch = (area.h - gap * (rows - 1)) / rows;
  const out: SlideRect[] = [];
  for (let r = 0; r < rows; r++)
    for (let c = 0; c < cols; c++)
      out.push({ x: area.x + c * (cw + gap), y: area.y + r * (ch + gap), w: cw, h: ch });
  return out;
}

/** Where a picture of size `w` × `h` goes in `cell`: as large as fits, centred, caption below. */
export function placePicture(
  size: { w: number; h: number },
  cell: SlideRect,
  caption: boolean,
): { picture: SlideRect; caption: SlideRect | null } {
  const room = caption ? Math.max(1, cell.h - CAPTION_HEIGHT - CAPTION_GAP) : cell.h;
  const k = Math.min(cell.w / Math.max(1, size.w), room / Math.max(1, size.h));
  const w = size.w * k;
  const h = size.h * k;
  const picture = { x: cell.x + (cell.w - w) / 2, y: cell.y + (room - h) / 2, w, h };
  return {
    picture,
    caption: caption
      ? { x: cell.x, y: picture.y + h + CAPTION_GAP, w: cell.w, h: CAPTION_HEIGHT }
      : null,
  };
}

/** What the album builder needs from the editor. */
export interface AlbumHost {
  run(cmd: SlideCommand): Promise<SlideEditResult | null>;
  insertPicture(bytes: Uint8Array, name: string): Promise<SlideEditResult | null>;
}

export interface AlbumResult {
  /** Slides added. */
  slides: number;
  /** Pictures placed. */
  pictures: number;
  /** Names of files that are not pictures the engine reads. */
  skipped: string[];
  /** First slide of the album (0-based), when one was added. */
  first: number | null;
}

const isTitle = (s: SlideShapeInfo) => s.placeholder === 'title' || s.placeholder === 'ctrTitle';

/** Layout for an album slide: title only (else any with a title) / blank (else any). */
export function albumLayoutIndex(
  layouts: { index: number; name: string; kind: string | null }[],
  titles: boolean,
): number | undefined {
  const by = (...kinds: string[]) => {
    for (const k of kinds) {
      const l = layouts.find((x) => x.kind === k);
      if (l) return l.index;
    }
    return undefined;
  };
  return titles ? by('titleOnly', 'obj', 'title') : by('blank', 'titleOnly', 'obj');
}

const ok = (r: SlideEditResult | null): r is SlideEditResult => !!r && !r.error;

/**
 * Build the album after the current slide. Every step is an ordinary command;
 * at the end they are one undo step. Files the engine cannot read as pictures
 * are skipped and reported; when no picture could be placed nothing is left behind.
 */
export async function buildAlbum(
  host: AlbumHost,
  pictures: AlbumPicture[],
  options: AlbumOptions,
  slide: { width: number; height: number },
): Promise<AlbumResult> {
  const o = effectiveOptions(options);
  const out: AlbumResult = { slides: 0, pictures: 0, skipped: [], first: null };
  const start = await host.run({ cmd: 'historyLen' });
  if (!ok(start)) return out;
  const since = start.history ?? 0;
  const layout = albumLayoutIndex(start.layouts, o.titles);
  const n = perSlide(o.layout);
  let cells: SlideRect[] = [];
  let used = n;
  try {
    for (const pic of pictures) {
      if (used >= n) {
        // A new slide: keep its title (when wanted), remove the other placeholders.
        const added = await host.run({
          cmd: 'addSlide',
          ...(layout !== undefined ? { layout } : {}),
        });
        if (!ok(added)) break;
        out.slides++;
        out.first ??= added.slide;
        const all = await host.run({ cmd: 'selectAll' });
        const shapes = ok(all) ? all.selection : [];
        const title = o.titles ? shapes.find(isTitle) : undefined;
        const extra = shapes.filter((s) => s !== title && s.placeholder !== null).map((s) => s.id);
        if (extra.length) {
          await host.run({ cmd: 'select', ids: extra });
          await host.run({ cmd: 'delete' });
        }
        await host.run({ cmd: 'select', ids: [] });
        const top = title ? title.rect.cy + title.rect.h / 2 : 0;
        cells = albumCells(o.layout, slide, top);
        used = 0;
      }
      const inserted = await host.insertPicture(pic.bytes, pic.name);
      if (!ok(inserted) || !inserted.bounds) {
        out.skipped.push(pic.name);
        continue;
      }
      const cell = cells[used]!;
      used++;
      out.pictures++;
      let box: SlideBox = inserted.bounds;
      const place = placePicture({ w: box.w, h: box.h }, cell, o.captions);
      const dw = place.picture.w - box.w;
      const dh = place.picture.h - box.h;
      if (Math.abs(dw) > 0.01 || Math.abs(dh) > 0.01) {
        const r = await host.run({
          cmd: 'transform',
          transform: { kind: 'resize', handle: 'se', dx: dw, dy: dh, keepAspect: true },
          snap: false,
        });
        if (ok(r) && r.bounds) box = r.bounds;
      }
      const dx = place.picture.x + place.picture.w / 2 - box.cx;
      const dy = place.picture.y + place.picture.h / 2 - box.cy;
      if (Math.abs(dx) > 0.01 || Math.abs(dy) > 0.01) {
        await host.run({ cmd: 'transform', transform: { kind: 'move', dx, dy }, snap: false });
      }
      if (o.frame === 'rounded') await host.run({ cmd: 'pictureCropShape', preset: 'roundRect' });
      else if (o.frame === 'outline') await host.run({ cmd: 'setLine', ...FRAME_LINE });
      if (place.caption) {
        await host.run({ cmd: 'insertTextBox', text: captionOf(pic.name), rect: place.caption });
        await host.run({ cmd: 'formatText', format: { kind: 'align', align: 'center' } });
      }
    }
  } finally {
    // Whatever happened, the steps so far are one undo step (and the batch ends).
    await host.run({ cmd: 'select', ids: [] });
    await host.run({ cmd: 'mergeHistory', since });
  }
  if (out.pictures === 0 && out.slides > 0) {
    // Only unreadable files: take the empty slide away again.
    await host.run({ cmd: 'undo' });
    out.slides = 0;
    out.first = null;
  } else if (out.first !== null) {
    await host.run({ cmd: 'goTo', slide: out.first });
  }
  return out;
}
