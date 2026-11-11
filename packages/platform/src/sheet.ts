/**
 * BlueSheet engine host: the same API whether `bo_engine::SheetEngine` runs as
 * WASM in a Web Worker (web) or natively behind Tauri commands (desktop).
 * Sizes and coordinates are in points, relative to the viewport.
 */
import type { InspectCommand } from './inspect';
import type { DataToolsCommand } from './sheetDataTools';
import type { SparklineCommand } from './sparkline';
import type { TableCommand } from './sheetTable';
import type {
  GlyphOutline,
  PageDisplayList,
  SheetPeer,
  SlideSmartArtColors,
  SlideSmartArtItem,
  SlideSmartArtLayout,
  SlideSmartArtStyle,
} from './types';

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
  /** `sheetProtection/@sheet` (ECMA-376 Part 1, §18.3.1.85). */
  protected: boolean;
  /** `tabColor` as `RRGGBB` (§18.3.1.93). */
  tabColor: string | null;
}

/** A threaded comment thread anchored on a cell. */
export interface SheetThreadInfo {
  id: string;
  row: number;
  col: number;
  /** A1 reference. */
  cell: string;
  done: boolean;
  /** First comment, then the replies. */
  comments: import('./people').ThreadCommentInfo[];
}

export interface WorkbookInfo {
  name: string;
  sheets: SheetSummary[];
  /** Saved with a password ([MS-OFFCRYPTO]). */
  encrypted: boolean;
  /** Has a VBA project / macro sheets (kept, never run). */
  macros: boolean;
  /** `workbookProtection/@lockStructure` (§18.2.29). */
  structureLocked: boolean;
  /** Cells converted from TCVN3 / VNI fonts to Unicode on open (undo restores them). */
  legacyConverted?: number;
  /** Package variant the next `.xlsx` save produces (`xlsx`, `xlsm`, `xltx`, `xltm`). */
  variant?: string;
  /** The VBA project of the opened `.xls` was signed: the signature is left out on save. */
  vbaSignatureDropped?: boolean;
}

/** What stays allowed on a protected sheet (§18.3.1.85, inverted flags). */
export interface SheetPermissions {
  selectLockedCells: boolean;
  selectUnlockedCells: boolean;
  formatCells: boolean;
  formatColumns: boolean;
  formatRows: boolean;
  insertColumns: boolean;
  insertRows: boolean;
  insertHyperlinks: boolean;
  deleteColumns: boolean;
  deleteRows: boolean;
  sort: boolean;
  autoFilter: boolean;
  pivotTables: boolean;
  objects: boolean;
}

export interface ProtectionState {
  sheet: { protected: boolean; hasPassword: boolean; allow: SheetPermissions };
  workbook: { structure: boolean; windows: boolean; hasPassword: boolean };
}

/** Status-bar figures of a (multi-range) selection. */
export interface RangeStats {
  sum: number;
  count: number;
  numbers: number;
  average: number | null;
  /** Smallest / largest number (`null` without numbers; absent from older engines). */
  min?: number | null;
  max?: number | null;
}

/** Result of the `flashFill` command. */
export interface FlashFillResult {
  /** A consistent, unambiguous pattern was found. */
  found: boolean;
  /** Cells filled. */
  filled: number;
}

/** A defined name (`nameList`). */
export interface DefinedNameInfo {
  name: string;
  /** With its `=`: `=Sheet1!$B$2:$B$4`. */
  refersTo: string;
  /** Sheet index, or null for the workbook. */
  scope: number | null;
  comment: string | null;
}

/** How the fill handle fills (`autofill` command). */
export type AutoFillMode = 'auto' | 'toggle' | 'copy' | 'series' | 'formats' | 'values';

/** Result of the `sortRegion` command. */
export interface SortRegion {
  /** The range a quick sort uses (a single cell's current region). */
  range: SheetRange;
  /** Whether its first row looks like column labels. */
  hasHeader: boolean;
  /** The current region around the selection's first cell. */
  around: SheetRange;
}

/** Engine error text of an edit refused by sheet protection. */
export const SHEET_PROTECTED = 'sheet protected';
/** Engine error text of a sheet operation refused by workbook protection. */
export const WORKBOOK_PROTECTED = 'workbook protected';
/** Engine error text of a wrong sheet / workbook protection password. */
export const WRONG_PROTECTION_PASSWORD = 'wrong protection password';

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
  /** A SmartArt diagram (CORE-SMART-005), and the cell under the point. */
  | { kind: 'smartArt'; index: number; row: number; col: number }
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

/**
 * `html`: every sheet as an HTML table; `html-sheet`: the given sheet only; `mht`: the
 * tables as a single-file web page; `xml`: Flat OPC
 * of the `.xlsx` package (CORE-FMT-003).
 */
export type SheetSaveFormat =
  | 'xlsx'
  | 'csv'
  | 'ods'
  | 'html'
  | 'html-sheet'
  | 'mht'
  | 'xml'
  // The given sheet as SYLK, DIF or formatted text, space delimited (CORE-FMT-004).
  | 'slk'
  | 'dif'
  | 'prn';

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
  /** `alignment/@shrinkToFit` (§18.8.1). */
  shrink?: boolean;
  /** `protection/@locked` (§18.8.33). */
  locked?: boolean;
  /** `protection/@hidden` (§18.8.33). */
  formulaHidden?: boolean;
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
  /** Shrink to fit (`shrinkToFit`). */
  shrink?: boolean;
  /** top, right, bottom, left line styles (`''` = none). */
  borders: [string, string, string, string];
  styleName: string;
  merged: boolean;
  locked: boolean;
  formulaHidden: boolean;
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
  | { kind: 'dataBar'; color: string; options?: DataBarOptions }
  | { kind: 'iconSet'; set: string; reverse?: boolean; options?: IconSetOptions };

/** A data bar / icon set threshold (ECMA-376 §18.3.1.11 `cfvo`). */
export interface CfvoSpec {
  /** num, percent, percentile, formula, min, max; data bars also autoMin / autoMax. */
  kind: string;
  value?: string | null;
  /** The band starts at values ≥ the threshold (false: >). */
  gte?: boolean;
}

/** Data bar options ([MS-XLSX] x14 dataBar: negative bars, axis, border, gradient). */
export interface DataBarOptions {
  min: CfvoSpec;
  max: CfvoSpec;
  minLength?: number;
  maxLength?: number;
  gradient?: boolean;
  border?: boolean;
  borderColor?: string | null;
  negativeColor?: string | null;
  negativeBorderColor?: string | null;
  negativeSameColor?: boolean;
  negativeSameBorder?: boolean;
  /** automatic | middle | none */
  axisPosition?: string;
  axisColor?: string | null;
  /** context | leftToRight | rightToLeft */
  direction?: string;
  showValue?: boolean;
}

