/**
 * BlueSheet engine host: the same API whether `bo_engine::SheetEngine` runs as
 * WASM in a Web Worker (web) or natively behind Tauri commands (desktop).
 * Sizes and coordinates are in points, relative to the viewport.
 */
import type { GlyphOutline, PageDisplayList } from './types';

export interface SheetSummary {
  name: string;
  lastRow: number | null;
  lastCol: number | null;
  frozenRows: number;
  frozenCols: number;
  hidden: boolean;
  /** Hidden row spans `[first, last]` (hidden or filtered out). */
  hiddenRows: [number, number][];
  hiddenCols: [number, number][];
  /** AutoFilter range `[r0, c0, r1, c1]`. */
  autoFilter: [number, number, number, number] | null;
  /** Merged ranges `[r0, c0, r1, c1]`. */
  merges: [number, number, number, number][];
  charts: number;
  /** Number of pivot tables on the sheet. */
  pivots: number;
}

export interface WorkbookInfo {
  name: string;
  sheets: SheetSummary[];
}

export type SheetHit =
  | { kind: 'cell'; row: number; col: number }
  | { kind: 'columnHeader'; col: number }
  | { kind: 'rowHeader'; row: number }
  /** Near the right edge of a column header (drag to resize, double-click to autofit). */
  | { kind: 'columnEdge'; col: number }
  | { kind: 'rowEdge'; row: number }
  | { kind: 'filterButton'; row: number; col: number }
  /** A chart, and the cell under the point. */
  | { kind: 'chart'; index: number; row: number; col: number }
  | { kind: 'corner' }
  | { kind: 'none' };

export interface ScreenRect {
  x: number;
  y: number;
  w: number;
  h: number;
}

export interface CellDetails {
  address: string;
  /** Editable input (`=FORMULA`, number, text). */
  input: string;
  /** Formatted display text. */
  display: string;
}

/** The visible window of a sheet. */
export interface SheetView {
  sheet: number;
  scrollX: number;
  scrollY: number;
  width: number;
  height: number;
}

export type SheetSaveFormat = 'xlsx' | 'csv';

// ------------------------------------------------- editing commands (mirrors bo_engine::sheet::cmd)

/** Zero-based inclusive range. */
export interface SheetRange {
  r0: number;
  c0: number;
  r1: number;
  c1: number;
}

/** Partial cell formatting; colours are `RRGGBB` (`''` = automatic / no fill). */
export interface StylePatch {
  numFmt?: string;
  fontName?: string;
  fontSize?: number;
  bold?: boolean;
  italic?: boolean;
  underline?: 'none' | 'single' | 'double';
  strike?: boolean;
  fontColor?: string;
  fillColor?: string;
  hAlign?: string;
  vAlign?: string;
  wrap?: boolean;
  indent?: number;
  rotation?: number;
}

export type BorderPreset =
  'outline' | 'all' | 'inside' | 'none' | 'top' | 'bottom' | 'left' | 'right';

export interface BorderSpec {
  preset: BorderPreset;
  style?: string;
  color?: string | null;
}

export interface CellFormatState {
  numFmt: string;
  fontName: string;
  fontSize: number;
  bold: boolean;
  italic: boolean;
  underline: string;
  strike: boolean;
  fontColor: string;
  fillColor: string;
  hAlign: string;
  vAlign: string;
  wrap: boolean;
  indent: number;
  rotation: number;
  /** top, right, bottom, left line styles (`''` = none). */
  borders: [string, string, string, string];
  styleName: string;
  merged: boolean;
}

export interface CfStyle {
  fill?: string;
  fontColor?: string;
  bold?: boolean;
  italic?: boolean;
}

export type CfRuleSpec =
  | { kind: 'cellIs'; operator: string; formula1: string; formula2?: string; style: CfStyle }
  | { kind: 'expression'; formula: string; style: CfStyle }
  | { kind: 'containsText'; text: string; style: CfStyle }
  | { kind: 'top10'; rank: number; bottom?: boolean; percent?: boolean; style: CfStyle }
  | { kind: 'duplicateValues'; style: CfStyle }
  | { kind: 'colorScale'; minColor: string; midColor?: string; maxColor: string }
  | { kind: 'dataBar'; color: string }
  | { kind: 'iconSet'; set: string; reverse?: boolean };

