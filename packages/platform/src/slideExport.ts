/**
 * Options of `EngineHost.exportSlides` (JSON `bo_engine::slide_export::SlideExport`):
 * handout PDF, notes pages PDF, slide pictures (PNG / JPEG / SVG) and the
 * presentation package (zip of the deck, its media clips and OFL fonts).
 */

/** Paper of handout / notes pages; `notes` = the deck's notes size. */
export type SlidePaper = 'a4' | 'letter' | 'notes';

/**
 * Header and footer of handout / notes pages. A missing field is taken from the
 * handout / notes master; an empty string hides the item.
 */
export interface SlideHeaderFooter {
  header?: string;
  footer?: string;
  /** Date text, already formatted by the host. */
  date?: string;
  pageNumbers?: boolean;
}

export interface HandoutExport extends SlideHeaderFooter {
  kind: 'handout';
  /** Slides per page. */
  perPage: 1 | 2 | 3 | 4 | 6 | 9;
  order?: 'horizontal' | 'vertical';
  /** Thin frame around each slide (default on). */
  frame?: boolean;
  paper?: SlidePaper;
}

export interface NotesExport extends SlideHeaderFooter {
  kind: 'notes';
  paper?: SlidePaper;
}

export interface ImageExport {
  kind: 'image';
  format: 'png' | 'jpg' | 'svg';
  /** 0-based slides; omitted = all. Several slides give a zip. */
  slides?: number[];
  /** Width in pixels. */
  width: number;
  /** JPEG quality 1..100. */
  quality?: number;
}

export interface PackageExport {
  kind: 'package';
}

/**
 * The open document or deck as PDF (CORE-FMT-002): PDF/A level B conformance
 * (ISO 19005-1/-2/-3; `3b` embeds the source file), tagged PDF (logical
 * structure, ISO 32000-1 §14.8) and bookmarks (§12.3.3) from headings, slide
 * titles or sheet names. Also served for BlueDoc by `exportSlides`.
 */
export interface PdfExport {
  kind: 'pdf';
  pdfa?: PdfAPart;
  tagged?: boolean;
  bookmarks?: boolean;
}

/** PDF/A part, conformance level B. */
export type PdfAPart = '1b' | '2b' | '3b';

export type SlideExportOptions =
  HandoutExport | NotesExport | ImageExport | PackageExport | PdfExport;
