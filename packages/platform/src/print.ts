/**
 * Print jobs (JSON `bo_engine::print::PrintOptions`, CORE-PRINT-001): one pipeline for
 * BlueDoc, BlueSlide and BlueSheet that produces the PDF that is printed. The print
 * panel's preview shows sheet `index` of exactly that PDF (`printPreview`).
 */
import type { PageDisplayList } from './types';

/** Which pages: every page, the current one, the selection, or the `range` text. */
export type PrintPageChoice = 'all' | 'current' | 'selection' | 'range';
export type PrintSubset = 'all' | 'odd' | 'even';
export type PrintPerSheet = 1 | 2 | 4 | 6 | 8 | 16;
export type PrintPaper = 'a3' | 'a4' | 'a5' | 'b5' | 'letter' | 'legal';
export type PrintColor = 'color' | 'grayscale' | 'blackWhite';
export type SlidePrintLayout = 'slides' | 'notes' | 'outline' | 'handouts';
export type SheetPrintScope = 'active' | 'workbook' | 'selection';

export interface DocPrintOptions {
  /** Print tracked changes and comments with their markup (default on). */
  markup?: boolean;
  /** Add a document properties page. */
  properties?: boolean;
  /** Labels of the properties page by key (`title`, `author`, …). */
  labels?: Record<string, string>;
}

export interface SlidePrintOptions {
  layout?: SlidePrintLayout;
  handoutPerPage?: 1 | 2 | 3 | 4 | 6 | 9;
  handoutOrder?: 'horizontal' | 'vertical';
  /** Print hidden slides. */
  hidden?: boolean;
}

/** BlueSheet choices; a missing field keeps the sheet's own page setup. */
export interface SheetPrintOptions {
  scope?: SheetPrintScope;
  /** 0-based sheets of `active` / `selection`. */
  sheets?: number[];
  /** Selected cells (`B2:D10`) for `selection`. */
  selection?: string;
  gridlines?: boolean;
  headings?: boolean;
  /** Rows repeated on every page (`1:2`; empty = none). */
  titleRows?: string;
  /** Columns repeated on every page (`A:B`; empty = none). */
  titleCols?: string;
  ignorePrintArea?: boolean;
}

export interface PrintOptions {
  pages?: PrintPageChoice;
  /** `1-3, 5, 8-` (BlueDoc also `p2s1-p3s2`: page 2 of section 1 …). */
  range?: string;
  /** 0-based current page / slide. */
  current?: number;
  /** 0-based selected pages / slides. */
  selected?: number[];
  subset?: PrintSubset;
  copies?: number;
  collate?: boolean;
  reverse?: boolean;
  perSheet?: PrintPerSheet;
  order?: 'across' | 'down';
  /** Scale to this paper; omitted = the page's own size. */
  paper?: PrintPaper;
  /** Thin frame around every page (BlueSlide: frame slides). */
  frame?: boolean;
  color?: PrintColor;
  doc?: DocPrintOptions;
  slide?: SlidePrintOptions;
  sheet?: SheetPrintOptions;
  /**
   * Preview job number: the engine reuses the sheets it built while the options JSON
   * (this number included) stays the same; the panel changes it each time it opens.
   */
  job?: number;
}

/** `printPreview` result. */
export interface PrintPreview {
  /** Sheets of paper in one copy. */
  sheets: number;
  copies: number;
  /** Index of `page` (clamped). */
  index: number;
  /** The sheet, `null` when nothing would be printed. */
  page: PageDisplayList | null;
}

/** Hosts that print through the shared pipeline. */
export interface PrintHost {
  printPdf(options: PrintOptions): Promise<Uint8Array>;
  printPreview(options: PrintOptions, index: number): Promise<PrintPreview>;
}