/** An icon of a custom icon set ([MS-XLSX] x14:cfIcon). */
export interface CfIconSpec {
  set: string;
  id: number;
}

/** Icon set options: thresholds above the lowest band, icon only, custom icons. */
export interface IconSetOptions {
  thresholds: CfvoSpec[];
  showValue?: boolean;
  icons?: CfIconSpec[];
}

/** A rule listed by the conditional formatting rules manager (`listCf`). */
export interface CfRuleInfo {
  block: number;
  rule: number;
  priority: number;
  kind: string;
  range: string;
  stopIfTrue: boolean;
  pivot: boolean;
  ext: boolean;
  spec: CfRuleSpec | null;
}

/** A change made in the rules manager (`editCf`). */
export type CfOp =
  | { op: 'moveUp' }
  | { op: 'moveDown' }
  | { op: 'delete' }
  | { op: 'stopIfTrue'; value: boolean }
  | { op: 'setRange'; range: string }
  | { op: 'replace'; rule: CfRuleSpec };

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
  /** Keep the largest (`top`, default) or smallest `val` items, or `val` percent of them. */
  top10?: { top?: boolean; percent?: boolean; val: number };
  /** Keep cells showing icon `id` of conditional-format icon set `set` (no `id`: no icon). */
  icon?: { set: string; id?: number };
}

/** A conditional-format icon: icon set name and zero-based icon. */
export interface SheetIcon {
  set: string;
  id: number;
}

/** Result of `advancedFilter`. */
export interface AdvancedFilterResult {
  matched: number;
  hidden: number;
  list: SheetRange;
  extract: SheetRange | null;
}

/** One sort level; `col` is the key row when sorting left to right. */
export interface SortKeySpec {
  col: number;
  descending?: boolean;
  caseSensitive?: boolean;
  by?: 'value' | 'cellColor' | 'fontColor' | 'icon';
  /** `RRGGBB` of a colour key. */
  color?: string;
  /** Icon of an icon key. */
  iconSet?: string;
  iconId?: number;
  /** Custom order of a value key (entries first, in this order). */
  customList?: string[];
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

/** "Show values as" (ECMA-376 `ST_ShowDataAs` plus the x14 calculations). */
export type PivotShowAs =
  | 'normal'
  | 'difference'
  | 'percent'
  | 'percentDiff'
  | 'runTotal'
  | 'percentOfRow'
  | 'percentOfCol'
  | 'percentOfTotal'
  | 'index'
  | 'percentOfParentRow'
  | 'percentOfParentCol'
  | 'percentOfParent'
  | 'percentOfRunningTotal'
  | 'rankAscending'
  | 'rankDescending';

/** `baseItem` values meaning the previous / next item. */
export const PIVOT_BASE_PREVIOUS = 1048828;
export const PIVOT_BASE_NEXT = 1048829;

export interface PivotValueSpec {
  field: number;
  function?: PivotFunction;
  /** Caption; omitted = "<Function> of <field>". */
  name?: string;
  showAs?: PivotShowAs;
  baseField?: number;
  baseItem?: number;
  /** Number format code; omitted = from the source column. */
  numFmt?: string;
}

export type PivotLayout = 'compact' | 'outline' | 'tabular';
export type PivotSort = 'manual' | 'ascending' | 'descending';

/** Settings of a row / column field (`pivotField`). */
export interface PivotFieldOptions {
  subtotal: boolean;
  subtotalTop: boolean;
  blankRow: boolean;
  repeatLabels: boolean;
  sort: PivotSort;
  /** Sort by this value field (index) instead of by label. */
  sortBy: number | null;
  /** Hidden item indices. */
  hidden: number[];
}

/** ECMA-376 `ST_PivotFilterType` values used by label, value and top-N filters. */
export type PivotFilterType =
  | 'count'
  | 'percent'
  | 'sum'
  | 'captionEqual'
  | 'captionNotEqual'
  | 'captionBeginsWith'
  | 'captionNotBeginsWith'
  | 'captionEndsWith'
  | 'captionNotEndsWith'
  | 'captionContains'
  | 'captionNotContains'
  | 'captionGreaterThan'
  | 'captionGreaterThanOrEqual'
  | 'captionLessThan'
  | 'captionLessThanOrEqual'
  | 'captionBetween'
  | 'captionNotBetween'
  | 'valueEqual'
  | 'valueNotEqual'
  | 'valueGreaterThan'
  | 'valueGreaterThanOrEqual'
  | 'valueLessThan'
  | 'valueLessThanOrEqual'
  | 'valueBetween'
  | 'valueNotBetween'
  | PivotDateFilterType;

/** Date filters of `ST_PivotFilterType`: fixed dates (ISO `yyyy-mm-dd` or serials) and periods. */
export const PIVOT_DATE_FILTERS = [
  'dateEqual',
  'dateNotEqual',
  'dateOlderThan',
  'dateOlderThanOrEqual',
  'dateNewerThan',
  'dateNewerThanOrEqual',
  'dateBetween',
  'dateNotBetween',
  'tomorrow',
  'today',
  'yesterday',
  'nextWeek',
  'thisWeek',
  'lastWeek',
  'nextMonth',
  'thisMonth',
  'lastMonth',
  'nextQuarter',
  'thisQuarter',
  'lastQuarter',
  'nextYear',
  'thisYear',
  'lastYear',
  'yearToDate',
  'Q1',
  'Q2',
  'Q3',
  'Q4',
  'M1',
  'M2',
  'M3',
  'M4',
  'M5',
  'M6',
  'M7',
  'M8',
  'M9',
  'M10',
  'M11',
  'M12',
] as const;
export type PivotDateFilterType = (typeof PIVOT_DATE_FILTERS)[number];

/** A label / value / top-N filter of a row or column field. */
export interface PivotFieldFilter {
  field: number;
  kind: PivotFilterType;
  /** Value field index (value and top-N filters). */
  measure: number;
  value1: string;
  value2: string;
  /** Top-N: top (true) or bottom. */
  top: boolean;
  count: number;
}

/** `pivotTableStyleInfo`. */
export interface PivotStyle {
  name: string;
  rowHeaders: boolean;
  colHeaders: boolean;
  rowStripes: boolean;
  colStripes: boolean;
  lastColumn: boolean;
}

/** Report settings sent with `pivotUpdate` (omitted = unchanged). */
export type PivotSettings = {
  layout?: PivotLayout;
  rowGrandTotals?: boolean;
  colGrandTotals?: boolean;
  fieldOptions?: (Partial<PivotFieldOptions> & { field: number })[];
  fieldFilters?: PivotFieldFilter[];
  style?: PivotStyle;
  /** Several value fields listed down the rows (`dataOnRows`). */
  dataOnRows?: boolean;
};

/** Grouping of a field as listed by `pivots`. */
export type PivotGroupInfo =
  | {
      by: 'dates';
      years: boolean;
      quarters: boolean;
      months: boolean;
      days: boolean;
      start: number;
      end: number;
    }
  | { by: 'numbers'; start: number; end: number; interval: number }
  | { by: 'derived'; base: number | null }
  | { by: 'discrete' };

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
  /** Subtotal caption; `{item}` is replaced by the item. */
  subtotal?: string;
  rowLabels?: string;
  colLabels?: string;
  /** Short month names, January first (date grouping). */
  months?: string[];
  /** Quarter caption; `{n}` is replaced by 1–4. */
  quarter?: string;
  yearsField?: string;
  quartersField?: string;
  monthsField?: string;
  /** Caption of a manual group; `{n}` is replaced by its number. */
  group?: string;
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
  /** Other pivot tables on the same cache (shared groupings and calculated fields). */
  sharedWith?: { sheet: number; sheetName: string; name: string }[];
  /** The value cells of the report (`null` without value fields). */
  dataArea?: SheetRange | null;
  location: SheetRange;
  /** Cells taken by the report, report filters included. */
  area: SheetRange;
  source: string | null;
  /** The source is the workbook data model (fields are `Table[Column]` or `[Measure]`). */
  model?: boolean;
  fields: {
    name: string;
    /** Data model: table of the field (`null` for a measure). */
    table?: string | null;
    /** Data model: column (or measure) name. */
    column?: string | null;
    /** Data model: a measure (value field only). */
    measure?: boolean;
    items: string[];
    /** Formula of a calculated field. */
    formula: string | null;
    /** Calculated or grouping field (no source column). */
    derived: boolean;
    /** Dates (date number format or date grouping). */
    date: boolean;
    group: PivotGroupInfo | null;
  }[];
  rows: number[];
  cols: number[];
  values: {
    field: number;
    function: PivotFunction;
    name: string | null;
    showAs: PivotShowAs;
    baseField: number;
    baseItem: number;
    numFmt: string | null;
  }[];
  filters: { field: number; items: number[] }[];
  layout: PivotLayout;
  rowGrandTotals: boolean;
  colGrandTotals: boolean;
  /** Settings of every cache field. */
  fieldOptions: PivotFieldOptions[];
  fieldFilters: PivotFieldFilter[];
  style: PivotStyle | null;
  dataOnRows: boolean;
  /** Calculated items (`calculatedItems`). */
  calcItems: { field: number; name: string; formula: string }[];
}

