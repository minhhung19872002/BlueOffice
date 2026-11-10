/**
 * Platform Abstraction Layer (prompt §3.4).
 *
 * UI and editor code never call browser-only or Tauri-only APIs for
 * platform-dependent work: everything goes through {@link Platform}.
 * `apps/web` builds a `WebPlatform`, `apps/desktop` a `DesktopPlatform`;
 * the rest of the code base is shared 100%.
 */

import type { InspectFix, InspectReport, InspectCommand } from './inspect';
import type { FindResult, FormatMark, OutlineItem, ViewCommand } from './views';
import type { SignatureLineCommand, SignatureLineImages, SignatureLineInfo } from './sigline';
import type { CharAdvanced, CharAdvancedPatch, RubySpec } from './charfx';

// ---------------------------------------------------------------- engine data

/** Page size in points. */
export interface PageSize {
  width: number;
  height: number;
}

/** Summary of an open document (mirrors `bo_engine::DocInfo`). */
export interface DocInfo {
  name: string;
  pageCount: number;
  pages: PageSize[];
  wordCount: number;
  /** `slide` for presentations (pages are slides), `doc` otherwise. */
  kind?: 'doc' | 'slide';
  /** The file has macros: they are kept when saving but never run. */
  macros?: boolean;
  /** Runs converted from TCVN3 / VNI fonts to Unicode on open (undo restores them). */
  legacyConverted?: number;
  /** Saved with a password ([MS-OFFCRYPTO]). */
  protected?: boolean;
  /**
   * Package variant the next save produces (`docx`, `dotm`, `ppsx`, …): the opened
   * file's, by its main part's content type, unless `saveFormat` changed it.
   */
  variant?: string;
}

/** Error messages of `EngineHost.open` for protected files. */
export const PASSWORD_REQUIRED = 'password required';
export const WRONG_PASSWORD = 'wrong password';

export type Rgba = [number, number, number, number];
/** `[glyphId, x, y, cluster]` relative to the run origin. */
export type Glyph = [number, number, number, number];

/** Display list command (mirrors `bo_render::Command`). Units: points, y down. */
export type DisplayCommand =
  | { op: 'fillRect'; x: number; y: number; w: number; h: number; color: Rgba }
  | {
      op: 'line';
      x1: number;
      y1: number;
      x2: number;
      y2: number;
      width: number;
      color: Rgba;
      /** (on, off) dash lengths. */
      dash?: [number, number];
    }
  | {
      op: 'image';
      x: number;
      y: number;
      w: number;
      h: number;
      /** Image id (document media part or slide image), bytes via `EngineHost.getMedia`. */
      image: string;
      /** l, t, r, b fractions cut away. */
      crop?: [number, number, number, number];
      /** Degrees clockwise. */
      rotation?: number;
    }
  | {
      op: 'glyphs';
      face: number;
      size: number;
      x: number;
      y: number;
      color: Rgba;
      text: string;
      glyphs: Glyph[];
    }
  | { op: 'pushClip'; x: number; y: number; w: number; h: number }
  | { op: 'popClip' }
  | { op: 'path'; segs: PathSeg[]; fill: Paint | null; stroke: Stroke | null }
  /** Pre-multiply the transform `[a, b, c, d, e, f]` (canvas `transform()` order). */
  | { op: 'pushTransform'; m: [number, number, number, number, number, number] }
  | { op: 'popTransform' }
  /** Intersect the clip with a path (nonzero rule; paired with `popClip`). */
  | { op: 'pushClipPath'; segs: PathSeg[] }
  /** Multiply the opacity of what follows (paired with `popOpacity`). */
  | { op: 'pushOpacity'; alpha: number }
  | { op: 'popOpacity' }
  /** Filled path blurred by `radius` points (soft shadow). */
  /** Soft shadow / glow: the path grown by `spread` (glow, §20.1.8.32) and blurred by `radius`. */
  | { op: 'blurPath'; segs: PathSeg[]; color: Rgba; radius: number; spread?: number };

/** Path segment as serialised by serde (`bo_render::PathSeg`). */
export type PathSeg =
  | { M: [number, number] }
  | { L: [number, number] }
  | { Q: [number, number, number, number] }
  | { C: [number, number, number, number, number, number] }
  | 'Z';

export interface GradientStop {
  offset: number;
  color: Rgba;
}

export type Paint =
  | { type: 'solid'; color: Rgba }
  | { type: 'linear'; x1: number; y1: number; x2: number; y2: number; stops: GradientStop[] }
  | { type: 'radial'; cx: number; cy: number; r: number; stops: GradientStop[] };

export interface Stroke {
  width: number;
  color: Rgba;
  dash: number[];
  cap: string;
  join: string;
}

/** Display list of a page (mirrors `bo_render::PageList`). */
export interface PageDisplayList {
  width: number;
  height: number;
  commands: DisplayCommand[];
  /** Review markup drawn beside the page in the view only (BlueDoc). */
  markup?: PageMarkup;
  /** Formatting marks (¶ · →) when shown (view only, BlueDoc). */
  marks?: FormatMark[];
}

/** Kind of a mark in the review markup (mirrors `bo_layout::MarkKind`). */
export type MarkKind = 'insert' | 'delete' | 'format' | 'comment';

/** Change bar in the left margin (points; mirrors `bo_layout::ChangeBar`). */
export interface ChangeBar {
  x: number;
  top: number;
  bottom: number;
  color: string;
}

/** Balloon (or collapsed icon) in the markup area (points; mirrors `bo_layout::Balloon`). */
export interface Balloon {
  id: number;
  kind: MarkKind;
  author: string;
  date: string | null;
  text: string;
  done: boolean;
  color: string;
  x: number;
  y: number;
  w: number;
  h: number;
  anchorX: number;
  anchorY: number;
  icon: boolean;
}

/** Markup of a page: the view's page grows by `width` (mirrors `bo_layout::PageMarkup`). */
export interface PageMarkup {
  width: number;
  bars: ChangeBar[];
  balloons: Balloon[];
}

/** Glyph outline: SVG path in font units with y pointing down. */
export interface GlyphOutline {
  face: number;
  glyph: number;
  unitsPerEm: number;
  path: string;
}

// ---------------------------------------------------------------- editing (mirrors bo_engine::edit)

/** Caret position: body paragraph id and byte offset in its text. */
export interface Pos {
  p: number;
  o: number;
}

export interface Selection {
  anchor: Pos;
  focus: Pos;
}

export interface CaretRect {
  page: number;
  x: number;
  y: number;
  h: number;
}

export interface SelRect {
  page: number;
  x: number;
  y: number;
  w: number;
  h: number;
}

export type Alignment = 'Left' | 'Center' | 'Right' | 'Justify';

/** Formatting at the selection (ribbon toggle states). */
export interface FormatState {
  bold: boolean;
  italic: boolean;
  underline: boolean;
  strike: boolean;
  superscript: boolean;
  subscript: boolean;
  font: string | null;
  sizeHalfPoints: number | null;
  color: string | null;
  highlight: string | null;
  align: Alignment;
  style: string | null;
  bullets: boolean;
  numbering: boolean;
  canUndo: boolean;
  canRedo: boolean;
  /** Effective paragraph indents and spacing (twips); `firstLine` < 0 = hanging. */
  indentLeft?: number;
  indentRight?: number;
  firstLine?: number;
  spaceBefore?: number;
  spaceAfter?: number;
  lineSpacing?: { rule: LineRule; value: number };
  tabs?: TabSpec[];
  shading?: string | null;
  section?: SectionState;
  table?: TableState | null;
  /** Floating picture at the selection (mirrors `bo_edit::picture::PictureState`). */
  picture?: PictureState | null;
  /** A picture the picture tools act on is at the selection (CORE-PIC-001). */
  pictureTools?: boolean;
  /** A SmartArt is at the selection (CORE-SMART-005). */
  smartArt?: boolean;
  /** Proofing language of the selection (§17.3.2.20 lang, BCP 47), null when not set. */
  lang?: string | null;
  /** Spelling and grammar are not checked (§17.3.2.21 noProof). */
  noProof?: boolean;
  /** Advanced character formatting (Font dialog, DOC-FMT-001). */
  advanced?: CharAdvanced;
}

/** Position of a floating picture in points (§20.4.2.10 positionH / §20.4.2.11 positionV). */
export interface PictureState {
  x: number;
  y: number;
  hRelative: string;
  vRelative: string;
}

/** Line spacing rule (§17.18.48): `auto` in 240ths of a line, else twips. */
export type LineRule = 'auto' | 'exact' | 'atLeast';

export type TabKind = 'left' | 'center' | 'right' | 'decimal' | 'bar';

/** Tab stop (twips from the left margin, §17.3.1.37). */
export interface TabSpec {
  pos: number;
  align?: TabKind;
  leader?: 'none' | 'dot' | 'hyphen' | 'underscore';
}

/** Page setup of the caret's section (twips; mirrors `bo_edit::SectionState`). */
export interface SectionState {
  index: number;
  count: number;
  width: number;
  height: number;
  marginTop: number;
  marginBottom: number;
  marginLeft: number;
  marginRight: number;
  header: number;
  footer: number;
  columns: number;
  columnSpace: number;
  titlePage: boolean;
  evenAndOdd: boolean;
  /** §17.11.19 numRestart / §17.11.21 pos of the section's footnotes. */
  footnoteRestart?: NoteRestart;
  footnotePos?: NotePos;
}

/** Page setup sent with `setPageSetup` (twips). */
export type PageSetupSpec = Omit<SectionState, 'index' | 'count' | 'titlePage' | 'evenAndOdd'>;

/** The table holding the caret (mirrors `bo_edit::TableState`). */
export interface TableState {
  rows: number;
  cols: number;
  row: number;
  col: number;
  headerRow: boolean;
  shading: string | null;
  align: Alignment;
  widthPct: number | null;
  width: number;
  multiCell: boolean;
  merged: boolean;
  /** Text direction of the caret's cell (§17.4.72); null = horizontal. */
  direction: TextDirection | null;
  /** Text wraps around the table (§17.4.58 tblpPr). */
  wrap: boolean;
  /** Fixed column widths (§17.4.53 tblLayout). */
  fixedLayout: boolean;
}

/** §17.18.93 ST_TextDirection (transitional names). */
export type TextDirection = 'lrTb' | 'tbRl' | 'btLr' | 'lrTbV' | 'tbRlV' | 'tbLrV';

/** One level of a table sort (mirrors `bo_edit::table_tools::SortKey`). */
export interface TableSortKey {
  /** Grid column, 0-based. */
  column: number;
  kind: 'text' | 'number' | 'date';
  descending: boolean;
}

export type TableEdit =
  | {
      kind:
        'deleteRow' | 'deleteColumn' | 'deleteCell' | 'deleteTable' | 'mergeCells' | 'splitCell';
    }
  | { kind: 'insertRow'; below: boolean }
  | { kind: 'insertCell'; right: boolean }
  | { kind: 'insertColumn'; right: boolean }
  | { kind: 'shading'; hex: string | null }
  | { kind: 'borders'; preset: 'all' | 'none' | 'outside' | 'inside' }
  | { kind: 'properties'; widthPct?: number | null; width?: number | null; align?: Alignment }
  | { kind: 'headerRow'; on: boolean }
  /** Heights are measured by the engine when omitted. */
  | { kind: 'distributeRows'; heights?: number[] }
  | { kind: 'distributeColumns' }
  /** Content widths are measured by the engine when omitted. */
  | { kind: 'autoFit'; mode: 'contents' | 'window' | 'fixed'; widths?: number[] }
  | { kind: 'sort'; keys: TableSortKey[]; header: boolean }
  | { kind: 'textDirection'; dir: TextDirection | null }
  | { kind: 'wrap'; around: boolean }
  /** Separator: "\t", ",", "\n" (one paragraph per cell) or any text. */
  | { kind: 'toText'; separator: string }
  | { kind: 'fromText'; separator: string; cols?: number | null };

/** §17.18.30 / §17.18.21 note placement. */
export type NotePos = 'pageBottom' | 'beneathText' | 'sectEnd' | 'docEnd';
/** §17.18.74 note numbering restart. */
export type NoteRestart = 'continuous' | 'eachSect' | 'eachPage';

/**
 * Open header/footer or footnote/endnote editor, edited in place on the page
 * (mirrors `bo_engine::story::StoryState`).
 */
export interface StoryState {
  kind: 'header' | 'footer' | 'footnote' | 'endnote';
  /** Id of the note being edited (`footnote` / `endnote`). */
  note?: number;
  section: number;
  variant: 'default' | 'first' | 'even';
  page: number;
  titlePage: boolean;
  evenAndOdd: boolean;
  /** Linked to the previous section's header/footer (§17.10.5). */
  linked: boolean;
  /** The section has a previous section to link to. */
  canLink: boolean;
}

/** A footnote or endnote (mirrors `bo_edit::section::NoteInfo`). */
export interface NoteInfo {
  id: number;
  endnote: boolean;
  text: string;
  para: number | null;
}

/** Font picker entry (mirrors `bo_engine::fonts::FontChoice`). */
export interface FontChoice {
  name: string;
  /** Loaded family that displays it (metric-compatible substitute). */
  substitute: string | null;
  available: boolean;
}

