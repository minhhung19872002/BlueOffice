/**
 * Platform Abstraction Layer (prompt §3.4).
 *
 * UI and editor code never call browser-only or Tauri-only APIs for
 * platform-dependent work: everything goes through {@link Platform}.
 * `apps/web` builds a `WebPlatform`, `apps/desktop` a `DesktopPlatform`;
 * the rest of the code base is shared 100%.
 */

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
  | { kind: 'headerRow'; on: boolean };

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

export type DocPasteMode = 'keep' | 'merge' | 'text';

export interface FindOptions {
  matchCase?: boolean;
  wholeWord?: boolean;
  regex?: boolean;
}

export interface FindMatch {
  p: number;
  start: number;
  end: number;
}

export type CharFormat =
  | { kind: 'bold' | 'italic' | 'underline' | 'strike' | 'superscript' | 'subscript' | 'clear' }
  | { kind: 'font'; name: string }
  | { kind: 'size'; halfPoints: number }
  | { kind: 'color'; hex: string | null }
  | { kind: 'highlight'; hex: string | null };

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
  /** Previewed record (0-based) or null when field names are shown. */
  preview: number | null;
}

/** Review pane state (mirrors `bo_engine::review::ReviewState`). */
export interface ReviewState {
  trackChanges: boolean;
  author: string;
  view: RevisionView;
  revisions: RevisionItem[];
  comments: CommentInfo[];
}

/** Editing command sent to the engine (JSON bridge, ADR-0008). */
export type EditCommand =
  | { cmd: 'state' | 'selectAll' | 'deleteBackward' | 'deleteForward' | 'splitParagraph' }
  | { cmd: 'undo' | 'redo' | 'insertPageBreak' | 'copy' | 'cut' }
  | { cmd: 'setSelection'; selection: Selection }
  | { cmd: 'click'; page: number; x: number; y: number; extend?: boolean; count?: number }
  | { cmd: 'dragTo'; page: number; x: number; y: number }
  | { cmd: 'move'; motion: Motion; extend?: boolean }
  | { cmd: 'insertText'; text: string }
  | { cmd: 'formatText'; format: CharFormat }
  | { cmd: 'formatParagraph'; format: ParaFormat }
  | { cmd: 'applyStyle'; id: string }
  | { cmd: 'insertTable'; rows: number; cols: number }
  | { cmd: 'insertSectionBreak'; kind: 'nextPage' | 'continuous' | 'evenPage' | 'oddPage' }
  | { cmd: 'find'; query: string; options?: FindOptions }
  | { cmd: 'selectMatch'; match: FindMatch }
  | { cmd: 'replace'; match: FindMatch; with: string }
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
  /** Equations (CORE-MATH-007): insert from UnicodeMath / LaTeX, query, linear ↔ built-up. */
  | { cmd: 'insertEquation'; text: string; latex?: boolean; display?: boolean; replace?: boolean }
  | { cmd: 'equation' }
  | { cmd: 'equationFormat'; linear: boolean }
  /** Grammar and style as you type (CORE-GRAM-001). */
  | { cmd: 'setGrammar'; on: boolean }
  | { cmd: 'grammarIgnore'; rule: string; text: string }
  | { cmd: 'closeHeaderFooter' | 'notes' | 'clearFormatting' | 'fontList' }
  | { cmd: 'pickFormat' | 'applyFormat' | 'cancelFormat' }
  | { cmd: 'insertField'; instr: string }
  | { cmd: 'insertNote'; endnote?: boolean; text?: string }
  | { cmd: 'setNoteText'; endnote?: boolean; id: number; text: string }
  | { cmd: 'newFromTemplate'; id: string; name: string }
  | { cmd: 'review' | 'acceptAllRevisions' | 'rejectAllRevisions' }
  | { cmd: 'setTrackChanges'; on: boolean }
  /** TCVN3 / VNI text → Unicode (legacy Vietnamese fonts); one undo step. */
  | { cmd: 'convertLegacyText' }
  | { cmd: 'setAuthor'; name: string }
  | { cmd: 'setRevisionView'; view: RevisionView }
  | { cmd: 'acceptRevision' | 'rejectRevision'; id?: number }
  | { cmd: 'nextRevision'; forward: boolean }
  | { cmd: 'addComment'; text: string }
  | { cmd: 'replyComment'; id: number; text: string }
  | { cmd: 'resolveComment'; id: number; done: boolean }
  | { cmd: 'deleteComment' | 'selectComment'; id: number }
  | { cmd: 'nextComment'; forward: boolean }
  | { cmd: 'updateFields' | 'insertToc' | 'crossReferenceTargets' }
  | { cmd: 'insertCaption'; label: string; text: string }
  | { cmd: 'insertCrossReference'; target: string; page: boolean }
  | { cmd: 'mergeData'; name: string; base64: string }
  | { cmd: 'mergeInsertField'; name: string }
  | { cmd: 'mergePreview'; record: number | null }
  | { cmd: 'mergeToDocument' }
  | { cmd: 'setSavePassword'; password?: string };

/** A command plus the host clock (Unix ms) used to date new revisions and comments. */
export type TimedCommand = EditCommand & { now?: number };

export interface ClipPayload {
  text: string;
  html: string;
  internal: string;
}