export interface CellAt {
  sheet: number;
  row: number;
  col: number;
}

/** Solver constraint relation (`bo_sheet::solver::Relation`); `dif` = permutation of 1..n. */
export type SolverRelation = 'le' | 'ge' | 'eq' | 'int' | 'bin' | 'dif';
/** Solver method (`bo_sheet::solver::Method`). */
export type SolverMethod = 'simplex' | 'nonlinear' | 'evolutionary';
/** Solver report (`bo_sheet::solver_report::ReportKind`). */
export type SolverReportKind = 'answer' | 'sensitivity' | 'limits';
/** Solver options (`bo_sheet::solver::Options`); omitted fields use method defaults. */
export interface SolverOptions {
  maxTimeMs?: number;
  maxIterations?: number;
  precision?: number;
  convergence?: number;
  multistart?: boolean;
  starts?: number;
  seed?: number;
  population?: number;
  mutation?: number;
  maxSubproblems?: number;
}
/** Result of the `solve` command. */
export interface SolverResult {
  status:
    'optimal' | 'infeasible' | 'unbounded' | 'notLinear' | 'error' | 'missingBounds' | 'limit';
  objective: number;
  values: [[number, number], number][];
  /** Report sheets written (with `reports`). */
  reports?: { kind: SolverReportKind; sheet: number }[];
}