/** Style gallery entry (mirrors `bo_engine::gallery::StyleEntry`, ECMA-376 Part 1, §17.7.4.17). */
export interface DocStyleEntry {
  /** `w:styleId`, passed to `applyStyle`. */
  id: string;
  name: string;
  kind: 'paragraph' | 'character';
  /** §17.7.4.14 qFormat. */
  quick: boolean;
  /** §17.7.4.19 uiPriority. */
  uiPriority?: number;
  /** Not hidden / semi-hidden (§17.7.4.16, §17.7.4.20 unhideWhenUsed once used). */
  recommended?: boolean;
  /** The content uses it. */
  inUse?: boolean;
  /** Linked character style shown with this paragraph style (§17.7.4.6 link). */
  linked?: string;
  basedOn?: string;
  /** Default of its type (cannot be deleted). */
  isDefault?: boolean;
  preview: {
    font: string;
    /** Points. */
    size: number;
    bold: boolean;
    italic: boolean;
    underline: boolean;
    /** `RRGGBB`. */
    color: string;
  };
}

/** Paste modes of BlueDoc: `link` pastes a URL / e-mail address as a hyperlink. */
export type DocPasteMode = 'keep' | 'merge' | 'text' | 'link';

export interface FindOptions {
  matchCase?: boolean;
  wholeWord?: boolean;
  regex?: boolean;
  /** Wildcard pattern (`?`, `*`, `[a-z]`, `[!x]`, `{n,m}`, `@`, `<`, `>`, `( )`; `\1` in the replacement). */
  wildcards?: boolean;
}

/** A match: paragraph `p`, byte offsets; `ep` is the end paragraph of a match spanning paragraph marks. */
export interface FindMatch {
  p: number;
  start: number;
  end: number;
  ep?: number;
}

export type CharFormat =
  | { kind: 'bold' | 'italic' | 'underline' | 'strike' | 'superscript' | 'subscript' | 'clear' }
  | { kind: 'font'; name: string }
  | { kind: 'size'; halfPoints: number }
  | { kind: 'color'; hex: string | null }
  | { kind: 'highlight'; hex: string | null }
  /** Proofing language (§17.3.2.20 lang); null removes the direct language. */
  | { kind: 'language'; lang: string | null }
  /** Do not check spelling or grammar (§17.3.2.21 noProof). */
  | { kind: 'noProof'; on: boolean }
  /** Font dialog: effects, spacing, position, scale, border, typography (DOC-FMT-001). */
  | { kind: 'advanced'; patch: CharAdvancedPatch };

export type ParaFormat =
  | { kind: 'align'; align: Alignment }
  | { kind: 'increaseIndent' | 'decreaseIndent' | 'bullets' | 'numbering' | 'clear' }
  | { kind: 'lineSpacing'; value: number }
  | { kind: 'lineSpacingRule'; rule: LineRule; value: number }
  | {
      kind: 'spaceBefore' | 'spaceAfter' | 'indentLeft' | 'indentRight' | 'indentFirstLine';
      twips: number;
    }
  | { kind: 'indents'; left: number; right: number; firstLine: number }
  | { kind: 'tabs'; tabs: TabSpec[] }
  | { kind: 'shading'; hex: string | null }
  /** §17.3.1.35 suppressLineNumbers / §17.3.1.34 suppressAutoHyphens. */
  | { kind: 'suppressLineNumbers' | 'suppressAutoHyphens'; on: boolean }
  | { kind: 'style'; id: string };

export type Motion =
  | 'left'
  | 'right'
  | 'wordLeft'
  | 'wordRight'
  | 'up'
  | 'down'
  | 'lineStart'
  | 'lineEnd'
  | 'pageUp'
  | 'pageDown'
  | 'docStart'
  | 'docEnd';

/** How tracked changes are displayed (mirrors `bo_layout::RevisionView`). */
export type RevisionView = 'markup' | 'final' | 'original';

/** Review display mode (mirrors `bo_engine::markup::MarkupMode`). */
export type MarkupMode = 'simple' | 'all' | 'none' | 'original';

/** Review display settings (mirrors `bo_engine::markup::MarkupOptions`). */
export interface MarkupOptions {
  mode: MarkupMode;
  insertionsDeletions: boolean;
  formatting: boolean;
  comments: boolean;
  hiddenReviewers: string[];
  /** All markup: deletions and formatting changes in balloons instead of inline. */
  deletionBalloons: boolean;
}

export type RevisionKind =
  | 'insert'
  | 'delete'
  | 'moveTo'
  | 'moveFrom'
  | 'format'
  | 'paragraphFormat'
  | 'paragraphInsert'
  | 'paragraphDelete'
  | 'rowInsert'
  | 'rowDelete'
  | 'cellFormat'
  | 'tableFormat'
  | 'sectionFormat'
  | 'cellInsert'
  | 'cellDelete'
  | 'cellMerge'
  | 'rowFormat'
  | 'tableGrid';

/** Header/footer or note holding a revision (mirrors `bo_edit::StoryRef`). */
export type StoryRef =
  | { kind: 'header' | 'footer'; section: number; variant: 'default' | 'first' | 'even' }
  | { kind: 'footnote' | 'endnote'; id: number };

/** A tracked change (mirrors `bo_edit::RevisionItem`); positions are in the story when `story` is set. */
export interface RevisionItem {
  id: number;
  kind: RevisionKind;
  author: string;
  date: string | null;
  start: Pos;
  end: Pos;
  text: string;
  story?: StoryRef;
  /** Formatting changes: keys of the changed properties (`bold`, `font`, `align`, …). */
  detail?: string[];
}

/** A comment (mirrors `bo_edit::CommentInfo`); threads are rebuilt from `parent`. */
export interface CommentInfo {
  id: number;
  author: string;
  initials: string | null;
  date: string | null;
  text: string;
  done: boolean;
  parent: number | null;
  start: Pos | null;
  end: Pos | null;
  quote: string;
  /** @mentions in `text` (CORE-COMMENT-001). */
  mentions?: import('./people').Mention[];
  /** Task assigned in the comment (complete when resolved). */
  task?: import('./people').CommentTask | null;
}

/** An index entry to mark (mirrors `bo_edit::IndexMark`, §17.16.5.72 XE). */
export interface IndexMark {
  main: string;
  sub?: string;
  /** Cross-reference text instead of a page number. */
  crossRef?: string;
  /** Page range of this bookmark. */
  bookmark?: string;
  bold?: boolean;
  italic?: boolean;
}

/** A citation to mark (mirrors `bo_edit::CitationMark`, §17.16.5.62 TA). */
export interface CitationMark {
  long: string;
  short?: string;
  /** 1–16. */
  category: number;
}

/** A cross-reference target (mirrors `bo_edit::RefTarget`). */
export interface RefTarget {
  /** Bookmark name or `caption:<paragraph id>`. */
  id: string;
  label: string;
}

/** Mail-merge state (mirrors `bo_engine::merge::MergeState`). */
export interface MergeState {
  name: string;
  fields: string[];
  count: number;
  /** Previewed record (0-based position in the recipient list) or null when field names are shown. */
  preview: number | null;
  /** Records in the recipient list after filter and exclusions. */
  active?: number;
  /** Filter, sort and exclusions of the recipient list. */
  options?: import('./mailMerge').RecipientOptions;
}

/** Review pane state (mirrors `bo_engine::review::ReviewState`). */
export interface ReviewState {
  trackChanges: boolean;
  /** Tracking is locked (§17.15.1.29 `w:edit="trackedChanges"`); absent from older engines. */
  trackingLocked?: boolean;
  /** Unlocking tracking needs a password. */
  lockPassword?: boolean;
  author: string;
  view: RevisionView;
  /** Display mode and markup options (absent from older engines). */
  markup?: MarkupOptions;
  revisions: RevisionItem[];
  comments: CommentInfo[];
}

/** Number format of a list level (ECMA-376 Part 1, §17.18.59 `ST_NumberFormat` subset). */
export type ListNumFormat =
  | 'decimal'
  | 'decimalZero'
  | 'lowerLetter'
  | 'upperLetter'
  | 'lowerRoman'
  | 'upperRoman'
  | 'bullet'
  | 'none';

/** One level of a new multilevel list (mirrors `bo_edit::ListLevelSpec`; §17.9.6 lvl). */
export interface ListLevelSpec {
  format: ListNumFormat;
  /** Level text (§17.9.11 lvlText): `%1`…`%9` stand for the levels' numbers. */
  text: string;
  /** Start value (§17.9.25 start). */
  start: number;
  /** Left indent of the text, twips. */
  indent: number;
  /** Hanging indent of the number, twips. */
  hanging: number;
  /** Paragraph style linked to the level (§17.9.23 pStyle), e.g. `Heading1`. */
  style?: string | null;
}

/** Set numbering value (mirrors `bo_edit::ListNumbering`). */
export type ListNumbering = { kind: 'restart'; value: number } | { kind: 'continue' };

