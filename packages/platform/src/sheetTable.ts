/**
 * BlueSheet tables (SHEET-TABLE-001, ECMA-376 Part 1, §18.5): the tables the engine lists and the
 * commands that change them (`bo_engine::sheet::SheetCommand`): totals row, style options,
 * resize, convert to range, automatic expansion / calculated columns.
 */
import type { SheetRange } from './sheet';

/** Totals row function (§18.18.86 `ST_TotalsRowFunction`). */
export type TableTotalFunction =
  'none' | 'sum' | 'min' | 'max' | 'average' | 'count' | 'countNums' | 'stdDev' | 'var' | 'custom';

/** One table column as `tables` lists it. */
export interface TableColumnInfo {
  name: string;
  totalsFunction: TableTotalFunction;
  /** Formula of a `custom` total (without `=`). */
  totalsFormula: string | null;
  totalsLabel: string | null;
  /** `calculatedColumnFormula` of a calculated column. */
  calculated: string | null;
}

/** A table as `tables` lists it. */
export interface TableInfo {
  index: number;
  id: number;
  name: string;
  /** Header and totals rows included. */
  range: SheetRange;
  headerRow: boolean;
  totalsRow: boolean;
  filterButton: boolean;
  /** Style name (`TableStyleMedium2`); `null` = no style. */
  style: string | null;
  bandedRows: boolean;
  bandedColumns: boolean;
  firstColumn: boolean;
  lastColumn: boolean;
  columns: TableColumnInfo[];
}

/** What the last typing did automatically to a table. */
export interface TableAutoAction {
  kind: 'expand' | 'calc';
  /** Increases with every automatic action. */
  seq: number;
}

/** Result of `tables`. */
export interface TableList {
  tables: TableInfo[];
  autoExpand: boolean;
  autoCalc: boolean;
  lastAuto: TableAutoAction | null;
}

/** One style of the gallery (`tableStyles`); colours are `RRGGBB`. */
export interface TableStyleEntry {
  name: string;
  family: 'light' | 'medium' | 'dark';
  /** Accent number 1–7. */
  accent: number;
  header: string | null;
  headerFont: string | null;
  stripe: string;
  totals: string | null;
}

/** Changes of `tableUpdate`. */
// A type alias (not an interface) so commands stay assignable to `{ cmd: string; [k: string]: unknown }`.
export type TableUpdate = {
  name?: string;
  headerRow?: boolean;
  totalsRow?: boolean;
  /** Label of the first column when the totals row is first shown. */
  totalsLabel?: string;
  /** Style name; `''` = no style. */
  style?: string;
  bandedRows?: boolean;
  bandedColumns?: boolean;
  firstColumn?: boolean;
  lastColumn?: boolean;
  filterButton?: boolean;
};

/** Table commands. */
export type TableCommand =
  | { cmd: 'tables'; sheet: number }
  | { cmd: 'tableStyles' }
  | ({ cmd: 'tableUpdate'; sheet: number; table: number } & TableUpdate)
  | {
      cmd: 'tableTotal';
      sheet: number;
      table: number;
      column: number;
      function: TableTotalFunction;
      formula?: string;
    }
  | { cmd: 'tableResize'; sheet: number; table: number; range: SheetRange }
  | { cmd: 'tableConvert'; sheet: number; table: number }
  | { cmd: 'tableOptions'; autoExpand?: boolean; autoCalc?: boolean };
