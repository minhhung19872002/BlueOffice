/**
 * BlueSheet checkboxes in cells, pictures placed in cells and `IMAGE` pictures
 * (SHEET-CELL-001). The engine stores and draws them (`bo_engine::sheet::cell_media_cmd`);
 * this module holds the editor side:
 *
 * - commands: Insert › Checkbox, Insert › Picture in cell (from a file), alternative text,
 *   remove checkbox; Space and a click on the box toggle (one undo step), Delete removes the
 *   values first and the control on a second press;
 * - `IMAGE` pictures: after each render the editor asks the engine which URLs it needs,
 *   fetches them through `Platform.images` and hands the bytes back;
 * - the media source the grid canvas uses to draw pictures of the display list.
 */
import { useTranslation } from '@blueoffice/i18n';
import type {
  CellMediaState,
  RemoteImages,
  SheetCommand,
  SheetHost,
  SheetRange,
} from '@blueoffice/platform';
import { Dialog } from '@blueoffice/ui';
import { useState } from 'react';
import editorStyles from './SheetEditor.module.css';

/** A rectangle in points (engine coordinates). */
export interface PtRect {
  x: number;
  y: number;
  w: number;
  h: number;
}

/** File types offered by "Picture in cell". */
export const PICTURE_TYPES = ['.png', '.jpg', '.jpeg', '.gif', '.bmp'];

/**
 * Box of the checkbox drawn in a cell: a square of `min(height − 4, 12)` points (at least 4)
 * centred in the cell — the same rule as `bo_sheet::cell_image::checkbox_rect`.
 */
export function checkboxBox(cell: PtRect): PtRect {
  const s = Math.min(Math.max(cell.h - 4, 4), 12, Math.max(cell.w, 0));
  return { x: cell.x + (cell.w - s) / 2, y: cell.y + (cell.h - s) / 2, w: s, h: s };
}

/** Is the point on the checkbox of `cell` (2 pt tolerance, as the engine)? */
export function checkboxHit(cell: PtRect, x: number, y: number): boolean {
  const b = checkboxBox(cell);
  const t = 2;
  return x >= b.x - t && x <= b.x + b.w + t && y >= b.y - t && y <= b.y + b.h + t;
}

/** Base64 of bytes (chunked so large pictures do not overflow the call stack). */
export function toBase64(bytes: Uint8Array): string {
  let s = '';
  for (let i = 0; i < bytes.length; i += 0x8000) {
    s += String.fromCharCode(...bytes.subarray(i, i + 0x8000));
  }
  return btoa(s);
}

/** Bytes of base64 text. */
export function fromBase64(b64: string): Uint8Array {
  const s = atob(b64);
  const out = new Uint8Array(s.length);
  for (let i = 0; i < s.length; i++) out[i] = s.charCodeAt(i);
  return out;
}

/** Lower-case extension of a file name (`png` when there is none). */
export function extOf(name: string): string {
  const m = /\.([A-Za-z0-9]+)$/.exec(name);
  return m ? m[1]!.toLowerCase() : 'png';
}

/** Where the grid canvas gets the bytes of `image` commands (`cellimg:…`, `url:…`). */
export function sheetMediaSource(host: Pick<SheetHost, 'exec'>): {
  getMedia(id: string): Promise<Uint8Array>;
} {
  return {
    async getMedia(id: string) {
      const b64 = await host.exec<string | null>({ cmd: 'cellMedia', id });
      return typeof b64 === 'string' && b64 ? fromBase64(b64) : new Uint8Array();
    },
  };
}

/**
 * Fetches the pictures `IMAGE` formulas need and hands them to the engine. Without a
 * `RemoteImages` service every request is reported blocked. Resolves `true` when the engine
 * got anything (the grid should be drawn again).
 */
export async function loadRequestedImages(
  host: Pick<SheetHost, 'exec'>,
  images: RemoteImages | undefined,
): Promise<boolean> {
  const got = await host.exec<string[]>({ cmd: 'imageRequests' });
  const urls = Array.isArray(got) ? got.filter((u): u is string => typeof u === 'string') : [];
  await Promise.all(
    urls.map(async (url) => {
      const r = images ? await images.fetchImage(url) : ({ ok: false, error: 'blocked' } as const);
      const cmd: SheetCommand = r.ok
        ? { cmd: 'imageLoaded', url, data: toBase64(r.bytes) }
        : { cmd: 'imageLoaded', url, error: r.error };
      await host.exec(cmd);
    }),
  );
  return urls.length > 0;
}

/** Checkbox / picture of a cell. */
export function mediaState(
  host: Pick<SheetHost, 'exec'>,
  sheet: number,
  row: number,
  col: number,
): Promise<CellMediaState | null> {
  return host.exec<CellMediaState>({ cmd: 'cellMediaState', sheet, row, col });
}

/**
 * A click at (`x`, `y`) points on cell (`row`, `col`): toggles its checkbox when the click is
 * on the box. Resolves `true` when it toggled.
 */
export async function clickCheckbox(
  host: Pick<SheetHost, 'exec'>,
  exec: (cmd: SheetCommand) => Promise<unknown>,
  sheet: number,
  row: number,
  col: number,
  cell: PtRect | null,
  x: number,
  y: number,
): Promise<boolean> {
  if (!cell || !checkboxHit(cell, x, y)) return false;
  const st = await mediaState(host, sheet, row, col);
  if (!st?.checkbox) return false;
  await exec({
    cmd: 'toggleCheckbox',
    sheet,
    range: { r0: row, c0: col, r1: row, c1: col },
    row,
    col,
  });
  return true;
}