/** Editing command sent to the engine (JSON bridge, ADR-0008). */
export type EditCommand =
  | import('./security').SecurityCommand
  | import('./picture').PictureCommand
  | import('./customXml').CustomXmlCommand
  /** Embedded / linked workbooks (CORE-EMBED-001). */
  | import('./ole').OleCommand
  | import('./speech').DocSpeechCommand
  | import('./translation').DocTranslateCommand
  | import('./docStyles').DocStyleCommand
  | DocSmartArtCommand
  | { cmd: 'state' | 'selectAll' | 'deleteBackward' | 'deleteForward' | 'splitParagraph' }
  | { cmd: 'undo' | 'redo' | 'insertPageBreak' | 'copy' | 'cut' }
  /** Undo history length → `count`; join the steps after `since` into one (BlueScript, CORE-SCRIPT-001). */
  | { cmd: 'historyLen' }
  | { cmd: 'mergeHistory'; since: number }
  | { cmd: 'setSelection'; selection: Selection }
  | { cmd: 'click'; page: number; x: number; y: number; extend?: boolean; count?: number }
  | { cmd: 'dragTo'; page: number; x: number; y: number }
  | { cmd: 'move'; motion: Motion; extend?: boolean }
  | { cmd: 'insertText'; text: string }
  | { cmd: 'formatText'; format: CharFormat }
  | { cmd: 'formatParagraph'; format: ParaFormat }
  | { cmd: 'applyStyle'; id: string }
  | { cmd: 'defineList'; levels: ListLevelSpec[]; apply?: boolean }
  | { cmd: 'setListNumbering'; numbering: ListNumbering }
  | { cmd: 'insertTable'; rows: number; cols: number }
  | { cmd: 'insertSectionBreak'; kind: 'nextPage' | 'continuous' | 'evenPage' | 'oddPage' }
  | { cmd: 'find'; query: string; options?: FindOptions; highlight?: boolean }
  | { cmd: 'selectMatch'; match: FindMatch }
  | { cmd: 'replace'; match: FindMatch; with: string; query?: string; options?: FindOptions }
  | { cmd: 'replaceAll'; query: string; options?: FindOptions; with: string }
  | { cmd: 'paste'; internal?: string; html?: string; text?: string; mode?: DocPasteMode }
  | { cmd: 'tab'; back?: boolean }
  | { cmd: 'tableEdit'; edit: TableEdit }
  /** Move the floating picture at the selection (points from its position references). */
  | { cmd: 'setPicturePosition'; x: number; y: number }
  | { cmd: 'setPageSetup'; setup: PageSetupSpec; scope?: 'section' | 'document' }
  | { cmd: 'setHeaderOptions'; titlePage?: boolean; evenAndOdd?: boolean }
  | { cmd: 'setHeaderLink'; linked: boolean }
  | { cmd: 'editNote'; endnote?: boolean; id: number }
  | {
      cmd: 'setNoteOptions';
      endnote?: boolean;
      options: {
        pos?: NotePos;
        fmt?: string;
        start?: number;
        restart?: NoteRestart;
      };
      scope?: 'section' | 'document';
    }
  | { cmd: 'editHeaderFooter'; page: number; header: boolean }
  /** Spelling as you type (CORE-SPELL-004): marks, word at a point, ignore / add, on-off. */
  | { cmd: 'spelling'; pages?: number[] }
  | { cmd: 'spellAt'; page: number; x: number; y: number }
  | { cmd: 'spellIgnore' | 'spellAddWord'; word: string }
  | { cmd: 'setSpelling'; on: boolean }
  /** Whole-document spelling and grammar check (CORE-SPELL-005): report, corrections, options. */
  | { cmd: 'proofCheck' }
  | { cmd: 'proofEdit'; edits: ProofEdit[] }
  | { cmd: 'proofOptions'; ignoreCaps?: boolean; ignoreDigits?: boolean; autoLanguage?: boolean }
  /** AutoCorrect as you type (CORE-AUTOCORR-001): set (`options`), restore defaults (`reset`) or query. */
  | { cmd: 'autoCorrect'; options?: AutoCorrectOptions; reset?: boolean }
  /** Equations (CORE-MATH-007): insert from UnicodeMath / LaTeX, query, linear ↔ built-up. */
  | {
      cmd: 'insertEquation';
      text: string;
      latex?: boolean;
      display?: boolean;
      replace?: boolean;
      /** A numbered equation paragraph with this SEQ label. */
      numbered?: string;
    }
  /** Phonetic guide (§17.3.3.25 ruby); `replace` replaces the selected one. */
  | { cmd: 'insertRuby'; ruby: RubySpec; replace?: boolean }
  | { cmd: 'equationTemplates' }
  | { cmd: 'equation' }
  | { cmd: 'equationFormat'; linear: boolean }
  /** Grammar and style as you type (CORE-GRAM-001). */
  | { cmd: 'setGrammar'; on: boolean }
  | { cmd: 'grammarIgnore'; rule: string; text: string }
  | { cmd: 'closeHeaderFooter' | 'notes' | 'clearFormatting' | 'fontList' | 'styleList' }
  | { cmd: 'pickFormat' | 'applyFormat' | 'cancelFormat' }
  | { cmd: 'insertField'; instr: string }
  | { cmd: 'insertNote'; endnote?: boolean; text?: string }
  | { cmd: 'setNoteText'; endnote?: boolean; id: number; text: string }
  | { cmd: 'newFromTemplate'; id: string; name: string }
  | { cmd: 'checkAdminFormat' }
  /** Correct the fixable Nghị định 30/2020 deviations (issue ids; all when omitted), one undoable edit. */
  | { cmd: 'fixAdminFormat'; ids?: string[] }
  | { cmd: 'review' | 'acceptAllRevisions' | 'rejectAllRevisions' }
  /** Compare with an original (.docx / .odt / .doc): the differences become tracked changes. */
  | { cmd: 'compareWith'; name: string; base64: string }
  /** Combine with another reviewer's copy: both reviewers' changes as tracked changes. */
  | { cmd: 'combineWith'; name: string; base64: string }
  | { cmd: 'setTrackChanges'; on: boolean }
  /** TCVN3 / VNI text → Unicode (legacy Vietnamese fonts); one undo step. */
  | { cmd: 'convertLegacyText' }
  | { cmd: 'setAuthor'; name: string }
  | { cmd: 'setRevisionView'; view: RevisionView }
  /** Review display mode / markup options; absent fields are kept. */
  | ({ cmd: 'setMarkup' } & Partial<MarkupOptions>)
  | { cmd: 'acceptRevision' | 'rejectRevision'; id?: number }
  /** Move the selected paragraphs up/down (a tracked move while tracking, §17.13.5.22/25). */
  | { cmd: 'moveParagraphs'; up: boolean }
  | { cmd: 'nextRevision'; forward: boolean }
  | {
      cmd: 'addComment';
      text: string;
      mentions?: import('./people').Mention[];
      task?: import('./people').CommentTask;
    }
  | {
      cmd: 'replyComment';
      id: number;
      text: string;
      mentions?: import('./people').Mention[];
      task?: import('./people').CommentTask;
    }
  | { cmd: 'resolveComment'; id: number; done: boolean }
  | { cmd: 'deleteComment' | 'selectComment'; id: number }
  | { cmd: 'nextComment'; forward: boolean }
  | { cmd: 'updateFields' | 'insertToc' | 'crossReferenceTargets' }
  /** Lock / unlock the fields in the selection (§17.16.18 fldLock). */
  | { cmd: 'lockFields'; lock: boolean }
  /** Fields in the selection become their results as plain text. */
  | { cmd: 'unlinkFields' }
  /** Field codes instead of results (Alt+F9). */
  | { cmd: 'showFieldCodes'; on: boolean }
  /** Insert an inline content control (§17.5.2 sdt) at the caret. */
  | { cmd: 'insertControl'; kind: ControlKind; placeholder?: string; items?: string[] }
  /** Fill the control around the caret: toggle a checkbox, pick an item, set a date (ISO) or text. */
  | { cmd: 'setControlValue'; value?: string }
  /** Insert a legacy form field (§17.16.17 ffData) at the caret. */
  | { cmd: 'insertFormField'; kind: FormFieldKind }
  /** Change the properties of the form field at the caret (its kind stays). */
  | { cmd: 'setFormField'; field: FormFieldData }
  /** Fill in the form field at the caret: text, a check box (`''` toggles, `'1'` / `'0'`), an entry. */
  | { cmd: 'setFormFieldValue'; value?: string }
  /** Select the next (`back`: previous) form field that can be filled in. */
  | { cmd: 'nextFormField'; back?: boolean }
  /** Restrict editing (§17.15.1.29 documentProtection); `edit` defaults to `forms`. */
  | {
      cmd: 'protectDocument';
      edit?: string;
      password?: string | null;
      /** Sections left editable while filling in forms (§17.6.6 formProt). */
      unprotectedSections?: number[];
    }
  /** Stop the editing restriction (the password must match). */
  | { cmd: 'unprotectDocument'; password?: string | null }
  | { cmd: 'insertCaption'; label: string; text: string }
  /** Insert a table of figures (`TOC \c`), an index (`INDEX`) or a table of authorities (`TOA`). */
  | {
      cmd: 'insertReferenceTable';
      instr: string;
      leader?: 'dot' | 'hyphen' | 'underscore' | 'none';
      /** More than one: the table gets a continuous section with that many columns. */
      columns?: number;
    }
  /** Mark the selection as an index entry (XE); `all`: every paragraph's first occurrence too. */
  | { cmd: 'markIndexEntry'; mark: IndexMark; all?: boolean }
  /** Mark the selection as a citation (TA); `all`: later mentions too. */
  | { cmd: 'markCitation'; mark: CitationMark; all?: boolean }
  /** Convert footnotes ↔ endnotes (all, or those referenced in the selection). */
  | { cmd: 'convertNotes'; mode: 'toEndnotes' | 'toFootnotes' | 'swap'; selection?: boolean }
  | { cmd: 'insertCrossReference'; target: string; page: boolean }
  | { cmd: 'mergeData'; name: string; base64: string }
  | { cmd: 'mergeInsertField'; name: string }
  | { cmd: 'mergePreview'; record: number | null }
  | { cmd: 'mergeToDocument' }
  /** Extended mail merge (recipients, rules, prompts, outputs, envelopes, labels). */
  | import('./mailMerge').MailMergeCommand
  | { cmd: 'setSavePassword'; password?: string }
  /** Package variant of the next saves (`docx`, `docm`, `dotx`, `dotm`); error `saveFormat`. */
  | { cmd: 'saveFormat'; ext: string }
  /** Theme of a `.thmx` (or another document / workbook / deck), base64; error `noTheme`. */
  | { cmd: 'applyThemeFile'; file: string }
  /** The document's theme as a `.thmx` → `design.themeFile` (base64); error `noTheme`. */
  | { cmd: 'saveThemeFile' }
  /** Design tab state (page colour, borders, watermark, themes, style sets). */
  | { cmd: 'design' }
  /** Page colour (§17.2.1 background), `RRGGBB`; absent removes it. */
  | { cmd: 'setPageColor'; color?: string | null }
  /** Page borders (§17.6.10 pgBorders); absent removes them. */
  | { cmd: 'setPageBorders'; borders?: PageBordersSpec | null; scope?: 'section' | 'document' }
  /** Text or picture watermark of every page; absent removes it. */
  | { cmd: 'setWatermark'; watermark?: WatermarkSpec | null }
  | { cmd: 'applyTheme' | 'applyStyleSet'; id: string }
  /** File › Info document properties (CORE-UI-002) → `docProperties`. */
  | import('./docProperties').DocPropertiesCommand
  /** Check document (CORE-INSPECT-001): `inspect`, `inspectRemove`, `inspectKeep` → `inspection`. */
  | InspectCommand
  | { cmd: 'inspectFix'; fix: InspectFix }
  /** View layouts, formatting marks, outline, navigation-pane results. */
  | ViewCommand
  /** Layout tab state (columns, line numbers, hyphenation, text direction, drop cap). */
  | { cmd: 'pageLayout' }
  /** Text columns (§17.6.4 cols); `forward` starts a continuous section at the caret. */
  | { cmd: 'setColumns'; columns: ColumnsSpec; scope?: 'section' | 'document' | 'forward' }
  /** Line numbering (§17.6.8 lnNumType); null turns it off. */
  | {
      cmd: 'setLineNumbering';
      numbering: LineNumbering | null;
      scope?: 'section' | 'document';
    }
  /** Text direction of sections (§17.6.20); null = horizontal. */
  | {
      cmd: 'setSectionTextDirection';
      dir: 'tbRl' | 'btLr' | null;
      scope?: 'section' | 'document';
    }
  /** Hyphenation settings (§17.15.1.10 autoHyphenation, …). */
  | { cmd: 'setHyphenation'; hyphenation: HyphenationSettings }
  /** Drop cap of the caret's paragraph (§17.3.1.11 framePr); null removes it. */
  | { cmd: 'setDropCap'; dropCap: DropCapSpec | null }
  /** Signature lines (CORE-SIGLINE-001). */
  | SignatureLineCommand;

/** One column of unequal columns (twips). */
export interface ColumnWidth {
  width: number;
  /** Space after the column. */
  space: number;
}

/** Text columns (mirrors `bo_edit::page_layout::ColumnsSpec`). */
export interface ColumnsSpec {
  count: number;
  /** Gap between equal columns (twips). */
  space: number;
  separator: boolean;
  /** One per column for unequal columns; empty = equal. */
  widths: ColumnWidth[];
}

/** §17.6.8 lnNumType (mirrors `bo_model::LineNumbering`). */
export interface LineNumbering {
  countBy: number;
  /** Stored less one: 0 starts at 1. */
  start: number;
  /** Twips; null = automatic. */
  distance: number | null;
  restart: 'newPage' | 'newSection' | 'continuous';
}

/** Hyphenation settings (mirrors `bo_model::Hyphenation`). */
export interface HyphenationSettings {
  auto: boolean;
  /** Twips; null = 0.25 inch. */
  zone: number | null;
  /** Null or 0 = no limit. */
  consecutiveLimit: number | null;
  noCaps: boolean;
}

/** A drop cap (mirrors `bo_edit::page_layout::DropCapSpec`). */
export interface DropCapSpec {
  position: 'drop' | 'margin';
  font?: string | null;
  lines: number;
  /** Distance from the text (twips). */
  distance: number;
}

/** Layout tab state (mirrors `bo_edit::page_layout::PageLayoutState`). */
export interface PageLayoutState {
  columns: ColumnsSpec;
  /** Text width of the caret's section (twips). */
  textWidth: number;
  lineNumbers?: LineNumbering;
  textDirection?: 'tbRl' | 'btLr';
  hyphenation: HyphenationSettings;
  dropCap?: DropCapSpec;
  suppressLineNumbers: boolean;
  suppressAutoHyphens: boolean;
}

/** Pages showing the page borders (§17.18.63 ST_PageBorderDisplay). */
export type PageBorderDisplay = 'allPages' | 'firstPage' | 'notFirstPage';

/** Page borders (mirrors `bo_edit::design::PageBordersSpec`). */
export interface PageBordersSpec {
  /** §17.18.2 ST_Border: a line style (`single`, `double`…) or an art style. */
  style: string;
  /** Eighths of a point for lines, points (1–31) for art borders. */
  size: number;
  /** `RRGGBB`; null = automatic. */
  color?: string | null;
  /** Distance in points (0–31). */
  space?: number;
  /** Top, left, bottom, right. */
  sides?: [boolean, boolean, boolean, boolean];
  offsetFromPage?: boolean;
  display?: PageBorderDisplay;
  behind?: boolean;
  shadow?: boolean;
}

/** A watermark request (mirrors `bo_engine::design::WatermarkSpec`). */
export interface WatermarkSpec {
  text?: string;
  font?: string;
  /** Points; absent = automatic. */
  size?: number | null;
  color?: string | null;
  semitransparent?: boolean;
  diagonal?: boolean;
  bold?: boolean;
  italic?: boolean;
  /** Picture bytes in base64. */
  picture?: string;
  /** Percent; absent = automatic. */
  scale?: number | null;
  washout?: boolean;
}

/** Design tab state (mirrors `bo_engine::design::DesignState`). */
export interface DesignState {
  pageColor: string | null;
  borders: (Required<PageBordersSpec> & { art: boolean }) | null;
  watermark: {
    text?: string;
    font?: string;
    color?: string;
    picture: boolean;
    washout: boolean;
    semitransparent: boolean;
    diagonal: boolean;
  } | null;
  theme: string | null;
  styleSet: string | null;
  themes: { id: string; colors: string[]; major: string; minor: string }[];
  styleSets: string[];
  /** Art border styles drawn with BlueOffice motifs. */
  artStyles: string[];
  /** `saveThemeFile`: the theme as a `.thmx` file, base64. */
  themeFile?: string;
}

/** A command plus the host clock (Unix ms) used to date new revisions and comments. */
export type TimedCommand = EditCommand & { now?: number };

export interface ClipPayload {
  text: string;
  html: string;
  internal: string;
  /** Picture of the copied content as laid out (PNG, base64; paste as picture). */
  png?: string;
}

/**
 * SmartArt in BlueDoc (CORE-SMART-005, `bo_engine::doc_smartart`): insert from the BlueOffice
 * layout library at the selection, read / change the selected one (text pane, layout, look).
 */