/** A sheet editing command (`bo_engine::sheet::SheetCommand`). */
export type SheetCommand =
  | import('./security').SecurityCommand
  /** Macro source of the open file, read-only (CORE-VBA-002) → `VbaProjectInfo`. */
  | import('./vba').VbaCommand
  | DataToolsCommand
  /** Get & Transform data (SHEET-PQ-001). */
  | import('./sheetQuery').SheetQueryCommand
  /** Data model: tables, relationships, measures (SHEET-DM-001). */
  | import('./sheetModel').SheetModelCommand
  /** Formula auditing and calculation (SHEET-AUDIT-001). */
  | import('./audit').AuditCommand
  /** Spelling and grammar of text cells (CORE-SPELL-006). */
  | import('./spelling').SheetSpellCommand
  /** Show Changes: compare with another version (CORE-VERSION-001) → `SheetCompareResult`. */
  | import('./versions').CompareWorkbookCommand
  /** Workbook views (SHEET-VIEW-001). */
  | import('./sheetViews').SheetViewCommand
  | { cmd: 'presets' }
  /** File › Info document properties (CORE-UI-002) → `DocProperties`. */
  | import('./docProperties').DocPropertiesCommand
  /** Check document (CORE-INSPECT-001) → `InspectReport`. */
  | InspectCommand
  /** Sparklines (SHEET-SPARK-001). */
  | SparklineCommand
  /** Quick Analysis and recommended charts (CORE-DESIGN-001), read only. */
  | import('./designIdeas').QuickAnalysisCommand
  /** Checkboxes and pictures in cells, IMAGE pictures (SHEET-CELL-001). */
  | import('./cellMedia').CellMediaCommand
  /** Tables (SHEET-TABLE-001). */
  | TableCommand
  /**
   * Choices of the next `exportPdf` (CORE-FMT-002): PDF/A part, tagged PDF and
   * bookmarks (one per sheet). Returns the choices.
   */
  | {
      cmd: 'setPdfExport';
      pdfa?: import('./slideExport').PdfAPart | null;
      tagged?: boolean;
      bookmarks?: boolean;
    }
  /** TCVN3 / VNI text → Unicode (legacy Vietnamese fonts); one undo step. */
  | { cmd: 'convertLegacyText' }
  /** Threaded comments of a sheet ([MS-XLSX] threaded comments) → `SheetThreadInfo[]`. */
  | { cmd: 'comments'; sheet: number }
  /** New thread on a cell, or a reply to thread `parent` → `{ id }`. */
  | {
      cmd: 'addComment';
      sheet: number;
      row?: number;
      col?: number;
      text: string;
      author?: string;
      date?: string;
      parent?: string;
      mentions?: import('./people').Mention[];
      task?: import('./people').CommentTask;
    }
  | { cmd: 'resolveComment'; sheet: number; id: string; done: boolean }
  | { cmd: 'deleteComment'; sheet: number; id: string }
  | { cmd: 'formatState'; sheet: number; row: number; col: number }
  | { cmd: 'format'; sheet: number; range: SheetRange; patch: StylePatch }
  | { cmd: 'borders'; sheet: number; range: SheetRange; spec: BorderSpec }
  | { cmd: 'cellStyle'; sheet: number; range: SheetRange; name: string }
  | { cmd: 'clearFormats'; sheet: number; range: SheetRange }
  | { cmd: 'paintFormat'; sheet: number; from: SheetRange; to: SheetRange }
  | {
      cmd: 'resize';
      sheet: number;
      rows: boolean;
      first: number;
      last: number;
      size: number;
      /** Columns: `size` in characters, as the column width dialog shows it. */
      chars?: boolean;
    }
  /** Row height in points or column width in characters → `{ size }`. */
  | { cmd: 'sizeOf'; sheet: number; rows: boolean; index: number }
  | { cmd: 'autofit'; sheet: number; rows: boolean; first: number; last: number }
  | { cmd: 'hide'; sheet: number; rows: boolean; first: number; last: number; hidden: boolean }
  | { cmd: 'freeze'; sheet: number; rows: number; cols: number }
  | { cmd: 'merge'; sheet: number; range: SheetRange; across?: boolean; center?: boolean }
  | { cmd: 'unmerge'; sheet: number; range: SheetRange }
  /** Insert / Delete cells: the cells of `range` shift down / up (`down`) or right / left. */
  | { cmd: 'shiftCells'; sheet: number; range: SheetRange; down: boolean; insert: boolean }
  | {
      cmd: 'sort';
      sheet: number;
      range: SheetRange;
      keys: SortKeySpec[];
      hasHeader?: boolean;
      leftToRight?: boolean;
    }
  | { cmd: 'sortColors'; sheet: number; range: SheetRange; key: number; leftToRight?: boolean }
  | { cmd: 'sortLists' }
  | {
      cmd: 'advancedFilter';
      sheet: number;
      list: SheetRange;
      criteriaSheet?: number;
      criteria: SheetRange;
      copyTo?: SheetRange | null;
      unique?: boolean;
    }
  | { cmd: 'showAllData'; sheet: number }
  | { cmd: 'region'; sheet: number; range: SheetRange }
  /** Where Ctrl+arrow lands from a cell (answer `{ row, col }`). */
  | {
      cmd: 'dataEdge';
      sheet: number;
      row: number;
      col: number;
      dir: 'up' | 'down' | 'left' | 'right';
    }
  /** Text entries of the column's list around a cell (answer `string[]`): AutoComplete, Alt+Down. */
  | { cmd: 'columnEntries'; sheet: number; row: number; col: number }
  /** → {@link SortRegion}. */
  | { cmd: 'sortRegion'; sheet: number; range: SheetRange }
  /** Defined names → {@link DefinedNameInfo}[]. */
  | { cmd: 'nameList' }
  | {
      cmd: 'defineName';
      name: string;
      /** `=Sheet1!$A$1:$A$5`, `=0,1` (`;` / decimal commas accepted). */
      refersTo: string;
      /** Sheet index, or null for the workbook. */
      scope?: number | null;
      comment?: string;
      /** The name being edited (replaced). */
      old?: { name: string; scope: number | null };
    }
  | { cmd: 'deleteName'; name: string; scope?: number | null }
  /** Names from the labels on the chosen sides of `range` → `{ created }`. */
  | {
      cmd: 'createNames';
      sheet: number;
      range: SheetRange;
      top?: boolean;
      left?: boolean;
      bottom?: boolean;
      right?: boolean;
    }
  | { cmd: 'toggleFilter'; sheet: number; range: SheetRange }
  | { cmd: 'filterValues'; sheet: number; col: number }
  | { cmd: 'setFilter'; sheet: number; col: number; filter: FilterSpec | null }
  | {
      cmd: 'autofill';
      sheet: number;
      source: SheetRange;
      target: SheetRange;
      /** Auto Fill Options choice; `toggle`: the handle dragged with Ctrl held. */
      mode?: AutoFillMode;
    }
  | { cmd: 'fillDir'; sheet: number; range: SheetRange; dir: 'down' | 'right' }
  /** Home › Fill › Series → `{ filled }`. */
  | {
      cmd: 'fillSeries';
      sheet: number;
      range: SheetRange;
      /** Series in columns (down) rather than rows. */
      columns: boolean;
      kind: 'linear' | 'growth' | 'date' | 'autoFill';
      unit?: 'day' | 'weekday' | 'month' | 'year';
      step: number;
      stop?: number | null;
    }
  | { cmd: 'addCf'; sheet: number; range: SheetRange; rule: CfRuleSpec; pivot?: boolean }
  | { cmd: 'listCf'; sheet: number }
  | { cmd: 'editCf'; sheet: number; block: number; rule: number; op: CfOp }
  | { cmd: 'clearCf'; sheet: number; range: SheetRange }
  | { cmd: 'validation'; sheet: number; row: number; col: number }
  | { cmd: 'setValidation'; sheet: number; range: SheetRange; spec: ValidationSpec }
  | { cmd: 'setInputs'; sheet: number; cells: [number, number, string][]; validate?: boolean }
  /** Ctrl+Enter: one entry into every selected cell (formulas follow each cell), one step. */
  | {
      cmd: 'fillInputs';
      sheet: number;
      row: number;
      col: number;
      text: string;
      ranges: SheetRange[];
    }
  | {
      cmd: 'insertChart';
      sheet: number;
      range: SheetRange;
      kind: string;
      /** Chart template to format the new chart with (CORE-CHART-007). */
      template?: import('./chartDesign').ChartTemplate;
      texts?: import('./chartDesign').ChartLayoutTexts;
    }
  /** Chart Design gallery (answer: `ChartGallery`). */
  | { cmd: 'chartGallery'; sheet: number; index: number; width?: number; height?: number }
  /** Applies a chart template / style / colour variation / quick layout. */
  | ({ cmd: 'chartDesign'; sheet: number; index: number } & import('./chartDesign').ChartDesignSpec)
  /** A template as a `.crtx` package (answer: `{ base64, name }`). */
  | { cmd: 'chartTemplateExport'; template: import('./chartDesign').ChartTemplate }
  /** The template of a `.crtx` package (answer: `ChartTemplate`). */
  | { cmd: 'chartTemplateImport'; base64: string; name?: string }
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
  /** Options of a region map (CORE-CHART-006); "" clears an option (automatic / default). */
  | ({ cmd: 'setChartMap'; sheet: number; index: number } & Partial<ChartMapOptions>)
  /** Registers a map boundary dataset (answer: `{ id, regions }`). */
  | {
      cmd: 'registerMapDataset';
      manifest: string;
      boundaries: string;
      aliases?: string;
    }
  /** The registered map datasets (answer: `{ id, country, regions }[]`). */
  | { cmd: 'mapDatasets' }
  | { cmd: 'deleteChart'; sheet: number; index: number }
  | SheetSmartArtCommand
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
      /** Give the destination columns the copied columns' widths. */
      colWidths?: boolean;
      /** Paste without cell borders. */
      noBorders?: boolean;
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
  | ({
      cmd: 'pivotUpdate';
      sheet: number;
      index: number;
      labels?: PivotLabels;
    } & PivotAreas &
      PivotSettings)
  | { cmd: 'pivotRefresh'; sheet: number; index: number; labels?: PivotLabels }
  | {
      cmd: 'pivotGroup';
      sheet: number;
      index: number;
      field: number;
      by: 'none' | 'dates' | 'numbers';
      years?: boolean;
      quarters?: boolean;
      months?: boolean;
      days?: boolean;
      start?: number | null;
      end?: number | null;
      interval?: number;
      labels?: PivotLabels;
    }
  | {
      cmd: 'pivotCalcField';
      sheet: number;
      index: number;
      /** Existing calculated field; omitted = add one. */
      field?: number | null;
      name?: string;
      formula?: string;
      remove?: boolean;
      labels?: PivotLabels;
    }
  | {
      cmd: 'pivotCalcItem';
      sheet: number;
      index: number;
      field: number;
      old?: string;
      name?: string;
      formula?: string;
      remove?: boolean;
      labels?: PivotLabels;
    }
  | {
      cmd: 'pivotGroupItems';
      sheet: number;
      index: number;
      field: number;
      items: number[];
      ungroup?: boolean;
      labels?: PivotLabels;
    }
  | { cmd: 'pivotChart'; sheet: number; index: number; kind?: string }
  | { cmd: 'pivotDelete'; sheet: number; index: number }
  | { cmd: 'pivotMove'; sheet: number; index: number; dest: CellAt; labels?: PivotLabels }
  | {
      cmd: 'pivotChangeSource';
      sheet: number;
      index: number;
      /** A1 range, optionally sheet-qualified. */
      source: string;
      labels?: PivotLabels;
    }
  | { cmd: 'pivotRefreshAll'; labels?: PivotLabels }
  | ({
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
    } & PrintSetupExtra)
  | { cmd: 'pageBreak'; sheet: number; row?: number; col?: number; remove?: boolean }
  | { cmd: 'resetPageBreaks'; sheet: number }
  | { cmd: 'printPreview'; sheet: number; page?: number }
  | { cmd: 'setSavePassword'; password?: string }
  /** Package variant of the next `.xlsx` saves → `{ ok, variant }` / `{ ok: false, error }`. */
  | { cmd: 'saveFormat'; ext: string }
  /** Theme of a `.thmx` (or another file), base64 → `{ ok }` / `{ ok: false, error: 'noTheme' }`. */
  | { cmd: 'applyThemeFile'; file: string }
  /** The workbook's theme as a `.thmx` → `{ ok, file }` (base64). */
  | { cmd: 'saveThemeFile' }
  | { cmd: 'protection'; sheet: number }
  | { cmd: 'protectSheet'; sheet: number; password?: string; allow?: Partial<SheetPermissions> }
  | { cmd: 'unprotectSheet'; sheet: number; password?: string }
  | { cmd: 'protectWorkbook'; password?: string }
  | { cmd: 'unprotectWorkbook'; password?: string }
  | { cmd: 'moveSheet'; from: number; to: number }
  | { cmd: 'setTabColor'; sheet: number; color?: string | null }
  | { cmd: 'batch'; cmds: SheetCommand[] }
  /** Undo history length → `{ history }`; join the steps after `since` into one (BlueScript, CORE-SCRIPT-001). */
  | { cmd: 'historyLen' }
  | { cmd: 'mergeHistory'; since: number }
  | { cmd: 'rangeStats'; sheet: number; ranges: SheetRange[] }
  /**
   * Flash Fill: learn a pattern from the filled cells of column `col` (the other columns of
   * the block around the cell are the sources) and fill its empty cells below the examples.
   */
  | { cmd: 'flashFill'; sheet: number; row: number; col: number }
  /** What-if data table over `range` (headers included); input cells as A1 addresses. */
  | {
      cmd: 'dataTable';
      sheet: number;
      range: SheetRange;
      rowInput?: string | null;
      colInput?: string | null;
    }
  /** Turn a range into a table (first row = column names). */
  | { cmd: 'createTable'; sheet: number; range: SheetRange }
  /** Timelines of a sheet. */
  | { cmd: 'timelines'; sheet: number }
  /** Filter a timeline to the days start..end (serials); without them, clear it. */
  | { cmd: 'timelineSelect'; sheet: number; index: number; start?: number; end?: number }
  /** Slicers of a sheet. */
  | { cmd: 'slicers'; sheet: number }
  /** Insert a slicer for the table column of a cell. */
  | { cmd: 'insertSlicer'; sheet: number; row: number; col: number }
  /** Choose a slicer's items (empty: clear its filter). */
  | { cmd: 'slicerSelect'; sheet: number; index: number; items: string[] }
  /** Delete a slicer (its filter is cleared first). */
  | { cmd: 'deleteSlicer'; sheet: number; index: number }
  /** Insert a slicer over a pivot table field (`pivotSheet` defaults to `sheet`). */
  | {
      cmd: 'insertPivotSlicer';
      sheet: number;
      pivotSheet?: number;
      pivot: number;
      field: number;
      labels?: PivotLabels;
    }
  /** Caption, column count and style of a slicer (absent: unchanged). */
  | {
      cmd: 'slicerOptions';
      sheet: number;
      index: number;
      caption?: string;
      columns?: number;
      style?: string;
    }
  /** The pivot tables a pivot slicer filters. */
  | { cmd: 'slicerConnect'; sheet: number; index: number; pivots: string[]; labels?: PivotLabels }
  /** Move / size a slicer: anchor cells `[row, col]`. */
  | {
      cmd: 'moveSlicer';
      sheet: number;
      index: number;
      from: [number, number];
      to: [number, number];
    }
  /** Insert a timeline over a date field of a pivot table. */
  | {
      cmd: 'insertTimeline';
      sheet: number;
      pivotSheet?: number;
      pivot: number;
      field: number;
      labels?: PivotLabels;
    }
  /** Time level (0 years … 3 days), caption and style of a timeline (absent: unchanged). */
  | {
      cmd: 'timelineOptions';
      sheet: number;
      index: number;
      level?: number;
      caption?: string;
      style?: string;
    }
  /** The pivot tables a timeline filters. */
  | {
      cmd: 'timelineConnect';
      sheet: number;
      index: number;
      pivots: string[];
      labels?: PivotLabels;
    }
  /** Move / size a timeline: anchor cells `[row, col]`. */
  | {
      cmd: 'moveTimeline';
      sheet: number;
      index: number;
      from: [number, number];
      to: [number, number];
    }
  /** Delete a timeline (its date filter is cleared first). */
  | { cmd: 'deleteTimeline'; sheet: number; index: number }
  /** Show the source records of a pivot table value cell on a new sheet (`{ sheet, name, rows }`). */
  | {
      cmd: 'pivotDrillDown';
      sheet: number;
      row: number;
      col: number;
      /** Base name of the new sheet. */
      name?: string;
      labels?: PivotLabels;
    }
  /** Hide items of a pivot field (a report filter: show the others), by item index. */
  | {
      cmd: 'pivotFieldItems';
      sheet: number;
      index: number;
      field: number;
      hidden: number[];
      labels?: PivotLabels;
    }
  /** Form controls of a sheet ([MS-XLSX] control properties, legacy VML client data). */
  | { cmd: 'formControls'; sheet: number }
  /** Insert a form control with its top-left corner at a cell. */
  | {
      cmd: 'insertFormControl';
      sheet: number;
      kind: FormControlKind;
      row: number;
      col: number;
      text?: string;
      name?: string;
    }
  /** Click a control (check box, option button, button: reports its macro, not run). */
  | { cmd: 'formControlClick'; sheet: number; index: number }
  /** Choose item `sel` (1-based, 0 none) of a combo / list box. */
  | { cmd: 'formControlSelect'; sheet: number; index: number; sel: number }
  /** Set (`value`) or step (`step` arrow steps, pages with `page`) a scroll bar / spin button. */
  | {
      cmd: 'formControlValue';
      sheet: number;
      index: number;
      value?: number;
      step?: number;
      page?: boolean;
    }
  /** Format Control dialog: absent fields stay, an empty reference / macro clears it. */
  | { cmd: 'formatFormControl'; sheet: number; index: number; spec: FormatControlSpec }
  /** Move a control (its size in cells is kept); `offset` in points. */
  | {
      cmd: 'moveFormControl';
      sheet: number;
      index: number;
      row: number;
      col: number;
      offset?: [number, number];
    }
  /** Delete a form control (ActiveX controls are kept). */
  | { cmd: 'deleteFormControl'; sheet: number; index: number }
  /**
   * Solver (`simplex` default, `nonlinear`, `evolutionary`; integer / binary / `dif` by branch
   * and bound or permutation genes); `apply` enters the solution and `reports` are written on
   * new sheets (one undo step). Returns `SolverResult`.
   */
  | {
      cmd: 'solve';
      sheet: number;
      objective: string;
      goal: 'max' | 'min' | { value: number };
      variables: string[];
      constraints?: {
        lhs: string;
        relation: SolverRelation;
        rhs?: string | null;
      }[];
      nonNegative?: boolean;
      method?: SolverMethod;
      options?: SolverOptions;
      reports?: SolverReportKind[];
      apply?: boolean;
    }
  /**
   * Analysis tool on the values of `input` (and `input2`: second sample, histogram bins or
   * regression x); writes the output table at `dest` or on a new sheet named `sheetName`.
   * Returns `{ sheet, range }`; errors carry `analysis: <code>` (see `AnalysisErrorCode`).
   */
  | {
      cmd: 'analysis';
      sheet: number;
      tool: AnalysisTool;
      input?: string | null;
      input2?: string | null;
      dest?: CellAt | null;
      sheetName?: string | null;
    }
  /** What-if Goal Seek (A1 addresses); `apply` enters the value found. */
  | {
      cmd: 'goalSeek';
      sheet: number;
      target: string;
      goal: number;
      changing: string;
      apply?: boolean;
    }
  /** What-if scenarios of a sheet → {@link ScenarioList}. */
  | { cmd: 'scenarios'; sheet: number }
  /** Add (no `index`) or replace a scenario → `{ index }`. `cells`: A1 cells / ranges
   *  separated by commas or spaces; `values`: one per cell (absent: current values). */
  | {
      cmd: 'scenarioSave';
      sheet: number;
      index?: number | null;
      name: string;
      cells: string;
      values?: string[] | null;
      comment?: string | null;
      user?: string | null;
      locked?: boolean;
      hidden?: boolean;
    }
  | { cmd: 'scenarioDelete'; sheet: number; index: number }
  /** Enter a scenario's values into its changing cells (one undo step). */
  | { cmd: 'scenarioShow'; sheet: number; index: number }
  /** Copy the scenarios of sheet `from` → `{ merged }`. */
  | { cmd: 'scenarioMerge'; sheet: number; from: number }
  /** Summary report on a new sheet → `{ sheet }`. */
  | {
      cmd: 'scenarioSummary';
      sheet: number;
      results?: string | null;
      labels?: ScenarioSummaryLabels;
    };