export interface ValidationSpec {
  kind: string;
  operator?: string;
  formula1: string;
  formula2?: string;
  allowBlank?: boolean;
  errorTitle?: string;
  error?: string;
  errorStyle?: string;
  prompt?: string;
}

export interface FilterSpec {
  values?: string[];
  conditions?: { operator: string; value: string }[];
  and?: boolean;
  /** Years (or months of a year) to keep. */
  dates?: { year: number; month?: number }[];
  /** Relative date / average filter (`today`, `thisMonth`, `lastYear`, `aboveAverage`, …). */
  dynamic?: string;
  /** Keep cells filled with / written in this `RRGGBB` colour. */
  cellColor?: string;
  fontColor?: string;
}

/** One sort level; `col` is the key row when sorting left to right. */
export interface SortKeySpec {
  col: number;
  descending?: boolean;
  caseSensitive?: boolean;
  by?: 'value' | 'cellColor' | 'fontColor';
  /** `RRGGBB` of a colour key. */
  color?: string;
}

export type PasteMode = 'all' | 'values' | 'formats' | 'formulas';

/** Aggregation of a pivot value field (ECMA-376 `ST_DataConsolidateFunction`). */
export type PivotFunction =
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

export interface PivotValueSpec {
  field: number;
  function?: PivotFunction;
  /** Caption; omitted = "<Function> of <field>". */
  name?: string;
}

/** Report filter: shown item indices (empty = all). */
export interface PivotFilterSpec {
  field: number;
  items?: number[];
}

/** Captions written into pivot report cells (localized by the UI). */
export interface PivotLabels {
  grandTotal?: string;
  values?: string;
  all?: string;
  multiple?: string;
  blank?: string;
  column?: string;
  name?: string;
}

/** A type alias (not an interface) so commands keep an implicit index signature. */
export type PivotAreas = {
  rows: number[];
  cols: number[];
  values: PivotValueSpec[];
  filters: PivotFilterSpec[];
};

/** A pivot table and its field list (`pivots` command). */
export interface PivotInfo {
  index: number;
  name: string;
  location: SheetRange;
  /** Cells taken by the report, report filters included. */
  area: SheetRange;
  source: string | null;
  fields: { name: string; items: string[] }[];
  rows: number[];
  cols: number[];
  values: { field: number; function: PivotFunction; name: string | null }[];
  filters: { field: number; items: number[] }[];
}

export interface CellAt {
  sheet: number;
  row: number;
  col: number;
}

