/**
 * Data tools of BlueSheet (`bo_sheet::datatools`, `bo_sheet::outline`): Text to Columns,
 * Remove Duplicates, Consolidate, Subtotal, outline groups and the invalid-data list.
 */

/** Zero-based inclusive range (same shape as `SheetRange`). */
interface Rng {
  r0: number;
  c0: number;
  r1: number;
  c1: number;
}

/** A cell on a sheet (same shape as `CellAt`). */
interface At {
  sheet: number;
  row: number;
  col: number;
}

/** How Text to Columns cuts a cell (`bo_sheet::datatools::SplitMode`). */
export type TextSplitMode =
  | {
      mode: 'delimited';
      /** One-character delimiters (`'\t'`, `';'`, `','`, `' '`, any other). */
      delimiters: string[];
      /** A run of delimiters separates only two fields. */
      consecutive?: boolean;
      /** Text qualifier (`'"'` or `"'"`); `null` = none. */
      qualifier?: string | null;
    }
  | {
      mode: 'fixed';
      /** Character positions where a new field starts. */
      breaks: number[];
    };

/** Day / month / year order of a date column. */
export type DateOrder = 'dmy' | 'mdy' | 'ymd' | 'dym' | 'myd' | 'ydm';

/** Format of one Text to Columns result column. */
export type TextColumnFormat = 'general' | 'text' | 'skip' | { date: DateOrder };

/** Summary function of Consolidate and Subtotal (§18.18.17 `ST_DataConsolidateFunction`). */
export type ConsolidateFunction =
  | 'sum'
  | 'count'
  | 'average'
  | 'max'
  | 'min'
  | 'product'
  | 'countNums'
  | 'stdDev'
  | 'stdDevp'
  | 'var'
  | 'varp';

export const CONSOLIDATE_FUNCTIONS: readonly ConsolidateFunction[] = [
  'sum',
  'count',
  'average',
  'max',
  'min',
  'product',
  'countNums',
  'stdDev',
  'stdDevp',
  'var',
  'varp',
];

/** Result of `textToColumns` with `preview`. */
export interface TextSplitPreview {
  rows: string[][];
  columns: number;
}

/** Result of `removeDuplicates`. */
export interface DedupeResult {
  removed: number;
  remaining: number;
}

/** Result of `subtotal`. */
export interface SubtotalResult {
  groups: number;
  range: Rng;
  levels: number;
}

/** Result of `invalidCells`: `[row, col]` pairs. */
export interface InvalidCellsResult {
  cells: [number, number][];
  truncated: boolean;
}

/** Data tools commands (members of `SheetCommand`). */
export type DataToolsCommand =
  | {
      cmd: 'textToColumns';
      sheet: number;
      range: Rng;
      split: TextSplitMode;
      formats?: TextColumnFormat[];
      dest?: At | null;
      preview?: boolean;
    }
  | {
      cmd: 'removeDuplicates';
      sheet: number;
      range: Rng;
      /** Absolute columns compared; empty = all. */
      columns: number[];
      header: boolean;
    }
  | {
      cmd: 'consolidate';
      sheet: number;
      /** A1 references, optionally sheet-qualified. */
      sources: string[];
      function: ConsolidateFunction;
      topRow: boolean;
      leftCol: boolean;
      links: boolean;
      dest: At;
    }
  | {
      cmd: 'subtotal';
      sheet: number;
      range: Rng;
      groupBy: number;
      function: ConsolidateFunction;
      columns: number[];
      replace?: boolean;
      pageBreaks?: boolean;
      summaryBelow?: boolean;
      /** Caption of a subtotal row; `{0}` = the group value. */
      label?: string | null;
      grandLabel?: string | null;
    }
  | { cmd: 'removeSubtotals'; sheet: number; range: Rng }
  | {
      cmd: 'outlineGroup';
      sheet: number;
      rows: boolean;
      first: number;
      last: number;
      ungroup?: boolean;
    }
  | { cmd: 'autoOutline'; sheet: number; range?: Rng | null }
  | { cmd: 'clearOutline'; sheet: number; range?: Rng | null }
  | { cmd: 'outlineShowLevel'; sheet: number; rows: boolean; level: number }
  | { cmd: 'invalidCells'; sheet: number };
