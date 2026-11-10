/**
 * BlueScript API (CORE-SCRIPT-001) — typings for scripts run from the BlueScript
 * pane of BlueDoc, BlueSheet and BlueSlide.
 *
 * Scripts are JavaScript (ES2023) run in a sandbox: no DOM, network, storage,
 * timers or platform API. Top-level `await` and `return` work. Every API call is
 * asynchronous — `await` it. A whole run is one undo step.
 *
 * Only the object of the open application is defined: `doc` in BlueDoc,
 * `workbook` and `sheet` in BlueSheet, `presentation` in BlueSlide.
 */

// ------------------------------------------------------------------ common

interface ScriptConsole {
  log(...values: unknown[]): void;
  info(...values: unknown[]): void;
  warn(...values: unknown[]): void;
  error(...values: unknown[]): void;
}

declare const console: ScriptConsole;

/** `#RRGGBB` or `RRGGBB`. */
type Color = string;

// ------------------------------------------------------------------ BlueDoc

interface CharacterFormat {
  bold?: boolean;
  italic?: boolean;
  underline?: boolean;
  strike?: boolean;
  superscript?: boolean;
  subscript?: boolean;
  font?: string;
  /** Points. */
  size?: number;
  color?: Color | null;
  highlight?: Color | null;
}

interface ParagraphFormat {
  align?: 'left' | 'center' | 'right' | 'justify';
  /** Points. */
  spaceBefore?: number;
  spaceAfter?: number;
  indentLeft?: number;
  indentRight?: number;
  indentFirstLine?: number;
  /** Paragraph style id (`Heading1`, `Normal` …). */
  style?: string;
  bullets?: boolean;
  numbering?: boolean;
}

interface FindOptions {
  matchCase?: boolean;
  wholeWord?: boolean;
  regex?: boolean;
  wildcards?: boolean;
}

/** A match; offsets are string indexes into the paragraph text. */
interface DocMatch {
  paragraph: number;
  start: number;
  endParagraph: number;
  end: number;
}

interface DocPosition {
  paragraph: number;
  offset: number;
}

interface DocInfo {
  name: string;
  pageCount: number;
  wordCount: number;
  modified: boolean;
}

/** A body paragraph (a snapshot: re-read `doc.paragraphs` after edits). */
interface Paragraph {
  readonly index: number;
  /** Text; embedded objects are U+FFFC. */
  readonly text: string;
  /** Paragraph style id. */
  getStyle(): Promise<string | null>;
  setStyle(styleId: string): Promise<void>;
  /** Replace the whole text. */
  setText(text: string): Promise<void>;
  /** Character formatting of the whole paragraph. */
  format(format: CharacterFormat): Promise<void>;
  setFormat(format: ParagraphFormat): Promise<void>;
  select(start?: number, end?: number): Promise<void>;
  /** New paragraph after this one; resolves to its index. */
  insertAfter(text: string, styleId?: string): Promise<number>;
  delete(): Promise<void>;
}

interface BlueDocument {
  readonly info: Promise<DocInfo>;
  /** All text, paragraphs separated by `\n`. */
  readonly text: Promise<string>;
  readonly paragraphs: Promise<Paragraph[]>;
  paragraph(index: number): Promise<Paragraph>;
  readonly selection: Promise<{ anchor: DocPosition; focus: DocPosition }>;
  /** Add a paragraph at the end; resolves to its index. */
  appendParagraph(text: string, styleId?: string): Promise<number>;
  /** Type text at the selection (`\n` starts a new paragraph). */
  insertText(text: string): Promise<void>;
  select(paragraph: number, start?: number, end?: number): Promise<void>;
  selectAll(): Promise<void>;
  /** Character formatting of the selection. */
  format(format: CharacterFormat): Promise<void>;
  find(query: string, options?: FindOptions): Promise<DocMatch[]>;
  /** Replace every match; resolves to the number replaced. */
  replace(query: string, replacement: string, options?: FindOptions): Promise<number>;
  /** Table at the selection, optionally filled row by row. */
  insertTable(rows: number, cols: number, values?: (string | number | null)[][]): Promise<void>;
  insertPageBreak(): Promise<void>;
  /** Any BlueDoc engine command (`{ cmd: 'insertToc' }` …); resolves to the engine result. */
  command(cmd: { cmd: string; [key: string]: unknown }): Promise<unknown>;
}

declare const doc: BlueDocument;

// ------------------------------------------------------------------ BlueSheet

type CellValue = string | number | boolean | null;

interface RangeFormat {
  /** Number format code (`#,##0.00`, `0%`, `dd/mm/yyyy` …). */
  numFmt?: string;
  fontName?: string;
  /** Points. */
  fontSize?: number;
  bold?: boolean;
  italic?: boolean;
  underline?: 'none' | 'single' | 'double';
  strike?: boolean;
  fontColor?: Color;
  fillColor?: Color;
  hAlign?: 'general' | 'left' | 'center' | 'right' | 'fill' | 'justify';
  vAlign?: 'top' | 'center' | 'bottom';
  wrap?: boolean;
  indent?: number;
  rotation?: number;
  shrink?: boolean;
  locked?: boolean;
  formulaHidden?: boolean;
}