/** A sheet editing command (`bo_engine::sheet::SheetCommand`). */
export type SheetCommand =
  | { cmd: 'presets' }
  | { cmd: 'formatState'; sheet: number; row: number; col: number }
  | { cmd: 'format'; sheet: number; range: SheetRange; patch: StylePatch }
  | { cmd: 'borders'; sheet: number; range: SheetRange; spec: BorderSpec }
  | { cmd: 'cellStyle'; sheet: number; range: SheetRange; name: string }
  | { cmd: 'clearFormats'; sheet: number; range: SheetRange }
  | { cmd: 'paintFormat'; sheet: number; from: SheetRange; to: SheetRange }
  | { cmd: 'resize'; sheet: number; rows: boolean; first: number; last: number; size: number }
  | { cmd: 'autofit'; sheet: number; rows: boolean; first: number; last: number }
  | { cmd: 'hide'; sheet: number; rows: boolean; first: number; last: number; hidden: boolean }
  | { cmd: 'freeze'; sheet: number; rows: number; cols: number }
  | { cmd: 'merge'; sheet: number; range: SheetRange; across?: boolean; center?: boolean }
  | { cmd: 'unmerge'; sheet: number; range: SheetRange }
  | {
      cmd: 'sort';
      sheet: number;
      range: SheetRange;
      keys: SortKeySpec[];
      hasHeader?: boolean;
      leftToRight?: boolean;
    }
  | { cmd: 'sortColors'; sheet: number; range: SheetRange; key: number; leftToRight?: boolean }
  | { cmd: 'region'; sheet: number; range: SheetRange }
  | { cmd: 'toggleFilter'; sheet: number; range: SheetRange }
  | { cmd: 'filterValues'; sheet: number; col: number }
  | { cmd: 'setFilter'; sheet: number; col: number; filter: FilterSpec | null }
  | { cmd: 'autofill'; sheet: number; source: SheetRange; target: SheetRange }
  | { cmd: 'fillDir'; sheet: number; range: SheetRange; dir: 'down' | 'right' }
  | { cmd: 'addCf'; sheet: number; range: SheetRange; rule: CfRuleSpec }
  | { cmd: 'clearCf'; sheet: number; range: SheetRange }
  | { cmd: 'validation'; sheet: number; row: number; col: number }
  | { cmd: 'setValidation'; sheet: number; range: SheetRange; spec: ValidationSpec }
  | { cmd: 'setInputs'; sheet: number; cells: [number, number, string][]; validate?: boolean }
  | { cmd: 'insertChart'; sheet: number; range: SheetRange; kind: string }
  | { cmd: 'charts'; sheet: number }
  | {
      cmd: 'editChart';
      sheet: number;
      index: number;
      kind?: string;
      title?: string;
      legend?: string;
      xTitle?: string;
      yTitle?: string;
    }
  | { cmd: 'deleteChart'; sheet: number; index: number }
  | {
      cmd: 'moveChart';
      sheet: number;
      index: number;
      from: [number, number];
      to: [number, number];
    }
  | {
      cmd: 'find';
      query: string;
      matchCase?: boolean;
      wholeCell?: boolean;
      inFormulas?: boolean;
      sheet?: number | null;
    }
  | {
      cmd: 'replace';
      query: string;
      with: string;
      matchCase?: boolean;
      wholeCell?: boolean;
      sheet?: number | null;
      at?: CellAt | null;
    }
  | { cmd: 'names' }
  | { cmd: 'resolve'; sheet: number; text: string }
  | { cmd: 'copy'; sheet: number; range: SheetRange; cut?: boolean }
  | {
      cmd: 'paste';
      sheet: number;
      row: number;
      col: number;
      mode?: PasteMode;
      transpose?: boolean;
      skipBlanks?: boolean;
      /** Selected target: a copy repeats to fill it. */
      range?: SheetRange;
    }
  | { cmd: 'pageSetup'; sheet: number }
  | { cmd: 'pivotSource'; sheet: number; range: SheetRange }
  | ({
      cmd: 'pivotCreate';
      sheet: number;
      range: SheetRange;
      /** Top-left of the report on an existing sheet; omitted = new sheet. */
      dest?: CellAt | null;
      labels?: PivotLabels;
    } & Partial<PivotAreas>)
  | { cmd: 'pivots'; sheet: number }
  | ({ cmd: 'pivotUpdate'; sheet: number; index: number; labels?: PivotLabels } & PivotAreas)
  | { cmd: 'pivotRefresh'; sheet: number; index: number; labels?: PivotLabels }
  | {
      cmd: 'setPageSetup';
      sheet: number;
      orientation?: 'portrait' | 'landscape';
      paperSize?: number;
      scale?: number;
      fit?: [number, number] | null;
      margins?: [number, number, number, number];
      printArea?: SheetRange | null;
      titleRows?: [number, number] | null;
      gridLines?: boolean;
    };

export interface ChartInfo {
  index: number;
  kind: string;
  title: string | null;
  legend: string | null;
  xTitle: string | null;
  yTitle: string | null;
  from: [number, number];
  /** Cell holding the bottom-right corner. */
  to: [number, number];
  series: number;
}

export interface Rejection {
  title: string;
  message: string;
  style: string;
}

export interface PageSetupInfo {
  orientation: 'portrait' | 'landscape';
  paperSize: number;
  scale: number;
  fit: [number, number] | null;
  margins: [number, number, number, number];
  printArea: string | null;
  titleRows: [number, number] | null;
  gridLines: boolean;
}

