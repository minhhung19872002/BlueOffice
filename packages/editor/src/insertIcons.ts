/**
 * Inserting icons of the icon library (CORE-ICON-001) through the picture commands each
 * application already has:
 *
 * - BlueSlide stores the SVG itself with a PNG fallback drawn by the engine
 *   (`pictureInsertSvg`: `asvg:svgBlip` extension of the picture's blip).
 * - BlueDoc and BlueSheet store raster pictures, so the icon is rendered to PNG first
 *   (`svgToPng`): an inline picture in BlueDoc, a picture in the cell in BlueSheet.
 *
 * The icon's name becomes the alternative text of the picture everywhere.
 */
import type {
  EditCommand,
  EditResult,
  SheetCommand,
  SlideCommand,
  SlideEditResult,
} from '@blueoffice/platform';
import { svgToPng, toBase64, type IconPick } from '@blueoffice/ui';

/** Renders an SVG document to a square PNG (`svgToPng`; replaced in tests). */
export type IconRaster = (svg: string, size: number) => Promise<Uint8Array>;

/** Pixels of the picture BlueDoc sizes the icon by: 96 px at 96 dpi = 1 inch. */
export const DOC_ICON_BOX_PX = 96;
/** Pixels of the stored PNG (4 × the box, sharp when enlarged or printed). */
export const ICON_PNG_PX = 384;
/** Pitch of icons inserted together on a slide: the 72 pt icon plus a 12 pt gap (points). */
export const SLIDE_ICON_PITCH_PT = 84;
/** Icons per row when several are inserted together on a slide. */
export const SLIDE_ICONS_PER_ROW = 5;

/** The part of `DocController` the insertion needs. */
export interface DocIconTarget {
  run(cmd: EditCommand): Promise<EditResult | null>;
  insertImage(bytes: Uint8Array, name: string): Promise<EditResult | null>;
}

/**
 * Inserts the icons at the caret of a BlueDoc document as inline pictures, one inch
 * square, one undo step for all of them. The engine sizes a new picture by its pixels,
 * so a small PNG reserves the box and `pictureChange` then swaps in the sharp one (same
 * box) and sets the alternative text.
 */
export async function insertDocIcons(
  target: DocIconTarget,
  icons: readonly IconPick[],
  raster: IconRaster = svgToPng,
): Promise<void> {
  const depth = (await target.run({ cmd: 'historyLen' }))?.count ?? null;
  try {
    for (const icon of icons) {
      const box = await raster(icon.svg, DOC_ICON_BOX_PX);
      const sharp = await raster(icon.svg, ICON_PNG_PX);
      const r = await target.insertImage(box, `${icon.id}.png`);
      if (!r || r.error) throw new Error(r?.error ?? icon.id);
      const c = await target.run({ cmd: 'pictureChange', data: toBase64(sharp), name: icon.name });
      if (c?.error) throw new Error(c.error);
    }
  } finally {
    if (depth !== null) await target.run({ cmd: 'mergeHistory', since: depth });
  }
}

/** The part of `SlideController` the insertion needs. */
export interface SlideIconTarget {
  run(cmd: SlideCommand): Promise<SlideEditResult | null>;
}

/**
 * Inserts the icons on the current slide as SVG pictures (vector original + PNG fallback),
 * laid out in rows from the centre so that several icons do not hide each other; one undo
 * step for all of them. The last one stays selected.
 */
export async function insertSlideIcons(
  target: SlideIconTarget,
  icons: readonly IconPick[],
): Promise<void> {
  const depth = (await target.run({ cmd: 'historyLen' }))?.history ?? null;
  const utf8 = new TextEncoder();
  try {
    for (const [i, icon] of icons.entries()) {
      const r = await target.run({
        cmd: 'pictureInsertSvg',
        data: toBase64(utf8.encode(icon.svg)),
        name: icon.name,
      });
      if (!r || r.error) throw new Error(r?.error ?? icon.id);
      if (i > 0) {
        const dx = (i % SLIDE_ICONS_PER_ROW) * SLIDE_ICON_PITCH_PT;
        const dy = Math.floor(i / SLIDE_ICONS_PER_ROW) * SLIDE_ICON_PITCH_PT;
        await target.run({ cmd: 'transform', transform: { kind: 'move', dx, dy } });
      }
    }
  } finally {
    if (depth !== null) await target.run({ cmd: 'mergeHistory', since: depth });
  }
}

/**
 * Places the icons as pictures in cells: the first in (`row`, `col`), the next ones in the
 * cells to its right. `exec` runs a BlueSheet command and throws when it is refused.
 */
export async function insertSheetIcons(
  exec: (cmd: SheetCommand) => Promise<unknown>,
  at: { sheet: number; row: number; col: number },
  icons: readonly IconPick[],
  raster: IconRaster = svgToPng,
): Promise<void> {
  for (const [i, icon] of icons.entries()) {
    const png = await raster(icon.svg, ICON_PNG_PX);
    await exec({
      cmd: 'placeImageInCell',
      sheet: at.sheet,
      row: at.row,
      col: at.col + i,
      data: toBase64(png),
      ext: 'png',
      alt: icon.name,
    });
  }
}