/** Distribution of the `random` analysis tool (`discrete` reads value / probability pairs). */
export type AnalysisRandomDist =
  | { kind: 'uniform'; lo: number; hi: number }
  | { kind: 'normal'; mean: number; sd: number }
  | { kind: 'bernoulli'; p: number }
  | { kind: 'binomial'; p: number; trials: number }
  | { kind: 'poisson'; lambda: number }
  | { kind: 'discrete' };

/** An analysis tool with its options (`bo_sheet::analysis::Tool`). */
export type AnalysisTool =
  | {
      tool: 'descriptive';
      byRows?: boolean;
      labels?: boolean;
      confidence?: number | null;
      kthLargest?: number | null;
      kthSmallest?: number | null;
    }
  | { tool: 'correlation' | 'covariance' | 'rankPercentile'; byRows?: boolean; labels?: boolean }
  | { tool: 'anovaSingle'; byRows?: boolean; labels?: boolean; alpha?: number }
  | { tool: 'anovaTwoFactor'; labels?: boolean; alpha?: number }
  | { tool: 'anovaTwoFactorRep'; rowsPerSample: number; alpha?: number }
  | { tool: 'fTest'; labels?: boolean; alpha?: number }
  | {
      tool: 'tTestPaired' | 'tTestEqual' | 'tTestUnequal';
      labels?: boolean;
      alpha?: number;
      hypothesized?: number;
    }
  | {
      tool: 'zTest';
      labels?: boolean;
      alpha?: number;
      hypothesized?: number;
      varA: number;
      varB: number;
    }
  | { tool: 'expSmoothing'; labels?: boolean; damping?: number; stdErrors?: boolean }
  | { tool: 'movingAverage'; labels?: boolean; interval?: number; stdErrors?: boolean }
  | { tool: 'histogram'; labels?: boolean; pareto?: boolean; cumulative?: boolean }
  | {
      tool: 'random';
      columns: number;
      count: number;
      seed?: number | null;
      dist: AnalysisRandomDist;
    }
  | {
      tool: 'sampling';
      labels?: boolean;
      method: { kind: 'periodic'; period: number } | { kind: 'random'; count: number };
      seed?: number | null;
    }
  | {
      tool: 'regression';
      labels?: boolean;
      constant?: boolean;
      confidence?: number;
      residuals?: boolean;
      stdResiduals?: boolean;
    }
  | { tool: 'fourier'; labels?: boolean; inverse?: boolean };

