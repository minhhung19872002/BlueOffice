/**
 * BlueSheet checkboxes in cells, pictures placed in cells and `IMAGE` pictures
 * (SHEET-CELL-001, `bo_engine::sheet::cell_media_cmd`).
 */
import type { SheetRange } from './sheet';

/** A picture in a cell (`cellImageInfo`). */
export type CellImageInfo =
  | { kind: 'local'; alt: string | null; decorative: boolean }
  | {
      kind: 'web';
      alt: string | null;
      url: string;
      /** 0 fit, 1 fill, 2 original size, 3 custom. */
      sizing: number;
      height: number | null;
      width: number | null;
      state: 'pending' | 'ready' | 'failed';
    };

/** `cellMediaState`: checkbox and picture of one cell. */
export interface CellMediaState {
  checkbox: boolean;
  /** Boolean value of the cell, `null` for other values. */
  value: boolean | null;
  image: CellImageInfo | null;
}

/** Commands of `bo_engine::sheet::cell_media_cmd::CellMediaCommand`. */
export type CellMediaCommand =
  /** Checkbox control on every cell of `range`; empty cells become FALSE. One undo step. */
  | { cmd: 'insertCheckbox'; sheet: number; range: SheetRange }
  /** Remove the checkbox control (values stay). One undo step. */
  | { cmd: 'removeCheckbox'; sheet: number; range: SheetRange }
  /** Flip the checkboxes of `range` to the opposite of the active cell → new state or `null`. */
  | { cmd: 'toggleCheckbox'; sheet: number; range: SheetRange; row: number; col: number }
  /** Delete key: values first, then the control → `{ handled, removed? }`. */
  | { cmd: 'deleteCheckboxes'; sheet: number; range: SheetRange }
  /** Store a picture in a cell (base64 bytes). One undo step. */
  | {
      cmd: 'placeImageInCell';
      sheet: number;
      row: number;
      col: number;
      data: string;
      ext: string;
      alt?: string | null;
    }
  /** Alternative text of a cell picture. One undo step. */
  | {
      cmd: 'cellImageAlt';
      sheet: number;
      row: number;
      col: number;
      alt: string | null;
      decorative: boolean;
    }
  /** → `CellImageInfo | null`. */
  | { cmd: 'cellImageInfo'; sheet: number; row: number; col: number }
  /** → `CellMediaState`. */
  | { cmd: 'cellMediaState'; sheet: number; row: number; col: number }
  /** URLs the `IMAGE` pictures need (each listed once) → `string[]`. */
  | { cmd: 'imageRequests' }
  /** Bytes (base64) or failure of a fetched URL → `{ ready }`. */
  | { cmd: 'imageLoaded'; url: string; data?: string; error?: 'blocked' | 'connect' }
  /** Base64 bytes of a display-list picture id → `string | null`. */
  | { cmd: 'cellMedia'; id: string };