/** Result of an editing command. */
export interface EditResult {
  info: DocInfo;
  selection: Selection;
  caret: CaretRect | null;
  rects: SelRect[];
  /** Pages whose display list changed. */
  dirty: number[];
  format: FormatState;
  modified: boolean;
  matches?: FindMatch[];
  count?: number;
  clip?: ClipPayload;
  error?: string;
  /** Present after document changes and review commands. */
  review?: ReviewState;
  /** Cross-reference targets (`crossReferenceTargets`). */
  targets?: RefTarget[];
  /** Mail-merge state (after merge commands and `state`). */
  merge?: MergeState;
  /** While collaborating: the change to share and the selection as UTF-16 offsets. */
  collab?: { change: CollabChange; selection: [number, number] };
  /** Open header/footer editor (the selection is in it). */
  story?: StoryState;
  /** Footnotes and endnotes (`notes` and note commands). */
  notes?: NoteInfo[];
  /** Font picker entries (`fontList`). */
  fonts?: FontChoice[];
  /** The Format Painter holds picked formatting. */
  painter?: boolean;
  /** Misspelt words to underline (`spelling`, `spellIgnore`, `spellAddWord`, `setSpelling`). */
  spelling?: SpellMark[];
  /** The misspelt word at a point (`spellAt`). */
  spell?: SpellInfo;
  /** The equation the selection covers or the caret follows. */
  equation?: EquationInfo;
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
  kind: 'shape' | 'picture' | 'group' | 'connector' | 'table' | 'chart' | 'graphic';
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

/** Slide editing command (JSON bridge, ADR-0008). */
export type SlideCommand =
  | { cmd: 'state' | 'selectAll' | 'delete' | 'duplicate' | 'group' | 'ungroup' }
  /** Password for the next saves ([MS-OFFCRYPTO]); empty removes it. */
  | { cmd: 'setSavePassword'; password?: string }
  | { cmd: 'undo' | 'redo' }
  /** New comment thread on the current slide, or a reply to thread `parent`. */
  | { cmd: 'addComment'; text: string; author?: string; date?: string; parent?: string }
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
  | { cmd: 'textExit' | 'copy' | 'cut' | 'paste' }
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
  | { cmd: 'show'; slide: number }
  | {
      cmd: 'insertChart';
      kind: SlideChartKind;
      title?: string;
      data?: SlideChartData;
      legend?: SlideLegend;
      rect?: SlideRect;
    }
  | {
      cmd: 'setChart';
      id?: number;
      kind?: SlideChartKind;
      /** Empty = no title. */
      title?: string;
      /** `none` = no legend. */
      legend?: SlideLegend;
      data?: SlideChartData;
    };

/** Slide transition effects (ECMA-376 Part 1, §19.3.1.50 transition children). */
export type SlideTransitionKind =
  'none' | 'cut' | 'fade' | 'push' | 'wipe' | 'cover' | 'pull' | 'split' | 'random';

export interface SlideTransitionSpec {
  /** `keep` keeps the effect and changes the timing only. */
  kind: SlideTransitionKind | 'keep';
  /** `l`/`u`/`r`/`d` (push, wipe, cover, pull), `horz`/`vert` (split). */
  dir?: string;
  durMs?: number;
  advanceOnClick?: boolean;
  /** 0 = no automatic advance. */
  advanceAfterMs?: number;
}

export interface SlideTransitionInfo {
  /** Modelled kind, or the element name of an effect BlueSlide does not model. */
  kind: string;
  known: boolean;
  dir: string | null;
  durMs: number;
  advanceOnClick: boolean;
  advanceAfterMs: number | null;
}

export type SlideAnimEffect = 'appear' | 'fade' | 'flyIn' | 'wipe';
export type SlideAnimTrigger = 'onClick' | 'withPrevious' | 'afterPrevious';

export interface SlideAnimSpec {
  effect?: SlideAnimEffect;
  class?: 'entr' | 'exit';
  /** `l` | `r` | `t` | `b` (fly-in, wipe). */
  direction?: string;
  trigger?: SlideAnimTrigger;
  durMs?: number;
  delayMs?: number;
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
  media: SlideMediaInfo[];
}

export type SlideChartKind = 'column' | 'bar' | 'line' | 'area' | 'pie' | 'doughnut';
export type SlideLegend = 'none' | 'r' | 'l' | 't' | 'b' | 'tr';

/** Data grid of a chart: categories down, series across. */
export interface SlideChartData {
  categories: string[];
  series: { name: string; values: (number | null)[] }[];
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
  comments: { id: string; author: string; date: string; text: string }[];
}

export interface SlideEditResult {
  info: DocInfo;
  slide: number;
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
  error?: string;
  /** While collaborating: the change to share and the selected object key. */
  collab?: { change: CollabChange; shape?: string };
  /** Comment threads of the current slide. */
  comments?: SlideThreadInfo[];
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
  /** Answer of the `show` command. */
  show?: SlideShowInfo;
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
  /** Run a slide editing command (presentations). */
  slideCommand(cmd: SlideCommand): Promise<SlideEditResult>;
  /**
   * Insert a PNG/JPEG picture on the current slide, or a video / audio clip when
   * `name` has a media extension (`.mp4`, `.webm`, `.mp3`, `.wav`, …).
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
  dispose(): void;
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
}