/** Codes in `analysis: <code>` errors of the `analysis` command. */
export const ANALYSIS_ERROR_CODES = [
  'nonNumeric',
  'tooFew',
  'shape',
  'powerOfTwo',
  'param',
  'tooLarge',
  'badRange',
  'noInput',
  'overlap',
  'badName',
] as const;
export type AnalysisErrorCode = (typeof ANALYSIS_ERROR_CODES)[number];

/** A what-if scenario as the engine lists it. */
export interface ScenarioInfo {
  index: number;
  name: string;
  /** Changing cells (A1). */
  cells: string[];
  /** One value per changing cell, as text. */
  values: string[];
  comment: string | null;
  user: string | null;
  locked: boolean;
  hidden: boolean;
}

/** The scenarios of a sheet. */
export interface ScenarioList {
  items: ScenarioInfo[];
  current: number | null;
  show: number | null;
  /** Result cells chosen last for the summary (space-separated A1). */
  results: string;
  /** Scenarios are protected with the sheet. */
  protected: boolean;
}

/** Captions of the scenario summary sheet (`{sheet}` in `title` = source sheet name). */
export interface ScenarioSummaryLabels {
  sheet?: string;
  title?: string;
  current?: string;
  changing?: string;
  results?: string;
  comment?: string;
  note?: string;
}

