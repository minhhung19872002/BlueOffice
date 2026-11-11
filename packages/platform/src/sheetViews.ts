/**
 * BlueSheet workbook views (SHEET-VIEW-001, `bo_engine::sheet::view_cmd`): view type
 * (ECMA-376 Part 1, §18.3.1.87 `sheetView/@view`), show formulas, per-view zoom,
 * page break preview with draggable breaks, the page layout view and custom views
 * (§18.3.1.25 `customSheetView`, §18.2.2 `customWorkbookView`).
 */

/** §18.18.69 ST_SheetViewType. */
export type SheetViewMode = 'normal' | 'pageBreakPreview' | 'pageLayout';

/** Result of `sheetViewState` / `setSheetView`. */
export interface SheetViewState {
  view: SheetViewMode;
  showFormulas: boolean;
  /** The outline bar is shown when the sheet has an outline (`showOutlineSymbols`). */
  outlineSymbols?: boolean;
  /** Zoom percent of the current view type. */
  zoom: number;
  /** Zoom percent kept for each view type. */
  zooms: Record<SheetViewMode, number>;
}

/** A break between two pages: `at` is the first row / column of the next page. */
export interface PageBreakInfo {
  at: number;
  manual: boolean;
}

/** One print area of `pageBreakPreview`. */
export interface BreakPreviewArea {
  r0: number;
  c0: number;
  r1: number;
  c1: number;
  scale: number;
  /** Column / row spans of the pages, `[first, last]`. */
  cols: [number, number][];
  rows: [number, number][];
  /** Pages in print order: indexes into `cols` / `rows` and the page number. */
  pages: { col: number; row: number; number: number }[];
  rowBreaks: PageBreakInfo[];
  colBreaks: PageBreakInfo[];
}

/** Result of `pageBreakPreview`. */
export interface BreakPreview {
  firstPage: number;
  areas: BreakPreviewArea[];
}

/** One printed page of `pageLayout` (points). */
export interface PageLayoutPage {
  width: number;
  height: number;
  /** Left, right, top, bottom, header, footer margins. */
  margins: [number, number, number, number, number, number];
  /** Cells on the page (`null`: an end-of-sheet comments page). */
  cells: {
    r0: number;
    c0: number;
    r1: number;
    c1: number;
    scale: number;
    /** Page position of the first body cell. */
    x: number;
    y: number;
  } | null;
}

/** A custom view of the workbook. */
export interface CustomViewInfo {
  name: string;
  guid: string;
  includePrintSettings: boolean;
  includeHiddenRowCol: boolean;
}

/** Result of `applyCustomView`: where the editor goes. */
export interface AppliedCustomView {
  sheet: number;
  zoom: number | null;
  active: [number, number] | null;
  topLeft: [number, number] | null;
}

/** View commands. */
export type SheetViewCommand =
  /** → {@link SheetViewState}. */
  | { cmd: 'sheetViewState'; sheet: number }
  /** No undo step → {@link SheetViewState}. */
  | {
      cmd: 'setSheetView';
      sheet: number;
      view?: SheetViewMode;
      showFormulas?: boolean;
      outlineSymbols?: boolean;
      zoom?: number;
    }
  /** → {@link BreakPreview}. */
  | { cmd: 'pageBreakPreview'; sheet: number }
  /** Moves (or with no `from`, inserts) a break; one undo step → `{ scale: number | null }`. */
  | {
      cmd: 'movePageBreak';
      sheet: number;
      rows: boolean;
      area?: number;
      from?: number;
      to: number;
    }
  /** → `{ pages: PageLayoutPage[] }`. */
  | { cmd: 'pageLayout'; sheet: number }
  /** → `{ row, col } | null`. */
  | { cmd: 'pageHit'; sheet: number; page: number; x: number; y: number }
  /** → {@link CustomViewInfo}[]. */
  | { cmd: 'customViews' }
  /** One undo step → `{ guid }`. */
  | {
      cmd: 'addCustomView';
      name: string;
      sheet: number;
      includePrintSettings?: boolean;
      includeHiddenRowCol?: boolean;
      zoom?: number;
      active?: [number, number];
      sqref?: string;
      topLeft?: [number, number];
    }
  /** One undo step → {@link AppliedCustomView}. */
  | { cmd: 'applyCustomView'; guid: string }
  /** One undo step. */
  | { cmd: 'deleteCustomView'; guid: string };