export interface SheetHost {
  init(): Promise<void>;
  open(bytes: Uint8Array, name: string): Promise<WorkbookInfo>;
  newWorkbook(name: string): Promise<WorkbookInfo>;
  info(): Promise<WorkbookInfo>;
  render(view: SheetView): Promise<PageDisplayList>;
  hitTest(view: SheetView, x: number, y: number): Promise<SheetHit>;
  rangeRect(
    view: SheetView,
    r0: number,
    c0: number,
    r1: number,
    c1: number,
  ): Promise<ScreenRect | null>;
  contentSize(sheet: number): Promise<[number, number]>;
  cell(sheet: number, row: number, col: number): Promise<CellDetails>;
  /** Sets inputs as one undo step. */
  setInputs(sheet: number, cells: [number, number, string][]): Promise<void>;
  structural(
    sheet: number,
    rows: boolean,
    at: number,
    count: number,
    remove: boolean,
  ): Promise<void>;
  undo(): Promise<boolean>;
  redo(): Promise<boolean>;
  addSheet(name?: string): Promise<number>;
  renameSheet(sheet: number, name: string): Promise<void>;
  deleteSheet(sheet: number): Promise<void>;
  save(format: SheetSaveFormat, sheet: number): Promise<Uint8Array>;
  exportPdf(): Promise<Uint8Array>;
  getGlyphOutlines(face: number, glyphs: number[]): Promise<GlyphOutline[]>;
  /** Runs an editing command; the result shape depends on the command. */
  exec<T = unknown>(cmd: SheetCommand): Promise<T>;
  /**
   * Notifies about changes made outside this editor (collaborators); the UI
   * re-reads info and redraws. Hosts without collaboration omit it.
   */
  onExternalChange?(listener: () => void): () => void;
}

/** Method names understood by both transports (web worker and Tauri). */
export type SheetMethod =
  | 'open'
  | 'newWorkbook'
  | 'info'
  | 'render'
  | 'hitTest'
  | 'rangeRect'
  | 'contentSize'
  | 'cell'
  | 'setInputs'
  | 'structural'
  | 'undo'
  | 'redo'
  | 'addSheet'
  | 'renameSheet'
  | 'deleteSheet'
  | 'save'
  | 'exportPdf'
  | 'exec'
  | 'glyphs';

/** One call to the engine; JSON results are already parsed by the transport. */
export type SheetRpc = (method: SheetMethod, args: unknown[]) => Promise<unknown>;

const v = (view: SheetView): number[] => [
  view.sheet,
  view.scrollX,
  view.scrollY,
  view.width,
  view.height,
];

/** {@link SheetHost} over any {@link SheetRpc} transport. */
export class RpcSheetHost implements SheetHost {
  constructor(
    private readonly rpc: SheetRpc,
    private readonly ready: () => Promise<void> = async () => {},
  ) {}

  private async call<T>(method: SheetMethod, ...args: unknown[]): Promise<T> {
    await this.ready();
    return (await this.rpc(method, args)) as T;
  }

  init(): Promise<void> {
    return this.ready();
  }
  open(bytes: Uint8Array, name: string) {
    return this.call<WorkbookInfo>('open', bytes, name);
  }
  newWorkbook(name: string) {
    return this.call<WorkbookInfo>('newWorkbook', name);
  }
  info() {
    return this.call<WorkbookInfo>('info');
  }
  render(view: SheetView) {
    return this.call<PageDisplayList>('render', ...v(view));
  }
  hitTest(view: SheetView, x: number, y: number) {
    return this.call<SheetHit>('hitTest', ...v(view), x, y);
  }
  rangeRect(view: SheetView, r0: number, c0: number, r1: number, c1: number) {
    return this.call<ScreenRect | null>('rangeRect', ...v(view), r0, c0, r1, c1);
  }
  contentSize(sheet: number) {
    return this.call<[number, number]>('contentSize', sheet);
  }
  cell(sheet: number, row: number, col: number) {
    return this.call<CellDetails>('cell', sheet, row, col);
  }
  setInputs(sheet: number, cells: [number, number, string][]) {
    return this.call<void>('setInputs', sheet, JSON.stringify(cells));
  }
  structural(sheet: number, rows: boolean, at: number, count: number, remove: boolean) {
    return this.call<void>('structural', sheet, rows, at, count, remove);
  }
  undo() {
    return this.call<boolean>('undo');
  }
  redo() {
    return this.call<boolean>('redo');
  }
  addSheet(name?: string) {
    return this.call<number>('addSheet', name ?? null);
  }
  renameSheet(sheet: number, name: string) {
    return this.call<void>('renameSheet', sheet, name);
  }
  deleteSheet(sheet: number) {
    return this.call<void>('deleteSheet', sheet);
  }
  save(format: SheetSaveFormat, sheet: number) {
    return this.call<Uint8Array>('save', format, sheet);
  }
  exportPdf() {
    return this.call<Uint8Array>('exportPdf');
  }
  getGlyphOutlines(face: number, glyphs: number[]) {
    return this.call<GlyphOutline[]>('glyphs', face, glyphs);
  }
  exec<T = unknown>(cmd: SheetCommand) {
    return this.call<T>('exec', JSON.stringify(cmd));
  }
}