/** Colours of a slicer or timeline style (`#RRGGBB`), resolved from the workbook theme. */
export interface FrameColors {
  selected: string;
  selectedText: string;
  header: string;
  headerText: string;
  border: string;
}

/** A pivot table a slicer or timeline may filter (every pivot table on its cache). */
export interface FrameConnection {
  sheet: number;
  sheetName: string;
  index: number;
  name: string;
  connected: boolean;
}

/** A timeline as the engine lists it. */
export interface TimelineInfo {
  index: number;
  name: string;
  caption: string;
  /** The date field of the pivot cache. */
  field?: string;
  /** 0 years, 1 quarters, 2 months, 3 days. */
  level: number;
  style?: string | null;
  colors?: FrameColors;
  from: [number, number];
  to: [number, number];
  filtered: boolean;
  periods: { label: string; start: number; end: number; selected: boolean }[];
  pivots?: FrameConnection[];
}

/** Kinds of form controls; `activeX` controls are kept as they are (fallback picture). */
export type FormControlKind =
  | 'button'
  | 'checkBox'
  | 'comboBox'
  | 'listBox'
  | 'optionButton'
  | 'scrollBar'
  | 'spinner'
  | 'groupBox'
  | 'label'
  | 'activeX';

export type CheckState = 'unchecked' | 'checked' | 'mixed';

/** Settings of the Format Control dialog. */
export interface FormatControlSpec {
  text?: string;
  name?: string;
  link?: string;
  inputRange?: string;
  checked?: CheckState;
  val?: number;
  min?: number;
  max?: number;
  inc?: number;
  page?: number;
  dropLines?: number;
  noThreeD?: boolean;
  horiz?: boolean;
  macroName?: string;
}

/** A form control as the engine lists it (state read from its linked cell). */
export interface FormControlInfo {
  index: number;
  kind: FormControlKind;
  name: string;
  text: string;
  link: string | null;
  inputRange: string | null;
  /** Items of a combo / list box. */
  items: string[] | null;
  checked: CheckState;
  /** Chosen item, 1-based (0 none). */
  sel: number;
  val: number;
  min: number;
  max: number;
  inc: number;
  page: number;
  dropLines: number;
  horiz: boolean;
  noThreeD: boolean;
  macro: string | null;
  /** Option-button group (index), `null` for other kinds. */
  group: number | null;
  /** Anchor cells [row, col] and offsets into them in points [x, y]. */
  from: [number, number];
  fromOff: [number, number];
  to: [number, number];
  toOff: [number, number];
  /** Fallback picture of an ActiveX control (data URL). */
  image: string | null;
  isNew: boolean;
}

/** A slicer as the engine lists it. */
export interface SlicerInfo {
  index: number;
  name: string;
  caption: string;
  /** `pivot`: filters a pivot table field; `table`: a table column. */
  kind?: 'pivot' | 'table';
  field?: string;
  style?: string | null;
  colors?: FrameColors;
  /** Pivot tables on the slicer's cache (pivot slicers). */
  pivots?: FrameConnection[];
  columns: number;
  /** Anchor cells [row, col]: top-left and bottom-right. */
  from: [number, number];
  to: [number, number];
  items: { label: string; selected: boolean }[];
}

/**
 * SmartArt in BlueSheet (CORE-SMART-005, `bo_engine::sheet::smartart_cmd`): the BlueOffice layout
 * library over a range of cells; anchors are `[row, col]`, `to` the cell holding the bottom-right
 * corner. `insertSmartArt` / `setSmartArt` return `{ index, smartArt }`.
 */
export type SheetSmartArtCommand =
  | {
      cmd: 'insertSmartArt';
      sheet: number;
      layout: SlideSmartArtLayout;
      items: SlideSmartArtItem[];
      colors?: SlideSmartArtColors;
      style?: SlideSmartArtStyle;
      from?: [number, number];
      to?: [number, number];
    }
  | {
      cmd: 'setSmartArt';
      sheet: number;
      index: number;
      layout?: SlideSmartArtLayout;
      items?: SlideSmartArtItem[];
      colors?: SlideSmartArtColors;
      style?: SlideSmartArtStyle;
    }
  | { cmd: 'smartArtInfo'; sheet: number; index: number }
  | {
      cmd: 'moveSmartArt';
      sheet: number;
      index: number;
      from: [number, number];
      to: [number, number];
    }
  | { cmd: 'deleteSmartArt'; sheet: number; index: number }
  /** Replace the diagram by a group of ordinary shapes in the same cells (one undo step). */
  | { cmd: 'smartArtToShapes'; sheet: number; index: number }
  /** Thumbnails of every layout of the library: the result is `{ previews }`. */
  | {
      cmd: 'smartArtPreviews';
      colors?: SlideSmartArtColors;
      style?: SlideSmartArtStyle;
      width?: number;
      height?: number;
    };

/** A SmartArt diagram of a sheet (result of `smartArtInfo`). */
export interface SheetSmartArtInfo {
  index: number;
  name: string;
  layout: SlideSmartArtLayout;
  items: SlideSmartArtItem[];
  colors: SlideSmartArtColors;
  style: SlideSmartArtStyle;
  from: [number, number];
  to: [number, number];
}

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
  /** `c:pivotSource` name of a pivot chart (`Sheet!Pivot`). */
  pivot?: string | null;
  /** Sheet and index of the pivot table a pivot chart plots (`null` when it is gone). */
  pivotAt?: [number, number] | null;
  /** Region map options (`null` for other charts; CORE-CHART-006). */
  map?: ChartMapInfo | null;
  /** The chart's elements and their options (`null` for chartex charts; CORE-CHART-005). */
  elements?: import('./chartElements').ChartElementsInfo | null;
}