export type DocSmartArtCommand =
  | {
      cmd: 'insertSmartArt';
      layout: SlideSmartArtLayout;
      items: SlideSmartArtItem[];
      colors?: SlideSmartArtColors;
      style?: SlideSmartArtStyle;
      /** Points (default: the text width, 9:16). */
      width?: number;
      height?: number;
      /** Float with square wrapping instead of inline. */
      floating?: boolean;
    }
  | {
      cmd: 'setSmartArt';
      layout?: SlideSmartArtLayout;
      items?: SlideSmartArtItem[];
      colors?: SlideSmartArtColors;
      style?: SlideSmartArtStyle;
    }
  | { cmd: 'smartArtInfo' };

/** The SmartArt at the selection of a document (mirrors `bo_engine::doc_smartart::DocSmartArt`). */
export interface DocSmartArtInfo {
  layout: SlideSmartArtLayout;
  items: SlideSmartArtItem[];
  colors: SlideSmartArtColors;
  style: SlideSmartArtStyle;
  /** Frame size in points. */
  width: number;
  height: number;
  floating: boolean;
}

/** Result of an editing command. */
export interface EditResult {
  /** The SmartArt at the selection after a SmartArt command (`null`: none). */
  smartArt?: DocSmartArtInfo | null;
  info: DocInfo;
  selection: Selection;
  caret: CaretRect | null;
  rects: SelRect[];
  /** Pages whose display list changed. */
  dirty: number[];
  format: FormatState;
  modified: boolean;
  matches?: FindMatch[];
  /** Highlight rectangles of every match (`find` with `highlight`). */
  highlights?: SelRect[];
  count?: number;
  clip?: ClipPayload;
  error?: string;
  /** Present after document changes and review commands. */
  review?: ReviewState;
  /** Cross-reference targets (`crossReferenceTargets`). */
  targets?: RefTarget[];
  /** Mail-merge state (after merge commands and `state`). */
  merge?: MergeState;
  /** Extended mail-merge results (records, prompts, files, e-mails, presets). */
  mergeOut?: import('./mailMerge').MergeOut;
  /** While collaborating: the change to share and the selection as UTF-16 offsets. */
  collab?: { change: CollabChange; selection: [number, number] };
  /** Open header/footer editor (the selection is in it). */
  story?: StoryState;
  /** Footnotes and endnotes (`notes` and note commands). */
  notes?: NoteInfo[];
  /** Font picker entries (`fontList`). */
  fonts?: FontChoice[];
  /** Style gallery (`styleList`). */
  styles?: DocStyleEntry[];
  /** The Format Painter holds picked formatting. */
  painter?: boolean;
  /** Check document report (`inspect` and the inspector commands). */
  inspection?: InspectReport;
  /** Document properties (`docProperties`, `setDocProperties`). */
  docProperties?: import('./docProperties').DocProperties;
  /** Misspelt words to underline (`spelling`, `spellIgnore`, `spellAddWord`, `setSpelling`). */
  spelling?: SpellMark[];
  /** The misspelt word at a point (`spellAt`). */
  spell?: SpellInfo;
  /** Whole-document spelling and grammar check (`proofCheck`). */
  proof?: ProofReport;
  /** Proofing options (`proofOptions`). */
  proofOptions?: ProofOptions;
  /** The equation the selection covers or the caret follows. */
  equation?: EquationInfo;
  /** The phonetic guide the selection covers or the caret follows. */
  ruby?: RubySpec;
  /** The content control around the caret. */
  control?: ControlInfo;
  /** The legacy form field at the caret. */
  formField?: FormFieldInfo;
  /** The document's editing restriction, when it has one. */
  protection?: ProtectionInfo;
  /** Equation templates (`equationTemplates`). */
  templates?: { id: string; linear: string }[];
  /** Current AutoCorrect options (`autoCorrect`). */
  autocorrect?: AutoCorrectOptions;
  /** The AutoCorrect rule applied after the typed character (undo reverts only it). */
  corrected?: AutoCorrectRule;
  /** Design tab state (design commands). */
  design?: DesignState;
  /** Protection state (security commands; after edits while regions are shown). */
  security?: import('./security').SecurityInfo;
  /** The picture at the selection (picture tools commands, CORE-PIC-001). */
  picture?: import('./picture').PictureInfo;
  /** Custom XML parts (`customXml*` commands, §15.2.4). */
  customXml?: import('./customXml').CustomXmlPartInfo[];
  /** The embedded / linked object at the selection (`ole*` commands, CORE-EMBED-001). */
  ole?: import('./ole').OleInfo;
  /** Linked files (`oleLinks`). */
  links?: import('./ole').OleLink[];
  /** Document outline (`outline` and outline commands). */
  outline?: OutlineItem[];
  /** Find results with context (`findResults`). */
  results?: FindResult[];
  /** Layout tab state (page layout commands). */
  pageLayout?: PageLayoutState;
  /** Nghị định 30/2020 format check (`checkAdminFormat`). */
  adminFormat?: AdminFormatReport;
  /** Signature lines of the document (signature line commands). */
  signatureLines?: SignatureLineInfo[];
  /** Read-aloud text and rectangles (`speechText`, `speechRects`, CORE-LANG-003). */
  speech?: import('./speech').DocSpeechInfo;
  /** Text segments of the body paragraphs (`translationSegments`, CORE-LANG-001). */
  translation?: import('./translation').DocTextSegment[];
  /** Style management answer (style commands, DOC-STYLE-001). */
  styleMgr?: import('./docStyles').StyleMgrOut;
  /** Pictures for signing a line (`signatureLineImages`). */
  signatureLineImages?: SignatureLineImages;
}

/** One deviation from Nghị định 30/2020 (mirrors `bo_engine::templates::check::FormatIssue`). */
export interface AdminFormatIssue {
  /** `<component>.<property>`, e.g. `quocHieu.size`. */
  id: string;
  component: string;
  property: string;
  severity: 'error' | 'warning';
  /** A number with its unit ("12–13 pt") or a token (`yes`, `center`, `headerRight`, …). */
  expected: string;
  actual: string;
  /** Body paragraph id of the first occurrence. */
  para?: number;
  section?: number;
  /** Start of the paragraph's text. */
  text?: string;
  count: number;
  /** `fixAdminFormat` can correct it (every occurrence). */
  fixable: boolean;
}

/** Result of `checkAdminFormat` / `fixAdminFormat`. */
export interface AdminFormatReport {
  issues: AdminFormatIssue[];
  /** Components recognised. */
  found: string[];
}

/** A replacement-list entry (mirrors `bo_edit::autocorrect::Replacement`). */
export interface AutoCorrectEntry {
  from: string;
  to: string;
}

/** AutoCorrect options (mirrors `bo_edit::AutoCorrectOptions`). */
export interface AutoCorrectOptions {
  enabled: boolean;
  replaceText: boolean;
  capitalizeSentences: boolean;
  capitalizeCells: boolean;
  twoInitialCaps: boolean;
  smartQuotes: boolean;
  dashes: boolean;
  ordinals: boolean;
  fractions: boolean;
  autoLists: boolean;
  autoLinks: boolean;
  entries: AutoCorrectEntry[];
  /** Abbreviations after which no capital is forced. */
  exceptions: string[];
  /** Words that keep two initial capitals. */
  capsExceptions: string[];
}

/** AutoCorrect rules (mirrors `bo_edit::autocorrect::Rule`). */
export type AutoCorrectRule =
  | 'replace'
  | 'capitalize'
  | 'capitalizeCell'
  | 'twoCaps'
  | 'smartQuote'
  | 'dash'
  | 'ordinal'
  | 'fraction'
  | 'list'
  | 'link';

/** Kinds of content control the editor inserts. */
export type ControlKind = 'text' | 'richText' | 'checkbox' | 'dropDown' | 'comboBox' | 'date';

/** A content control (§17.5.2 sdt) as the engine reports it. */
export interface ControlInfo {
  kind: ControlKind | 'picture' | 'repeatingSection' | 'group' | 'other';
  title: string | null;
  tag: string | null;
  /** Display texts of a list control. */
  items: string[];
  checked: boolean | null;
  /** A date control's date (ISO 8601). */
  date: string | null;
  format: string | null;
  /** The content shows the placeholder. */
  placeholder: boolean;
  /** The contents cannot be edited. */
  locked: boolean;
  /** Bound node (§17.5.2.6 dataBinding `w:xpath`). */
  binding?: string;
  /** The bound node exists (`false`: unbound, the cached content stays). */
  bound?: boolean;
}

/** Kind of a legacy form field (`FORMTEXT`, `FORMCHECKBOX`, `FORMDROPDOWN`). */
export type FormFieldKind = 'text' | 'checkBox' | 'dropDown';

/** §17.18 `ST_FFTextType`. */
export type FormTextType =
  'regular' | 'number' | 'date' | 'currentDate' | 'currentTime' | 'calculated';

/** Help / status text of a form field (`autoText`: names an AutoText entry). */
export interface FormHelpText {
  autoText: boolean;
  text: string;
}

interface FormFieldCommon {
  /** Field (and bookmark) name. */
  name?: string | null;
  enabled: boolean;
  /** Fields are recalculated when the user leaves this one. */
  calcOnExit: boolean;
  /** Macros are kept, never run. */
  entryMacro?: string | null;
  exitMacro?: string | null;
  helpText?: FormHelpText | null;
  statusText?: FormHelpText | null;
}

/** The data of a legacy form field (ECMA-376 Part 1, §17.16.17 ffData). */
export type FormFieldData = FormFieldCommon &
  (
    | {
        kind: 'text';
        type: FormTextType;
        /** Default text, or the `=` expression of a calculated field. */
        default: string;
        /** 0 = no limit. */
        maxLength: number;
        /** Case, numeric picture or date picture. */
        format: string;
      }
    | {
        kind: 'checkBox';
        /** Half-points; `null` = the size of the text. */
        size: number | null;
        default: boolean;
        checked: boolean | null;
      }
    | {
        kind: 'dropDown';
        result: number | null;
        default: number | null;
        entries: string[];
      }
  );

/** The form field at the caret. */
export interface FormFieldInfo {
  field: FormFieldData;
  /** Result text, the selected entry or ☒ / ☐. */
  value: string;
  /** Enabled and not computed. */
  fillable: boolean;
}

/** The document's editing restriction (§17.15.1.29). */
export interface ProtectionInfo {
  /** `forms`, `readOnly`, `comments`, `trackedChanges` or `none`. */
  edit: string;
  enforced: boolean;
  hasPassword: boolean;
  /** Sections left editable while filling in forms. */
  unprotectedSections: number[];
  sections: number;
}

/** An equation of the document as the equation editor shows it. */
export interface EquationInfo {
  /** UnicodeMath linear form. */
  linear: string;
  /** Display equation on its own line (m:oMathPara). */
  display: boolean;
  /** Currently shown in the linear form. */
  isLinear: boolean;
}

/** A misspelt word: wavy line on `page` from `x` to `x + w` at `y` (points). */
export interface SpellMark {
  page: number;
  x: number;
  y: number;
  w: number;
  /** A grammar / style issue (blue) rather than a misspelt word (red). */
  grammar?: boolean;
}

/** A misspelt word with corrections (select `start..end` of paragraph `para` to replace it). */
export interface SpellInfo {
  word: string;
  para: number;
  start: number;
  end: number;
  suggestions: string[];
  /** Grammar rule id (`extraSpace`, `capitalize`, `toneStyle` …) when this is a grammar issue. */
  rule?: string;
}

/** A story of a document (mirrors `bo_edit::proofing::Story`). */
export type ProofStory =
  | { kind: 'body' }
  | { kind: 'header' | 'footer'; section: number; variant: 'default' | 'first' | 'even' }
  | { kind: 'footnote' | 'endnote'; id: number }
  | { kind: 'comment'; index: number };

/** One issue of the whole-document check (mirrors `bo_engine::proof::ProofIssue`). */
export interface ProofIssue {
  story: ProofStory;
  /** Position of the story in checking order. */
  order: number;
  /** Paragraph in the story (body paragraph id for the main text). */
  para: number;
  /** Byte offsets in the paragraph index text. */
  start: number;
  end: number;
  word: string;
  grammar?: boolean;
  /** Grammar rule id. */
  rule?: string;
  suggestions: string[];
  /** Language the word was checked in. */
  lang?: string;
  /** Sentence text around the word. */
  before: string;
  after: string;
}

/** Readability statistics of the main text. */
export interface ProofStats {
  words: number;
  characters: number;
  sentences: number;
  paragraphs: number;
  wordsPerSentence: number;
  sentencesPerParagraph: number;
  charactersPerWord: number;
}

/** Result of `proofCheck`. */
export interface ProofReport {
  issues: ProofIssue[];
  /** Languages in the text without a dictionary (not checked). */
  unchecked: string[];
  /** Primary subtags of the languages with a dictionary (`vi`, `en`). */
  languages: string[];
  stats: ProofStats;
}

/** A correction of the check: replace the range and/or set its language. */
export interface ProofEdit {
  story: ProofStory;
  para: number;
  start: number;
  end: number;
  text?: string;
  lang?: string;
}

/** Proofing options. */
export interface ProofOptions {
  ignoreCaps: boolean;
  ignoreDigits: boolean;
  autoLanguage: boolean;
}

// ---------------------------------------------------------------- live collaboration (bo_engine::collab)