/** Cells of a sheet (`A1`, `B2:D10`, `'Other sheet'!A1:B2`). */
interface Range {
  readonly sheet: number;
  readonly address: string;
  /** Values (formulas: their results), row by row. */
  readonly values: Promise<CellValue[][]>;
  /** Inputs as typed (`=SUM(A1:A3)`, `12`, `text`). */
  readonly formulas: Promise<string[][]>;
  /** Write values starting at the top-left cell (strings are entered as typed). */
  setValues(values: CellValue[][]): Promise<number>;
  setFormulas(formulas: string[][]): Promise<number>;
  /** The same value in every cell. */
  setValue(value: CellValue): Promise<void>;
  format(format: RangeFormat): Promise<void>;
  clear(what?: 'all' | 'contents' | 'formats'): Promise<void>;
  merge(center?: boolean): Promise<void>;
  borders(
    preset: 'outline' | 'all' | 'inside' | 'none' | 'top' | 'bottom' | 'left' | 'right',
    style?: string,
    color?: Color,
  ): Promise<void>;
}

interface Sheet {
  readonly index: number;
  readonly name: string;
  range(address: string): Range;
  /** 0-based row and column. */
  cell(row: number, col: number): Range;
  rename(name: string): Promise<void>;
  /** Chart of the data in `address` (`column`, `bar`, `line`, `pie`, `area`, `scatter` …); resolves to its index. */
  addChart(kind: string, address: string, options?: { title?: string }): Promise<number | null>;
  readonly charts: Promise<unknown[]>;
}

interface Workbook {
  readonly sheets: Promise<Sheet[]>;
  readonly activeSheet: Promise<Sheet>;
  sheet(nameOrIndex: string | number): Promise<Sheet>;
  addSheet(name?: string): Promise<Sheet>;
  /** Any BlueSheet engine command (`{ cmd: 'autofit', … }` …). */
  command(cmd: { cmd: string; [key: string]: unknown }): Promise<unknown>;
}

declare const workbook: Workbook;
/** The sheet shown when the script started. */
declare const sheet: Sheet;

// ------------------------------------------------------------------ BlueSlide

/** Points, top-left corner. */
interface Rect {
  x: number;
  y: number;
  w: number;
  h: number;
}

interface TextFormat {
  bold?: boolean;
  italic?: boolean;
  underline?: boolean;
  strike?: boolean;
  /** Points. */
  size?: number;
  color?: Color | null;
  font?: string;
}

interface ChartData {
  categories: string[];
  series: { name: string; values: (number | null)[] }[];
}

/** A shape on a slide (a snapshot). */
interface Shape {
  readonly slide: number;
  readonly id: number;
  readonly name: string;
  readonly kind:
    | 'shape'
    | 'picture'
    | 'group'
    | 'connector'
    | 'table'
    | 'chart'
    | 'smartart'
    | 'object'
    | 'graphic';
  /** Text (`\n` between paragraphs), null when the shape holds none. */
  readonly text: string | null;
  readonly rect: Rect & { rotation: number };
  setText(text: string): Promise<void>;
  format(format: TextFormat): Promise<void>;
  setFill(color: Color | null): Promise<void>;
  setLine(color: Color | null, width?: number): Promise<void>;
  /** Move the top-left corner to (x, y). */
  move(x: number, y: number): Promise<void>;
  delete(): Promise<void>;
}

interface Slide {
  readonly index: number;
  readonly shapes: Promise<Shape[]>;
  readonly notes: Promise<string>;
  setNotes(text: string): Promise<void>;
  addTextBox(text: string, rect?: Rect): Promise<Shape>;
  /** Preset geometry (`rect`, `ellipse`, `roundRect`, `rightArrow` …). */
  addShape(preset: string, rect?: Rect, text?: string): Promise<Shape>;
  addTable(rows: number, cols: number, rect?: Rect): Promise<Shape>;
  addChart(
    kind: string,
    data?: ChartData,
    options?: { title?: string; rect?: Rect },
  ): Promise<Shape>;
  delete(): Promise<void>;
  duplicate(): Promise<Slide>;
}

interface Presentation {
  readonly info: Promise<{ name: string; slideCount: number; current: number }>;
  readonly slides: Promise<Slide[]>;
  slide(index: number): Slide;
  /** New slide after `after` (default: the current one), with layout index `layout`. */
  addSlide(after?: number, layout?: number): Promise<Slide>;
  /** Any BlueSlide engine command (`{ cmd: 'setTransition', … }` …). */
  command(cmd: { cmd: string; [key: string]: unknown }): Promise<unknown>;
}

declare const presentation: Presentation;
