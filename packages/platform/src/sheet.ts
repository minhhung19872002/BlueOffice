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
}

export interface WorkbookInfo {
  name: string;
  sheets: SheetSummary[];
}

export type SheetHit =
  | { kind: 'cell'; row: number; col: number }
  | { kind: 'columnHeader'; col: number }
  | { kind: 'rowHeader'; row: number }
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
}