/** A change of the shared state (`bo_collab::Change`), exchanged as JSON with the replica. */
export interface CollabChange {
  /** Quill-style deltas over the body text, applied in order. */
  body?: unknown[][];
  slides?: Record<string, string | null>;
  shapes?: Record<string, string | null>;
  blobs?: Record<string, string | null>;
}

/** Geometry of a collaborator's caret / selection. */
export interface PeerRects {
  caret: CaretRect | null;
  rects: SelRect[];
  /** The paragraph the caret is in, on the caret's page (margin bar). */
  para?: CaretRect | null;
}

/** A collaborator shown in the document. */
export interface PeerCursor extends PeerRects {
  clientId: number;
  name: string;
  color: string;
}

/** A collaborator's selected slide object. */
export interface SlidePeer {
  clientId: number;
  name: string;
  color: string;
  slide: number;
  rect: SlideBox;
  /** On the slide without a selected object (`rect` is empty). */
  slideOnly?: boolean;
}

/** A collaborator's selected BlueSheet cell / range (awareness `cell`). */
export interface SheetPeer {
  clientId: number;
  name: string;
  color: string;
  /** Sheet name. */
  sheet: string;
  /** A1 range on that sheet, e.g. `B2:C4` or `B2`. */
  range: string;
}

// ---------------------------------------------------------------- slide editing (mirrors bo_engine::slide_edit)

/** Box in slide points: centre, size, rotation (degrees clockwise). */
export interface SlideBox {
  cx: number;
  cy: number;
  w: number;
  h: number;
  rot: number;
}

export interface SlideRect {
  x: number;
  y: number;
  w: number;
  h: number;
}

export type HandleKind = 'nw' | 'n' | 'ne' | 'e' | 'se' | 's' | 'sw' | 'w' | 'rot';

export interface SlideHandle {
  kind: HandleKind;
  x: number;
  y: number;
}

export interface SlideGuide {
  /** `v`: vertical line at x = pos; `h`: horizontal line at y = pos. */
  axis: 'v' | 'h';
  pos: number;
  from: number;
  to: number;
}

export interface SlideShapeInfo {
  id: number;
  name: string;
  kind:
    | 'shape'
    | 'picture'
    | 'group'
    | 'connector'
    | 'table'
    | 'chart'
    | 'smartart'
    | 'model3d'
    | 'object'
    | 'graphic';
  rect: SlideBox;
  /** Text in the editing format: `\n` paragraphs, `\u000b` line breaks. */
  text: string | null;
  placeholder: string | null;
  lockAspect: boolean;
  editableText: boolean;
}

export type SlideAlign = 'left' | 'center' | 'right' | 'justify';

export interface SlideFormatState {
  hasText: boolean;
  bold: boolean;
  italic: boolean;
  underline: boolean;
  strike: boolean;
  size: number | null;
  color: string | null;
  highlight: string | null;
  font: string | null;
  align: SlideAlign | null;
  bullets: boolean;
  numbering: boolean;
  /** List level, 0-based. */
  level: number;
  /** Line spacing multiple (`null` = single / exact points). */
  lineSpacing: number | null;
}

/** Text being edited inside a shape (offsets in UTF-16 code units). */
export interface SlideTextEdit {
  id: number;
  start: number;
  end: number;
  /** Caret line `[x1, y1, x2, y2]` in slide points (collapsed selection). */
  caret: [number, number, number, number] | null;
  /** Selection highlight boxes, one per line. */
  rects: SlideBox[];
  rect: SlideBox;
  /** Text in the editing format. */
  text: string;
  /** Table cell `[row, col]` being edited (`rect` is the cell's box). */
  cell?: [number, number];
}

export type SlideTransform =
  | { kind: 'move'; dx: number; dy: number }
  | { kind: 'resize'; handle: HandleKind; dx: number; dy: number; keepAspect?: boolean }
  | { kind: 'rotate'; x: number; y: number; step?: boolean };

export type SlideTextFormat =
  | {
      kind: 'bold' | 'italic' | 'underline' | 'strike' | 'bullets' | 'numbering';
      value?: boolean;
    }
  | { kind: 'size' | 'sizeBy'; points: number }
  | { kind: 'color' | 'highlight'; hex: string | null }
  | { kind: 'font'; typeface: string }
  | { kind: 'align'; align: SlideAlign }
  | { kind: 'level'; delta: number }
  | { kind: 'lineSpacing'; value: number };

/** Type of a freeform anchor point (how its control points move together). */
export type SlidePointType = 'corner' | 'smooth' | 'symmetric';

/** Shape merge operations (CORE-SHAPE-001). */
export type SlideMergeOp = 'union' | 'combine' | 'fragment' | 'intersect' | 'subtract';

/** Text fill of WordArt (CORE-SHAPE-002); colours `RRGGBB`, angle in degrees. */
export type SlideTextFill =
  | { type: 'auto' }
  | { type: 'none' }
  | { type: 'solid'; color: string }
  | { type: 'gradient'; from: string; to: string; angle?: number };

/** Shape effect / WordArt edit of the selected shapes (CORE-SHAPE-002). */
export type SlideEffectOp =
  /** Reflection: fade length (fraction, absent removes it) and gap in points. */
  | { kind: 'reflection'; size?: number; distance?: number }
  /** Soft edges in points (absent or 0 removes them). */
  | { kind: 'softEdge'; radius?: number }
  /** 3-D rotation: ST_PresetCameraType (absent removes the scene). */
  | { kind: 'rotation3d'; preset?: string }
  /** Text warp: ST_TextShapeType (absent removes it). */
  | { kind: 'textWarp'; preset?: string }
  /** Text outline colour (absent removes it), width in points. */
  | { kind: 'textOutline'; color?: string; width?: number }
  | { kind: 'textFill'; fill: SlideTextFill }
  /** Warp gallery thumbnails (answer in `warpPreviews`). */
  | { kind: 'warpPreviews' };

/** One edit of a freeform outline; coordinates in slide points. */
export type SlidePointEdit =
  | { kind: 'move'; path: number; point: number; x: number; y: number }
  /** Control point: `side` 0 = incoming, 1 = outgoing. */
  | { kind: 'handle'; path: number; point: number; side: 0 | 1; x: number; y: number }
  | { kind: 'add'; x: number; y: number; tolerance?: number }
  | { kind: 'delete'; path: number; point: number }
  | { kind: 'segment'; path: number; segment: number; curved: boolean }
  | { kind: 'pointType'; path: number; point: number; pointType: SlidePointType };

/** Editable outline of the selected freeform (slide points). */
export interface SlideFreeformInfo {
  id: number;
  paths: {
    points: {
      x: number;
      y: number;
      kind: SlidePointType;
      /** Incoming and outgoing control points (`null` beside straight segments). */
      handles: [[number, number] | null, [number, number] | null];
    }[];
    segments: { curved: boolean; mid: [number, number] }[];
    /** Outline as an SVG path in slide points. */
    d: string;
  }[];
}

/** Slide editing command (JSON bridge, ADR-0008). */
export type SlideCommand =
  | import('./security').SecurityCommand
  /** Playback options of video / audio clips (SLIDE-MEDIA-001). */
  | import('./slideMedia').SlideMediaCommand
  /** Slide masters, layouts, themes, slide size (SLIDE-MASTER-001). */
  | import('./slideMaster').SlideMasterCommand
  | import('./picture').PictureCommand
  /** Embedded / linked workbooks (CORE-EMBED-001). */
  | import('./ole').OleCommand
  /** Outline, notes page, notes / handout master views, sections (SLIDE-VIEW-001). */
  | import('./slideViews').SlideViewCommand
  /** Custom shows, show setup, timings, show links, kept ink (SLIDE-SHOW-001). */
  | import('./slideShow').SlidePresentCommand
  /** Draw tab ink: add, erase, read (CORE-INK-001). */
  | import('./slideInk').SlideInkCommand
  /** Recorded slide shows: save / clear / read (SLIDE-REC-001). */
  | import('./slideShow').SlideRecordCommand
  /** 3D models: view, presets, pan / zoom, alt text (CORE-3DMODEL-001). */
  | import('./slideModel3d').SlideModel3dCommand
  /** Spelling and grammar of the deck (CORE-SPELL-006). */
  | import('./spelling').SlideSpellCommand
  /** Compare with another version of the deck (CORE-VERSION-001) → `DeckCompareResult`. */
  | import('./versions').CompareDeckCommand
  | { cmd: 'state' | 'selectAll' | 'delete' | 'duplicate' | 'group' | 'ungroup' }
  /** File › Info document properties (CORE-UI-002) → `docProperties`. */
  | import('./docProperties').DocPropertiesCommand
  /** Check document (CORE-INSPECT-001) → `inspection`. */
  | InspectCommand
  | { cmd: 'inspectFix'; fix: InspectFix }
  /** Password for the next saves ([MS-OFFCRYPTO]); empty removes it. */
  | { cmd: 'setSavePassword'; password?: string }
  /** Package variant of the next saves (`pptx`, `pptm`, `potx`, `potm`, `ppsx`, `ppsm`). */
  | { cmd: 'saveFormat'; ext: string }
  /** The first master's theme as a `.thmx` → `design.themeFile` (base64). */
  | { cmd: 'saveThemeFile' }
  | { cmd: 'undo' | 'redo' }
  /** Undo history length → `history`; join the steps after `since` into one (BlueScript, CORE-SCRIPT-001). */
  | { cmd: 'historyLen' }
  | { cmd: 'mergeHistory'; since: number }
  /** New comment thread on the current slide, or a reply to thread `parent`. */
  | {
      cmd: 'addComment';
      text: string;
      author?: string;
      date?: string;
      parent?: string;
      mentions?: import('./people').Mention[];
      task?: import('./people').CommentTask;
    }
  | { cmd: 'resolveComment'; id: string; done: boolean }
  | { cmd: 'deleteComment'; id: string }
  | { cmd: 'goTo'; slide: number }
  | { cmd: 'select'; ids: number[] }
  | { cmd: 'selectRect'; rect: SlideRect; extend?: boolean }
  | { cmd: 'selectNext'; back?: boolean }
  | { cmd: 'click'; x: number; y: number; extend?: boolean; deep?: boolean }
  | { cmd: 'hitTest'; x: number; y: number; deep?: boolean }
  | {
      cmd: 'transform';
      transform: SlideTransform;
      snap?: boolean;
      tolerance?: number;
      preview?: boolean;
    }
  | { cmd: 'zOrder'; to: 'front' | 'back' | 'forward' | 'backward' }
  | { cmd: 'align'; to: 'left' | 'center' | 'right' | 'top' | 'middle' | 'bottom' }
  | { cmd: 'distribute'; axis: 'horizontal' | 'vertical' }
  | { cmd: 'insertShape'; preset: string; rect?: SlideRect }
  | { cmd: 'insertTextBox'; text?: string; rect?: SlideRect }
  | { cmd: 'insertTable'; rows: number; cols: number; rect?: SlideRect }
  | {
      cmd: 'setText';
      id?: number;
      text: string;
      merge?: boolean;
      sel?: [number, number];
      /** Text of table cell `[row, col]` of table `id`. */
      cell?: [number, number];
    }
  | { cmd: 'formatText'; format: SlideTextFormat; range?: [number, number] }
  | { cmd: 'textSelect'; id?: number; start: number; end: number; cell?: [number, number] }
  | { cmd: 'textHit'; x: number; y: number; extend?: boolean }
  | { cmd: 'cellTextHit'; x: number; y: number }
  | { cmd: 'cellNext'; back?: boolean }
  | { cmd: 'textExit' | 'copy' | 'cut' }
  /** Paste the internal clipboard; `keepSource`: shapes keep the source slide's theme colours and fonts. */
  | { cmd: 'paste'; keepSource?: boolean }
  | { cmd: 'copySlides' | 'cutSlides'; indices?: number[] }
  | { cmd: 'adjust'; index: number; x: number; y: number; merge?: boolean }
  | {
      cmd: 'connectorEnd';
      end: 0 | 1;
      x: number;
      y: number;
      merge?: boolean;
      tolerance?: number;
    }
  | { cmd: 'insertConnector'; preset?: string }
  /** Edit points: the selected shape's outline becomes a freeform (custGeom). */
  | { cmd: 'convertToFreeform' }
  /** Edit a point or segment of the selected freeform (`merge`: same drag). */
  | { cmd: 'editPoint'; edit: SlidePointEdit; merge?: boolean }
  /** Merge the selected shapes (selection order: the first gives formatting). */
  | { cmd: 'mergeShapes'; op: SlideMergeOp }
  /** Shape effects and WordArt of the selected shapes. */
  | { cmd: 'shapeEffects'; op: SlideEffectOp }
  | { cmd: 'tableSelect'; id?: number; r0: number; c0: number; r1?: number; c1?: number }
  | { cmd: 'tableHit'; x: number; y: number; extend?: boolean }
  | { cmd: 'setCellText'; row?: number; col?: number; text: string; merge?: boolean }
  | { cmd: 'tableInsertRow'; below?: boolean }
  | { cmd: 'tableInsertCol'; right?: boolean }
  | {
      cmd: 'tableDeleteRows' | 'tableDeleteCols' | 'tableMerge' | 'tableSplit' | 'tableExit';
    }
  | { cmd: 'tableFill'; color: string | null }
  | { cmd: 'tableOptions'; firstRow?: boolean; bandRow?: boolean }
  | { cmd: 'setFill'; color: string | null }
  | { cmd: 'setLine'; color: string | null; width?: number }
  | { cmd: 'addSlide'; after?: number; layout?: number }
  | { cmd: 'deleteSlide' | 'duplicateSlide'; index?: number }
  | { cmd: 'moveSlide'; from: number; to: number }
  | { cmd: 'setNotes'; slide?: number; text: string }
  | { cmd: 'setTransition'; slides?: number[]; all?: boolean; transition: SlideTransitionSpec }
  | ({ cmd: 'addAnimation' } & SlideAnimSpec)
  | ({ cmd: 'setAnimation'; index: number } & SlideAnimSpec)
  | { cmd: 'removeAnimation'; index: number }
  | { cmd: 'moveAnimation'; from: number; to: number }
  /** Animation painter: the selected shapes get the effects of shape `from`. */
  | { cmd: 'copyAnimations'; from: number }
  | { cmd: 'show'; slide: number }
  | {
      cmd: 'insertChart';
      kind: SlideChartKind;
      title?: string;
      data?: SlideChartData;
      legend?: SlideLegend;
      rect?: SlideRect;
      /** Chart template to format the new chart with (CORE-CHART-007). */
      template?: import('./chartDesign').ChartTemplate;
      texts?: import('./chartDesign').ChartLayoutTexts;
    }
  /** Chart Design gallery of the selected chart (answer in `chartGallery`). */
  | { cmd: 'chartGallery'; id?: number; width?: number; height?: number }
  /** SmartArt (CORE-SMART-003/004): insert from the layout library, edit outline/layout, to shapes. */
  | {
      cmd: 'insertSmartArt';
      layout: SlideSmartArtLayout;
      items: SlideSmartArtItem[];
      colors?: SlideSmartArtColors;
      style?: SlideSmartArtStyle;
      rect?: SlideRect;
    }
  | {
      cmd: 'setSmartArt';
      id?: number;
      layout?: SlideSmartArtLayout;
      items?: SlideSmartArtItem[];
      colors?: SlideSmartArtColors;
      style?: SlideSmartArtStyle;
    }
  | { cmd: 'smartArtToShapes'; id?: number }
  | ({
      cmd: 'setChart';
      id?: number;
      kind?: SlideChartKind;
      /** Empty = no title. */
      title?: string;
      /** `none` = no legend. */
      legend?: SlideLegend;
      data?: SlideChartData;
    } & import('./chartDesign').ChartDesignSpec);