/** Editable options of a region map (chartex `regionMap`). */
export interface ChartMapOptions {
  /** `mercator` | `miller` | `robinson` | `albers`; `''` = automatic. */
  projection: string;
  /** Area level (`countryRegion`, `world`, `state` …); `''` = automatic. */
  level: string;
  /** Region labels: `none` | `bestFitOnly` | `showAll`. */
  labels: string;
  /** Colour scale. */
  scale: 'sequential' | 'diverging';
  /** `RRGGBB` colours; `''` = default. */
  minColor: string;
  midColor: string;
  maxColor: string;
}

/** A region map as `charts` lists it. */
export interface ChartMapInfo {
  projection: string | null;
  level: string | null;
  labels: string | null;
  scale: 'sequential' | 'diverging';
  categorical: boolean;
  minColor: string | null;
  midColor: string | null;
  maxColor: string | null;
  /** `needsDataset`: a map of provinces without a boundary dataset for its locations. */
  status: 'ok' | 'needsDataset';
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
  extra: PrintSetupInfo;
}

/** Header / footer texts with `&` codes (ECMA-376 Part 1, §18.3.1.46). */
export interface HeaderFooterSpec {
  oddHeader: string;
  oddFooter: string;
  evenHeader: string;
  evenFooter: string;
  firstHeader: string;
  firstFooter: string;
  differentOddEven: boolean;
  differentFirst: boolean;
  scaleWithDoc?: boolean;
  alignWithMargins?: boolean;
}

export type PrintErrors = 'displayed' | 'blank' | 'dash' | 'NA';
export type PrintComments = 'none' | 'atEnd' | 'asDisplayed';
export type PageOrder = 'downThenOver' | 'overThenDown';

/** Optional `setPageSetup` fields beyond the basics. */
// A type alias (not an interface) so `setPageSetup` stays assignable to `{ cmd: string; [k: string]: unknown }`.
export type PrintSetupExtra = {
  titleCols?: [number, number] | null;
  printAreas?: SheetRange[];
  headings?: boolean;
  centerH?: boolean;
  centerV?: boolean;
  pageOrder?: PageOrder;
  firstPageNumber?: number | null;
  blackAndWhite?: boolean;
  draft?: boolean;
  errors?: PrintErrors;
  comments?: PrintComments;
  /** Header and footer margins in inches. */
  hfMargins?: [number, number];
  headerFooter?: HeaderFooterSpec;
};

/** `pageSetup` query: the extra print settings. */
export interface PrintSetupInfo {
  titleCols: [number, number] | null;
  headings: boolean;
  centerH: boolean;
  centerV: boolean;
  pageOrder: PageOrder;
  firstPageNumber: number | null;
  blackAndWhite: boolean;
  draft: boolean;
  errors: PrintErrors;
  comments: PrintComments;
  hfMargins: [number, number];
  headerFooter: Required<HeaderFooterSpec>;
  /** 1-based first row / column of each page after a manual break. */
  rowBreaks: number[];
  colBreaks: number[];
}

/** `printPreview` result. */
export interface PrintPreviewInfo {
  pages: number;
  page: PageDisplayList | null;
}

export interface SheetHost {
  init(): Promise<void>;
  /**
   * Opens a workbook; encrypted files ([MS-OFFCRYPTO]) need `password`: the
   * promise rejects with `password required` / `wrong password`.
   */
  open(bytes: Uint8Array, name: string, password?: string): Promise<WorkbookInfo>;
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
  /** PDF of a print job (CORE-PRINT-001). */
  printPdf?(options: import('./print').PrintOptions): Promise<Uint8Array>;
  /** Sheet `index` of a print job's PDF (print panel preview). */
  printPreview?(
    options: import('./print').PrintOptions,
    index: number,
  ): Promise<import('./print').PrintPreview>;
  getGlyphOutlines(face: number, glyphs: number[]): Promise<GlyphOutline[]>;
  /** Runs an editing command; the result shape depends on the command. */
  exec<T = unknown>(cmd: SheetCommand): Promise<T>;
  /**
   * Notifies about changes made outside this editor (collaborators); the UI
   * re-reads info and redraws. Hosts without collaboration omit it.
   */
  onExternalChange?(listener: () => void): () => void;
  /** Binds the workbook to a live session (`bo_engine::sheet::collab`); the seeder gets its change. */
  collabStart?(seed: boolean, clientId: number): Promise<SheetCollabChange>;
  /** Local changes to share since the last call. */
  collabLocal?(): Promise<SheetCollabChange>;
  /** Applies a replica change (`WasmCollab.takeRemote`). */
  collabApply?(change: string): Promise<WorkbookInfo>;
  collabStop?(): Promise<void>;
  /**
   * Live collaboration: publish this user's selection (sheet name + A1 range) as
   * presence. Hosts without collaboration omit it.
   */
  collabSelect?(sheet: string, range: string): void;
  /** Collaborators' selected cells / ranges (presence), on every change. */
  onSheetPeers?(listener: (peers: SheetPeer[]) => void): () => void;
}

/** A `bo-collab` change envelope (cells, meta, texts …) as JSON. */
export type SheetCollabChange = Record<string, unknown>;

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
  | 'printPdf'
  | 'printPreview'
  | 'exec'
  | 'collabStart'
  | 'collabLocal'
  | 'collabApply'
  | 'collabStop'
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
  open(bytes: Uint8Array, name: string, password?: string) {
    return this.call<WorkbookInfo>('open', bytes, name, password ?? null);
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
  printPdf(options: import('./print').PrintOptions) {
    return this.call<Uint8Array>('printPdf', JSON.stringify(options));
  }
  printPreview(options: import('./print').PrintOptions, index: number) {
    return this.call<import('./print').PrintPreview>(
      'printPreview',
      JSON.stringify(options),
      index,
    );
  }
  getGlyphOutlines(face: number, glyphs: number[]) {
    return this.call<GlyphOutline[]>('glyphs', face, glyphs);
  }
  exec<T = unknown>(cmd: SheetCommand) {
    return this.call<T>('exec', JSON.stringify(cmd));
  }
  collabStart(seed: boolean, clientId: number) {
    return this.call<SheetCollabChange>('collabStart', seed, clientId);
  }
  collabLocal() {
    return this.call<SheetCollabChange>('collabLocal');
  }
  collabApply(change: string) {
    return this.call<WorkbookInfo>('collabApply', change);
  }
  collabStop() {
    return this.call<void>('collabStop');
  }
}
