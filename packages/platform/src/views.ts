/**
 * BlueDoc views (DOC-VIEW-001): view layouts, formatting marks, outline and
 * navigation-pane find results. Mirrors `bo_engine::views`.
 */
import type { FindMatch, FindOptions } from './types';

/** Engine layout of the view (`bo_engine::views::ViewMode`). */
export type ViewLayoutMode = 'print' | 'web' | 'draft';

/** Kind of a formatting mark (`bo_engine::views::FormatMarkKind`). */
export type FormatMarkKind =
  | 'paragraph'
  | 'space'
  | 'noBreakSpace'
  | 'tab'
  | 'lineBreak'
  | 'pageBreak'
  | 'columnBreak'
  | 'anchor'
  /** End of a table cell (¤ in place of ¶). */
  | 'cellEnd'
  /** End of a table row (¤ right of the row). */
  | 'rowEnd';

/** A formatting mark on a page (points), drawn by the view only. */
export interface FormatMark {
  kind: FormatMarkKind;
  x: number;
  w: number;
  top: number;
  h: number;
  baseline: number;
  size: number;
}

/** A top-level body paragraph of the outline (`bo_engine::views::OutlineItem`). */
export interface OutlineItem {
  para: number;
  /** 0-based outline level (0 = Heading 1); null = body text. */
  level: number | null;
  text: string;
  /** `text` is the paragraph's whole text: the outline view edits it in place. */
  editable?: boolean;
  section: number;
  /** 0-based page where the paragraph starts. */
  page: number;
}

/** A find match with context (`bo_engine::views::FindResult`). */
export interface FindResult {
  match: FindMatch;
  before: string;
  text: string;
  after: string;
  page: number;
}

/** View commands sent to the engine. */
export type ViewCommand =
  | { cmd: 'setViewLayout'; mode: ViewLayoutMode; width?: number }
  | { cmd: 'showFormattingMarks'; on: boolean }
  | { cmd: 'outline' }
  | { cmd: 'setOutlineLevel'; para?: number; level?: number | null }
  | { cmd: 'moveOutline'; para?: number; up: boolean }
  | { cmd: 'moveOutlineTo'; para: number; before?: number | null }
  /** Outline view: the paragraph's text typed in place (one undo step). */
  | { cmd: 'setOutlineText'; para: number; text: string }
  | { cmd: 'findResults'; query: string; options?: FindOptions };