/** Slide transition effects (ECMA-376 Part 1, §19.3.1.50 transition children). */
/**
 * Slide transitions: ECMA-376 Part 1, §19.3.1.50 transition children, [MS-PPTX] p14 effects and the
 * p15:prstTrans presets (ids of `bo_pptx::transitions::CATALOG`).
 */
export type SlideTransitionKind =
  | 'none'
  | 'cut'
  | 'fade'
  | 'push'
  | 'wipe'
  | 'split'
  | 'cover'
  | 'pull'
  | 'random'
  | 'randomBar'
  | 'blinds'
  | 'checker'
  | 'comb'
  | 'circle'
  | 'diamond'
  | 'plus'
  | 'wedge'
  | 'dissolve'
  | 'newsflash'
  | 'strips'
  | 'wheel'
  | 'zoom'
  | 'reveal'
  | 'flash'
  | 'vortex'
  | 'switch'
  | 'flip'
  | 'ripple'
  | 'honeycomb'
  | 'glitter'
  | 'shred'
  | 'gallery'
  | 'cube'
  | 'box'
  | 'rotate'
  | 'orbit'
  | 'doors'
  | 'window'
  | 'ferris'
  | 'conveyor'
  | 'pan'
  | 'warp'
  | 'flythrough'
  | 'wheelReverse'
  | 'fallOver'
  | 'drape'
  | 'curtains'
  | 'wind'
  | 'prestige'
  | 'fracture'
  | 'crush'
  | 'peelOff'
  | 'pageCurlDouble'
  | 'pageCurlSingle'
  | 'airplane'
  | 'origami'
  | 'morph';

export interface SlideTransitionSpec {
  /** `keep` keeps the effect and changes the timing only. */
  kind: SlideTransitionKind | 'keep';
  /** Variant of the effect (see `SlideTransitionInfo.dirs`): `l`/`u`/`r`/`d`, corners, `horz`/`vert`, `horzIn`, `black`, spokes, … */
  dir?: string;
  durMs?: number;
  advanceOnClick?: boolean;
  /** 0 = no automatic advance. */
  advanceAfterMs?: number;
  /** `none` removes the sound, `stop` stops the sound playing (§19.3.1.42 sndAc). */
  sound?: 'none' | 'stop';
  /** A new sound: WAV bytes in base64. */
  soundData?: string;
  soundName?: string;
  /** Play the sound until the next one. */
  soundLoop?: boolean;
}

/** Sound of a transition. */
export interface SlideTransitionSound {
  /** Stops the sound playing instead of starting one. */
  stop: boolean;
  /** Embedded WAV part (`EngineHost.getMedia`). */
  part: string | null;
  name: string;
  loop: boolean;
}

export interface SlideTransitionInfo {
  /** Modelled kind, or the element name of an effect BlueSlide does not model. */
  kind: string;
  known: boolean;
  dir: string | null;
  /** Variants of the effect (first: the default). */
  dirs?: string[];
  sound?: SlideTransitionSound | null;
  durMs: number;
  advanceOnClick: boolean;
  advanceAfterMs: number | null;
}

/** Animation effects BlueSlide creates (`bo_pptx::anim_catalog`, ECMA-376 Part 1, §19.5). */
export type SlideAnimEffect =
  | 'appear'
  | 'fade'
  | 'flyIn'
  | 'wipe'
  | 'blinds'
  | 'box'
  | 'checkerboard'
  | 'circle'
  | 'crawl'
  | 'diamond'
  | 'dissolve'
  | 'peek'
  | 'plus'
  | 'randomBars'
  | 'split'
  | 'strips'
  | 'swivel'
  | 'wedge'
  | 'wheel'
  | 'zoom'
  | 'bounce'
  | 'float'
  | 'growTurn'
  | 'spinner'
  | 'expand'
  | 'growShrink'
  | 'spin'
  | 'transparency'
  | 'pulse'
  | 'teeter'
  | 'blink'
  | 'fillColor'
  | 'lineDown'
  | 'lineUp'
  | 'lineLeft'
  | 'lineRight'
  | 'arcDown'
  | 'arcUp'
  | 'turnDown'
  | 'circlePath'
  | 'squarePath'
  | 'diamondPath'
  | 'triangle'
  | 'loop'
  | 'wave';
export type SlideAnimTrigger = 'onClick' | 'withPrevious' | 'afterPrevious';

export interface SlideAnimSpec {
  effect?: SlideAnimEffect;
  /** Entrance effects run backwards as exits; emphasis and motion paths have their own class. */
  class?: 'entr' | 'exit';
  /** Variant of the effect (see `SlideAnimInfo.dirs`). */
  direction?: string;
  trigger?: SlideAnimTrigger;
  durMs?: number;
  delayMs?: number;
  /** Runs: 1 = once, 0 = until the next click. */
  repeat?: number;
  /** Shape whose click starts the effect (0 = the slide's sequence). */
  triggerShape?: number;
}

/** One effect of the main animation sequence (§19.5). */
export interface SlideAnimInfo {
  index: number;
  shapeId: number;
  shapeName: string;
  effect: SlideAnimEffect | 'other';
  class: 'entr' | 'exit' | 'emph' | 'path' | 'other';
  direction: string | null;
  trigger: SlideAnimTrigger;
  delayMs: number;
  durMs: number;
  /** Click step (0 = with the slide) and times relative to it. */
  click: number;
  startMs: number;
  endMs: number;
  /** Bounds `[x, y, w, h]` (points) of the animated top-level shape. */
  rect: [number, number, number, number] | null;
  /** Layer of the slide show holding the shape. */
  layer?: number | null;
  /** Runs of the effect; 0 = until the next click. */
  repeat?: number;
  /** Shape whose click starts the effect (interactive sequence). */
  triggerShape?: number | null;
  /** Bookmark of the trigger shape's media that starts the effect ([MS-PPTX] bmkTgt). */
  triggerBookmark?: string | null;
  /** §19.5 cmd of a media call effect: the show plays the clip, the shape does not move. */
  mediaCmd?: string | null;
  /** Variants the effect offers (first: the default). */
  dirs?: string[];
  /** What the effect does, read from its behaviours (§19.5). */
  play?: SlideEffectPlay;
}

/** One behaviour of an effect on the effect's timeline (`bo_pptx::anim_play::Track`). */
export type SlideEffectTrack = {
  beginMs: number;
  durMs: number;
  repeat: number;
  autoRev: boolean;
  hold: boolean;
} & (
  | { kind: 'set'; attr: string; value: string }
  | { kind: 'anim'; attr: string; keys: [number, number][]; discrete: boolean }
  | { kind: 'filter'; filter: string; entering: boolean }
  | { kind: 'motion'; points: [number, number][] }
  | { kind: 'rotate'; by: number }
  | { kind: 'scale'; from: [number, number]; to: [number, number] }
  | { kind: 'color'; attr: string; to: number | null }
);

export interface SlideEffectPlay {
  durMs: number;
  tracks: SlideEffectTrack[];
}

/** A video or audio clip on a slide. */
export interface SlideMediaInfo {
  id: number;
  name: string;
  kind: 'video' | 'audio';
  rect: [number, number, number, number];
  /** Embedded part (bytes from `EngineHost.getMedia`). */
  part: string | null;
  mime: string | null;
  link: string | null;
  /** How the clip plays (SLIDE-MEDIA-001). */
  playback?: import('./slideMedia').SlideMediaPlayback;
  /** Narration / camera of a recorded show (SLIDE-REC-001). */
  recorded?: 'narration' | 'camera';
}

/** What the slide show needs to play one slide. */
export interface SlideShowInfo {
  slide: number;
  width: number;
  height: number;
  transition: SlideTransitionInfo | null;
  /** Bottom to top; animated shapes have a layer of their own. */
  layers: { ids: number[]; backdrop: boolean; page: PageDisplayList }[];
  animations: SlideAnimInfo[];
  clicks: number;
  /** Shapes that start effects when clicked, with their boxes `[x, y, w, h]`. */
  triggers?: { shapeId: number; rect: [number, number, number, number] }[];
  /** Shapes whose hyperlink the show follows (custom shows, jumps). */
  links?: import('./slideShow').SlideShowLink[];
  media: SlideMediaInfo[];
  /** Morph from the previous slide ([MS-PPTX] p159:morph), when that is the transition. */
  morph?: SlideMorphInfo;
}

/** A shape found on two consecutive slides: boxes `[cx, cy, w, h, rot]` (points, degrees). */
export interface SlideMorphPair {
  from: [number, number, number, number, number];
  to: [number, number, number, number, number];
  fromPage: PageDisplayList;
  toPage: PageDisplayList;
}

/** What the slide show needs to morph into a slide. */
export interface SlideMorphInfo {
  fromBackdrop: PageDisplayList;
  toBackdrop: PageDisplayList;
  pairs: SlideMorphPair[];
  /** Shapes of the previous slide only (they fade out). */
  gone: PageDisplayList[];
  /** Shapes of this slide only (they fade in). */
  new: PageDisplayList[];
  /** `byObject` | `byWord` | `byChar`. */
  option: string;
}

export type SlideChartKind =
  | 'column'
  | 'bar'
  | 'line'
  | 'area'
  | 'pie'
  | 'doughnut'
  | 'stackedColumn'
  | 'percentColumn'
  | 'stackedBar'
  | 'percentBar'
  | 'stackedLine'
  | 'percentLine'
  | 'stackedArea'
  | 'percentArea'
  | 'explodedPie'
  | 'pieOfPie'
  | 'barOfPie'
  | 'radar'
  | 'radarMarker'
  | 'radarFilled'
  | 'stock'
  | 'stockUpDown'
  | 'surface'
  | 'column3D'
  | 'clusteredColumn3D'
  | 'stackedColumn3D'
  | 'percentColumn3D'
  | 'cylinder3D'
  | 'cone3D'
  | 'pyramid3D'
  | 'bar3D'
  | 'stackedBar3D'
  | 'percentBar3D'
  | 'line3D'
  | 'area3D'
  | 'stackedArea3D'
  | 'percentArea3D'
  | 'pie3D'
  | 'explodedPie3D'
  | 'surface3D';
export type SlideLegend = 'none' | 'r' | 'l' | 't' | 'b' | 'tr';

/** Data grid of a chart: categories down, series across. */
export interface SlideChartData {
  categories: string[];
  series: { name: string; values: (number | null)[] }[];
}

/** Layout categories of the BlueOffice SmartArt library. */
export type SlideSmartArtLayout =
  'list' | 'process' | 'cycle' | 'hierarchy' | 'matrix' | 'pyramid' | 'relationship';