/** The registry's `add` as the editor passes it in. */
export type AddCommand<S> = (
  id: string,
  run: (s: S) => Promise<unknown>,
  extra?: Partial<{ shortcut: string; isEnabled: (s: S) => boolean }>,
) => void;

/** What the commands need from the editor. */
export interface CellMediaDeps {
  exec: (cmd: SheetCommand) => Promise<unknown>;
  /** Pick a picture file (the platform's file dialog). */
  pickFile: (accept: string[]) => Promise<{ name: string; bytes: Uint8Array } | null>;
  /** Open the alternative-text dialog for the picture of a cell. */
  openAltText: (d: AltTextTarget) => void;
  /** Report an error to the user. */
  fail: (e: unknown) => void;
}

/** The picture whose alternative text is edited. */
export interface AltTextTarget {
  sheet: number;
  row: number;
  col: number;
  alt: string;
  decorative: boolean;
}

/** Editor state the commands read. */
export interface CellMediaEditorState {
  host: Pick<SheetHost, 'exec'>;
  hasWorkbook: boolean;
  sheet: number;
  active: { row: number; col: number };
  range: SheetRange;
}

/**
 * Registers the commands: `insert.checkbox`, `cell.removeCheckbox`, `cell.toggleCheckbox`
 * (Space, handled by the editor only on checkbox cells), `insert.pictureInCell`,
 * `cell.pictureAltText`.
 */
export function registerCellMediaCommands<S>(
  add: AddCommand<S>,
  view: (s: S) => CellMediaEditorState,
  deps: CellMediaDeps,
): void {
  const has = { isEnabled: (s: S) => view(s).hasWorkbook };
  add(
    'insert.checkbox',
    (s) => {
      const v = view(s);
      return deps.exec({ cmd: 'insertCheckbox', sheet: v.sheet, range: v.range });
    },
    has,
  );
  add(
    'cell.removeCheckbox',
    (s) => {
      const v = view(s);
      return deps.exec({ cmd: 'removeCheckbox', sheet: v.sheet, range: v.range });
    },
    has,
  );
  add(
    'cell.toggleCheckbox',
    (s) => {
      const v = view(s);
      return deps.exec({
        cmd: 'toggleCheckbox',
        sheet: v.sheet,
        range: v.range,
        row: v.active.row,
        col: v.active.col,
      });
    },
    has,
  );
  add(
    'insert.pictureInCell',
    async (s) => {
      const v = view(s);
      const f = await deps.pickFile(PICTURE_TYPES);
      if (!f) return;
      await deps.exec({
        cmd: 'placeImageInCell',
        sheet: v.sheet,
        row: v.active.row,
        col: v.active.col,
        data: toBase64(f.bytes),
        ext: extOf(f.name),
        alt: null,
      });
    },
    has,
  );
  add(
    'cell.pictureAltText',
    async (s) => {
      const v = view(s);
      const { row, col } = v.active;
      const img = (await mediaState(v.host, v.sheet, row, col))?.image;
      // Only stored pictures have editable text (IMAGE takes it from its arguments).
      if (img?.kind !== 'local') return;
      deps.openAltText({
        sheet: v.sheet,
        row,
        col,
        alt: img.alt ?? '',
        decorative: img.decorative,
      });
    },
    has,
  );
}

/**
 * Delete on a selection with checkboxes: the engine clears their values, or removes the
 * control when they are already empty. Resolves `false` when the selection has no checkbox
 * (the editor clears it as usual).
 */
export async function deleteCheckboxes(
  exec: (cmd: SheetCommand) => Promise<unknown>,
  sheet: number,
  range: SheetRange,
): Promise<boolean> {
  const r = (await exec({ cmd: 'deleteCheckboxes', sheet, range })) as {
    handled?: boolean;
  } | null;
  return !!r?.handled;
}

/** Alternative text of a picture placed in a cell. */
export function CellPictureAltDialog({
  initial,
  decorative: initialDecorative,
  onClose,
  onApply,
}: {
  initial: string;
  decorative: boolean;
  onClose: () => void;
  onApply: (alt: string | null, decorative: boolean) => void | Promise<void>;
}) {
  const { t } = useTranslation();
  const [alt, setAlt] = useState(initial);
  const [decorative, setDecorative] = useState(initialDecorative);
  return (
    <Dialog
      open
      onOpenChange={(o) => !o && onClose()}
      title={t('sheetEditor.cellMedia.altTitle')}
      testId="cell-picture-alt-dialog"
      width={400}
      footer={
        <>
          <button type="button" className={editorStyles.btn} onClick={onClose}>
            {t('sheetEditor.dlg.cancel')}
          </button>
          <button
            type="button"
            className={editorStyles.btnPrimary}
            data-testid="cell-picture-alt-ok"
            onClick={() => void onApply(decorative ? null : alt.trim() || null, decorative)}
          >
            {t('sheetEditor.dlg.ok')}
          </button>
        </>
      }
    >
      <label>
        {t('sheetEditor.cellMedia.altLabel')}
        <textarea
          data-testid="cell-picture-alt-text"
          value={alt}
          rows={3}
          disabled={decorative}
          placeholder={t('sheetEditor.cellMedia.altHint')}
          onChange={(e) => setAlt(e.target.value)}
        />
      </label>
      <label className={editorStyles.check}>
        <input
          type="checkbox"
          data-testid="cell-picture-decorative"
          checked={decorative}
          onChange={(e) => setDecorative(e.target.checked)}
        />
        {t('sheetEditor.cellMedia.decorative')}
      </label>
    </Dialog>
  );
}