/** SmartArt colours and look (BlueOffice palettes). */
export type SlideSmartArtColors = 'colorful' | 'blue' | 'green' | 'orange' | 'purple' | 'gray';
export type SlideSmartArtStyle = 'fill' | 'outline';

/** One line of the SmartArt text pane (level 0 = top). */
export interface SlideSmartArtItem {
  text: string;
  level: number;
}

/** The selected SmartArt. */
export interface SlideSmartArtInfo {
  id: number;
  layout: SlideSmartArtLayout;
  items: SlideSmartArtItem[];
  colors: SlideSmartArtColors;
  style: SlideSmartArtStyle;
}

export interface SlideChartInfo {
  id: number;
  kind: SlideChartKind | 'other';
  title: string | null;
  legend: Exclude<SlideLegend, 'none'> | null;
  data: SlideChartData;
  embedded: boolean;
}

export interface SlidePreview {
  boxes: SlideBox[];
  bounds: SlideBox | null;
  guides: SlideGuide[];
  dx: number;
  dy: number;
  angle: number | null;
}

/** Result of a slide command. */
/** A comment thread of a slide ([MS-PPTX] modern comments). */
export interface SlideThreadInfo {
  id: string;
  done: boolean;
  /** First comment, then the replies. */
  comments: import('./people').ThreadCommentInfo[];
}

export interface SlideEditResult {
  info: DocInfo;
  slide: number;
  /** Top-level shapes of the current slide (what can trigger an effect). */
  objects?: { id: number; name: string }[];
  selection: SlideShapeInfo[];
  bounds: SlideBox | null;
  handles: SlideHandle[];
  placeholders: SlideBox[];
  dirty: number[];
  slidesChanged: boolean;
  modified: boolean;
  canUndo: boolean;
  canRedo: boolean;
  format: SlideFormatState;
  notes: string;
  layouts: { index: number; name: string; kind: string | null }[];
  preview?: SlidePreview;
  hits?: number[];
  /** Undo history length (`historyLen`, `mergeHistory`). */
  history?: number;
  error?: string;
  /** While collaborating: the change to share and the selected object key. */
  collab?: { change: CollabChange; shape?: string; slide?: string };
  /** Comment threads of the current slide. */
  comments?: SlideThreadInfo[];
  /** Check document report (`inspect` and the inspector commands). */
  inspection?: InspectReport;
  /** Document properties (`docProperties`, `setDocProperties`). */
  docProperties?: import('./docProperties').DocProperties;
  /** Text editing inside a shape. */
  textEdit?: SlideTextEdit;
  /** Table being edited: anchor cells (merged cells once) and the selection. */
  table?: {
    id: number;
    rows: number;
    cols: number;
    /** `[r0, c0, r1, c1]`, inclusive. */
    sel: [number, number, number, number];
    cells: {
      row: number;
      col: number;
      rowSpan: number;
      colSpan: number;
      rect: SlideBox;
      text: string;
    }[];
    firstRow: boolean;
    bandRow: boolean;
    /** Text of the first selected cell. */
    text: string;
  };
  /** Adjust handles of the selected preset shape (§20.1.9.5 avLst). */
  adjust: { index: number; x: number; y: number }[];
  /** The selected connector: ends and whether each is attached to a shape. */
  connector?: {
    id: number;
    ends: [[number, number], [number, number]];
    attached: [boolean, boolean];
  };
  /** Connection sites of the other shapes while a connector is selected. */
  sites?: [number, number][];
  /** The selected freeform's anchor points and segments (edit points). */
  freeform?: SlideFreeformInfo;
  /** What the internal clipboard holds. */
  clip?: 'shapes' | 'slides';
  /** After copy / cut: content for the system clipboard (`png` in base64). */
  clipboard?: { text: string; png?: string };
  /** Transition of the current slide. */
  transition?: SlideTransitionInfo | null;
  /** Main animation sequence of the current slide. */
  animations?: SlideAnimInfo[];
  /** Video and audio clips of the current slide. */
  media?: SlideMediaInfo[];
  /** The selected chart. */
  chart?: SlideChartInfo;
  /** Answer of `chartGallery`. */
  chartGallery?: import('./chartDesign').ChartGallery;
  /** Answer of `shapeEffects` / `warpPreviews`: SVG paths in a 120 × 72 box. */
  warpPreviews?: { name: string; d: string }[];
  /** The selected SmartArt. */
  smartart?: SlideSmartArtInfo;
  /** Answer of the `show` command. */
  show?: SlideShowInfo;
  /** Custom shows, show setup, hidden slides and timings (Slide Show tab). */
  presenting?: import('./slideShow').SlidePresentInfo;
  /** Protection state (security commands, CORE-SEC-001). */
  security?: import('./security').SecurityInfo;
  /** Slide size, theme and masters (Design tab). */
  design?: import('./slideMaster').SlideDesignInfo;
  /** The master view's pane, while it is open. */
  masterView?: import('./slideMaster').SlideMasterViewInfo;
  /** The selected picture (picture tools commands, CORE-PIC-001). */
  picture?: import('./picture').PictureInfo;
  /** The selected embedded / linked object (`ole*` commands, CORE-EMBED-001). */
  ole?: import('./ole').OleInfo;
  /** Linked files (`oleLinks`). */
  links?: import('./ole').OleLink[];
  /** Sections of the deck (absent without sections). */
  sections?: import('./slideViews').SlideSection[];
  /** The notes / handout master view's pane, while it is open. */
  pageMaster?: import('./slideViews').SlidePageMasterInfo;
  /** Answer of `outline` / `applyOutline`. */
  outline?: import('./slideViews').SlideOutlineSlide[];
  /** Answer of `notesPages`. */
  notesPages?: import('./slideViews').SlideNotesPage[];
  /** Ink of the current slide (`inkInfo` / `addInk` / `eraseInk`, CORE-INK-001). */
  ink?: import('./slideInk').SlideInkInfo;
  /** Recordings of the deck (answer of the recording commands, SLIDE-REC-001). */
  recording?: { slides: import('./slideShow').SlideRecordingInfo[] };
  /** The selected 3D model (CORE-3DMODEL-001). */
  model3d?: import('./slideModel3d').SlideModel3dInfo;
  /** Answer of `model3dMesh`. */
  model3dMesh?: import('./slideModel3d').SlideModel3dMesh;
}

// ---------------------------------------------------------------- services

/** The core engine. Same API whether it runs as WASM (web) or natively (desktop). */
export interface EngineHost {
  readonly backend: 'wasm' | 'native';
  /** Load fonts and get ready. Idempotent. */
  init(): Promise<void>;
  /**
   * Open a file (`.docx`/`.docm`, `.doc`, `.odt`, `.pptx`, text). Protected files need
   * `password`: the promise rejects with {@link PASSWORD_REQUIRED} / {@link WRONG_PASSWORD}.
   */
  open(bytes: Uint8Array, name: string, password?: string): Promise<DocInfo>;
  getPage(index: number): Promise<PageDisplayList>;
  getGlyphOutlines(face: number, glyphs: number[]): Promise<GlyphOutline[]>;
  exportPdf(): Promise<Uint8Array>;
  /** Start a blank document. */
  newDocument(name: string): Promise<DocInfo>;
  /** Run an editing command. */
  command(cmd: TimedCommand): Promise<EditResult>;
  /** Insert a PNG/JPEG picture at the selection. */
  insertImage(bytes: Uint8Array, name: string): Promise<EditResult>;
  /** Bytes of a media part referenced by an `image` display command. */
  getMedia(name: string): Promise<Uint8Array>;
  /** Serialize the open document as .docx. */
  saveDocx(): Promise<Uint8Array>;
  /** Mark the current state as saved. */
  markSaved(): Promise<void>;
  /** Speaker notes of a slide (presentations; empty for documents). */
  getSlideNotes(index: number): Promise<string>;
  /** Saves the open presentation as `.pptx`. */
  savePptx(): Promise<Uint8Array>;
  /** Handout / notes PDF, slide pictures or presentation package (presentations). */
  exportSlides?(options: import('./slideExport').SlideExportOptions): Promise<Uint8Array>;
  /** PDF of a print job (CORE-PRINT-001): pages, pages per sheet, copies, markup, slides layout. */
  printPdf?(options: import('./print').PrintOptions): Promise<Uint8Array>;
  /** Sheet `index` of a print job's PDF, for the print panel's preview. */
  printPreview?(
    options: import('./print').PrintOptions,
    index: number,
  ): Promise<import('./print').PrintPreview>;
  /** Run a slide editing command (presentations). */
  slideCommand(cmd: SlideCommand): Promise<SlideEditResult>;
  /**
   * Insert a PNG/JPEG picture on the current slide, a video / audio clip when
   * `name` has a media extension (`.mp4`, `.webm`, `.mp3`, `.wav`, …), or a 3D model
   * (`.glb`, `.gltf`, `.obj`, `.stl`, stored as GLB).
   */
  slideInsertPicture(bytes: Uint8Array, name: string): Promise<SlideEditResult>;
  /**
   * Live collaboration (`bo_engine::collab`): bind the open document to a session.
   * `seed` = first editor of the room; returns the change to publish.
   */
  collabStart?(seed: boolean, clientId?: number): Promise<CollabChange>;
  /** Apply a change from the replica (JSON `CollabChange`). */
  collabApply?(change: string): Promise<EditResult>;
  /** Geometry of collaborators' selections (UTF-16 `[anchor, head]`). */
  collabPeers?(selections: [number, number][]): Promise<PeerRects[]>;
  collabStop?(): Promise<void>;
  /** Bind the open presentation; `clientId` numbers slides created in the session. */
  slideCollabStart?(seed: boolean, clientId: number): Promise<CollabChange>;
  slideCollabApply?(change: string): Promise<SlideEditResult>;
  /** Boxes of collaborators' selected objects (keys `slide/shape`). */
  slideCollabPeers?(keys: string[]): Promise<({ slide: number; rect: SlideBox } | null)[]>;
  /** Collaborators changed the document (set by collaboration wrappers). */
  onExternalChange?(listener: (r: EditResult) => void): () => void;
  onSlideExternalChange?(listener: (r: SlideEditResult) => void): () => void;
  /** Collaborators' carets / selected objects changed. */
  onPeers?(listener: (peers: PeerCursor[]) => void): () => void;
  onSlidePeers?(listener: (peers: SlidePeer[]) => void): () => void;
  /**
   * Slide show export (`bo_engine::slide_edit::video`): lay the show out on a timeline;
   * then frames by index, as straight RGBA (or PNG), or added to an animated GIF.
   */
  videoBegin?(opts: ShowVideoOptions): Promise<ShowVideoInfo>;
  videoFrame?(index: number, png?: boolean): Promise<Uint8Array>;
  videoGifAdd?(index: number): Promise<void>;
  videoGifFinish?(): Promise<Uint8Array>;
  videoEnd?(): Promise<void>;
  dispose(): void;
}

/** Options of a slide show export (`bo_engine::slide_edit::video::VideoOptions`). */
export interface ShowVideoOptions {
  /** Frame width in pixels (height from the slide ratio). */
  width: number;
  fps: number;
  /** Time on a slide without its own timing. */
  secondsPerSlide: number;
  /** Use the slides' "advance after" times. */
  useTimings: boolean;
  /** GIF: loop forever. */
  loop?: boolean;
  /** GIF: error-diffusion dithering. */
  dither?: boolean;
}

/** Size and timeline of a slide show export. */
export interface ShowVideoInfo {
  width: number;
  height: number;
  fps: number;
  frameCount: number;
  durationMs: number;
  /** Slides in the video (hidden slides left out): 0-based `slide`, its place on the timeline. */
  slides: { slide: number; startMs: number; transitionMs: number; lengthMs: number }[];
}

/** Container and codec string of a video encoding (WebCodecs). */
export interface VideoCodecChoice {
  container: 'mp4' | 'webm';
  /** WebCodecs codec string (`avc1.…`, `vp09.…`, `vp8`). */
  codec: string;
}

/** One encoded video frame. */
export interface EncodedVideoChunkData {
  data: Uint8Array;
  timestampUs: number;
  durationUs: number;
  key: boolean;
}

/** An open video encoder. */
export interface VideoEncoderSession {
  encode(rgba: Uint8Array, timestampUs: number, durationUs: number, key: boolean): Promise<void>;
  /** Waits for every frame; `description` = decoder configuration (H.264 `avcC`), if any. */
  flush(): Promise<{ chunks: EncodedVideoChunkData[]; description: Uint8Array | null }>;
  close(): void;
}

/** PCM audio read in blocks (a narration track of the video export). */
export interface AudioSource {
  sampleRate: number;
  channels: number;
  /** Length in sample frames. */
  frames: number;
  /** Frames `start..start + count` (one array per channel). */
  read(start: number, count: number): Float32Array[];
}

/** One encoded audio frame. */
export interface EncodedAudioChunkData {
  data: Uint8Array;
  timestampUs: number;
  durationUs: number;
}

/** An encoded audio track: AAC-LC (MP4) or Opus (WebM). */
export interface EncodedAudioTrack {
  codec: 'aac' | 'opus';
  sampleRate: number;
  channels: number;
  /** AAC `AudioSpecificConfig` / Opus `OpusHead`, when the encoder gives one. */
  description: Uint8Array | null;
  chunks: EncodedAudioChunkData[];
}

/** Video encoding of the platform (absent or `probe` → `null` where not available). */
export interface VideoEncoding {
  /**
   * Encodes `source` for `container` (AAC for MP4, Opus for WebM); `null` when the platform
   * has no such audio encoder (the video is then made without sound).
   */
  encodeAudio?(
    container: 'mp4' | 'webm',
    source: AudioSource,
    signal?: AbortSignal,
  ): Promise<EncodedAudioTrack | null>;
  probe(width: number, height: number, fps: number): Promise<VideoCodecChoice | null>;
  open(
    choice: VideoCodecChoice,
    width: number,
    height: number,
    fps: number,
  ): Promise<VideoEncoderSession>;
}

/** A file picked by the user. */
export interface OpenedFile {
  name: string;
  /** Absolute path on desktop; undefined on the web. */
  path?: string;
  bytes: Uint8Array;
}

/** Opening and saving files. */
export interface FileAccess {
  /** Ask the user for a file; resolves `null` when cancelled. */
  openFile(accept: string[]): Promise<OpenedFile | null>;
  /** Save bytes; returns the saved path/name or `null` when cancelled. */
  saveFile(bytes: Uint8Array, suggestedName: string, mime: string): Promise<string | null>;
  /** Desktop: read a file by its absolute path (recent files). */
  openPath?(path: string): Promise<OpenedFile>;
}

/** Where a document comes from and goes back to. */
export interface DocumentRef {
  provider: string;
  id: string;
  name: string;
}

export interface StoredDocument {
  ref: DocumentRef;
  bytes: Uint8Array;
  /** Opaque version for optimistic concurrency (If-Match). */
  version?: string;
  permissions: { edit: boolean; download: boolean; share: boolean };
}

/** Document source: LocalStorageProvider | BlueOfficeServerProvider | BlueDriveProvider | WopiProvider. */
export interface DocumentStorage {
  readonly id: string;
  open(ref?: DocumentRef): Promise<StoredDocument | null>;
  save(doc: StoredDocument, bytes: Uint8Array): Promise<StoredDocument>;
  saveAs(name: string, bytes: Uint8Array): Promise<StoredDocument | null>;
}

/** What the system clipboard holds (each format when present). */
export interface ClipboardContent {
  text?: string;
  html?: string;
  /** PNG picture. */
  image?: Uint8Array;
}

export interface ClipboardAccess {
  readText(): Promise<string>;
  writeText(text: string): Promise<void>;
  /** Write rich content (HTML + plain-text fallback). */
  writeHtml(html: string, text: string): Promise<void>;
  /** Write several formats at once (plain text, HTML, a PNG picture). */
  writeItems?(items: { text: string; html?: string; png?: Uint8Array }): Promise<void>;
  /** Read text, HTML and a PNG picture (may ask the user for permission). */
  read?(): Promise<ClipboardContent>;
  /** HTML on the clipboard ('' when none or not readable; Paste Special). */
  readHtml?(): Promise<string>;
}

export interface FontDescriptor {
  family: string;
  file: string;
}

export interface FontSource {
  /** Fonts the engine loads, in face-id order. */
  list(): Promise<FontDescriptor[]>;
  load(file: string): Promise<Uint8Array>;
}

export interface PrintService {
  /** Print a PDF produced by the engine. */
  printPdf(pdf: Uint8Array, title: string): Promise<void>;
}

export interface UserInfo {
  id: string;
  name: string;
  avatarUrl?: string;
}

export interface AuthSession {
  readonly user: UserInfo | null;
  getAccessToken(): Promise<string | null>;
  signIn(): Promise<void>;
  signOut(): Promise<void>;
}

/** Messages between windows of the app (presenter ↔ audience screen). */
export interface MessageBus {
  /** Send to the other windows on this channel (not to this one). */
  post(msg: unknown): void;
  onMessage(handler: (msg: unknown) => void): () => void;
  close(): void;
}

export interface WindowManager {
  setTitle(title: string): void;
  openNew(url: string): Promise<void>;
}

export interface OfflineCache {
  get(key: string): Promise<Uint8Array | null>;
  put(key: string, value: Uint8Array): Promise<void>;
  delete(key: string): Promise<void>;
}

export interface RecentFile {
  name: string;
  path: string;
  openedAt: number;
}

/**
 * A `bluedoc://open?host=…&file=…&token=…` deep link (prompt §6A.3); the
 * same fields as the web embed URL `/edit?file=…&host=…`.
 */
export interface HostFileLink {
  app: 'doc' | 'sheet' | 'slide';
  host: string;
  fileId: string;
  token?: string;
}

export interface ShellIntegration {
  recentFiles(): Promise<RecentFile[]>;
  addRecentFile(file: RecentFile): Promise<void>;
  /** Register a handler for local files opened by the OS (file association, second instance). */
  onOpenRequest(handler: (file: OpenedFile) => void): () => void;
  /** Register a handler for host-file deep links (desktop only; web: never fires). */
  onOpenLink?(handler: (link: HostFileLink) => void): () => void;
  /**
   * BlueOffice server that opens host files from deep links (desktop:
   * `BLUEOFFICE_SERVER`, else the stored setting); null when not configured.
   */
  serverUrl?(): Promise<string | null>;
  /** Stores the server setting (null clears it). */
  setServerUrl?(url: string | null): Promise<void>;
  /** The server comes from the environment (`BLUEOFFICE_SERVER`), overriding the setting. */
  serverFromEnvironment?(): Promise<boolean>;
  /** Desktop: the process was started for a file or a deep link (else the Start screen shows). */
  hasLaunchRequest?(): Promise<boolean>;
  /** A small persisted user setting (JSON text), e.g. the BlueDrive sync folders. */
  getSetting?(key: string): Promise<string | null>;
  setSetting?(key: string, value: string | null): Promise<void>;
  /** Opens a web page in the system browser (host sign-in). */
  openExternal?(url: string): Promise<void>;
  /** Sign-in redirects to the app (`bluedoc://auth?code=…&state=…`). */
  onAuthCallback?(handler: (url: string) => void): () => void;
}

/** Signer kinds of the "Ký số" dialog (ADR-0007). Web: `remote` only. */
export type SignerKind = 'file' | 'token' | 'remote';

/** Remote signing service of the generic JSON protocol (`docs/deploy/remote-signing.md`). */
export interface RemoteSignerConfig {
  url: string;
  credentialId?: string;
  accessToken?: string;
}

/** The signer chosen in the dialog. Secrets (PIN, password, token) are never stored. */
export type SignerChoice =
  | { kind: 'file'; fileName: string; fileBase64: string; password?: string }
  | { kind: 'token'; module: string; slot?: number; id?: string; pin: string }
  | ({ kind: 'remote' } & RemoteSignerConfig);

export interface SignRequest {
  signer: SignerChoice;
  /** RFC 3161 time-stamp authority (PAdES B-T / XAdES-T); empty = no time-stamp. */
  tsaUrl?: string;
  /** PDF only: `/Name`, `/Reason`, `/Location`. */
  name?: string;
  reason?: string;
  location?: string;
  documentName: string;
  /** OOXML: the signature signs this signature line (CORE-SIGLINE-001). */
  signatureLine?: SignatureLineSignRequest;
}

/** What signing a signature line adds (`bo_sign::ooxml::SignatureLineRequest`). */
export interface SignatureLineSignRequest {
  /** The line's GUID (`o:signatureline/@id`). */
  id: string;
  /** Typed name. */
  text?: string;
  /** Signer's picture (PNG / JPEG, base64). */
  imageBase64?: string;
  comments?: string;
  /** Pictures of the line as signed and as invalid (PNG, base64). */
  validImageBase64?: string;
  invalidImageBase64?: string;
}

/** A certificate on a USB token (PKCS#11). */
export interface TokenCertificate {
  slot: number;
  tokenLabel: string;
  tokenSerial: string;
  label: string;
  id: string;
  subject: string;
  issuer: string;
  notAfter: string;
}

/** `bo_sign::report` structures (JSON). */
export interface ChainProblemReport {
  code: string;
  subject: string | null;
  message: string;
}
export interface ChainSummary {
  trusted: boolean;
  path: string[];
  anchor: string | null;
  problems: ChainProblemReport[];
}
export interface SignatureSummary {
  signer: string;
  signingTime: string | null;
  coversWholeDocument: boolean;
  timestamp: { time: string; tsa: string; chain: ChainSummary } | null;
  chain: ChainSummary;
  validatedAt: string;
  /** OOXML: id of the signature line this signature signs. */
  signatureLine?: string;
}
/** One signed signature line, verified on its own (`bo_sign::report::SignatureLineStatus`). */
export interface SignatureLineStatus {
  id: string;
  valid: boolean;
  signer: string | null;
  text: string | null;
  error: string | null;
}
export interface SignatureReport {
  kind: 'pdf' | 'ooxml';
  signatures: SignatureSummary[];
  /** PDF document time-stamps (PAdES B-LTA); absent when there are none. */
  documentTimestamps?: {
    timestamp: { time: string; tsa: string; chain: ChainSummary };
    coversWholeDocument: boolean;
  }[];
  /** A signature is broken (document changed) or unreadable. */
  error: string | null;
  /** OOXML: the signatures of signature lines; absent when there are none. */
  signatureLines?: SignatureLineStatus[];
}

/** Defaults configured by the deployment. */
export interface SigningDefaults {
  remoteUrl?: string | null;
  tsaUrl?: string | null;
  pkcs11Module?: string | null;
}

/**
 * Digital signatures (desktop: native `bo-sign` via Tauri; web: WASM
 * two-phase signing + the remote service through `fetch`).
 */
export interface SigningService {
  readonly kinds: SignerKind[];
  defaults(): Promise<SigningDefaults>;
  /** Desktop only: certificates on the tokens of a PKCS#11 module. */
  listTokenCertificates?(module: string): Promise<TokenCertificate[]>;
  /** Signs a PDF or OOXML file; returns the signed file. */
  sign(bytes: Uint8Array, request: SignRequest): Promise<Uint8Array>;
  /** Verifies every signature against the configured trust anchors. */
  verify(bytes: Uint8Array): Promise<SignatureReport>;
}

/** Result of an update check (desktop auto-update, Tauri updater). */
export type UpdateStatus =
  /** This build has no updater (dev builds, web). */
  | { state: 'disabled' }
  /** No signing key / endpoint configured in the release config. */
  | { state: 'notConfigured' }
  | { state: 'upToDate'; current: string }
  | { state: 'available'; current: string; version: string; notes?: string; date?: string }
  | { state: 'failed'; message: string };

/** Application updates (desktop only). */
export interface UpdateService {
  check(): Promise<UpdateStatus>;
  /** Downloads, verifies and installs the announced update, then restarts the app. */
  install(): Promise<void>;
}

/** Everything platform-specific, in one object. */
export interface Platform {
  readonly kind: 'web' | 'desktop';
  files: FileAccess;
  storage: DocumentStorage;
  clipboard: ClipboardAccess;
  fonts: FontSource;
  print: PrintService;
  engine: EngineHost;
  auth: AuthSession;
  window: WindowManager;
  offline: OfflineCache;
  shell: ShellIntegration;
  /** Channel between windows (web: BroadcastChannel, desktop: Tauri events). */
  channel?(name: string): MessageBus;
  /** BlueSheet engine (absent in hosts that only embed BlueDoc). */
  sheet?: import('./sheet').SheetHost;
  /** Digital signatures (absent where not available). */
  signing?: SigningService;
  /** "Check for updates" (desktop). */
  updates?: UpdateService;
  /** Video encoding (WebCodecs where the browser / webview has it). */
  video?: VideoEncoding;
  /** Other monitors for the audience screen (absent: a plain second window). */
  displays?: import('./slideShow').DisplayService;
  /** Mail-merge recipients from the host (SQL through the server); absent where none. */
  mergeData?: import('./mailMerge').MergeDataProvider;
  /** Sends merged e-mails (server or desktop); absent where none. */
  mail?: import('./mailMerge').MailProvider;
  /** Pictures from the web for the sheet `IMAGE` function (absent: not allowed). */
  images?: import('./remoteImages').RemoteImages;
  /** Version history of the open file (hosted sessions; CORE-VERSION-001). */
  versions?: import('./versions').VersionHistory;
  /** People to @mention in comments + their notifications (hosted sessions; CORE-COMMENT-001). */
  people?: import('./people').PeopleDirectory;
  /** Scheduled BlueScript runs of the open file (hosted sessions; CORE-SCRIPT-002). */
  scriptSchedules?: import('./scriptSchedules').ScriptSchedules;
  /** Text to speech for read aloud (absent where the platform cannot speak). */
  speech?: import('./speech').SpeechProvider;
  /** Translation (absent where none is configured; CORE-LANG-001). */
  translation?: import('./translation').TranslationProvider;
  /** Folders and web pages for Get & Transform data (SHEET-PQ-001; absent: files only). */
  dataSources?: import('./sheetQuery').QueryDataSources;
  /** Speech to text for dictation (absent where the platform cannot listen; CORE-LANG-002). */
  dictation?: import('./dictation').DictationProvider;
  /** Microphone / camera recording (absent where the platform has none; SLIDE-REC-001). */
  media?: import('./mediaCapture').MediaCaptureService;
}
