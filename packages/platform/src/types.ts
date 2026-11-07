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
}

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
  | { op: 'pushClipPath'; segs: PathSeg[] };

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
}

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
  | 'paragraphDelete';

/** A tracked change of the main text (mirrors `bo_edit::RevisionItem`). */
export interface RevisionItem {
  id: number;
  kind: RevisionKind;
  author: string;
  date: string | null;
  start: Pos;
  end: Pos;
  text: string;
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
  | { cmd: 'paste'; internal?: string; html?: string; text?: string }
  | { cmd: 'review' | 'acceptAllRevisions' | 'rejectAllRevisions' }
  | { cmd: 'setTrackChanges'; on: boolean }
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
  | { cmd: 'insertCrossReference'; target: string; page: boolean };

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
}

// ---------------------------------------------------------------- services

/** The core engine. Same API whether it runs as WASM (web) or natively (desktop). */
export interface EngineHost {
  readonly backend: 'wasm' | 'native';
  /** Load fonts and get ready. Idempotent. */
  init(): Promise<void>;
  open(bytes: Uint8Array, name: string): Promise<DocInfo>;
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

export interface ClipboardAccess {
  readText(): Promise<string>;
  writeText(text: string): Promise<void>;
  /** Write rich content (HTML + plain-text fallback). */
  writeHtml(html: string, text: string): Promise<void>;
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
  /** BlueSheet engine (absent in hosts that only embed BlueDoc). */
  sheet?: import('./sheet').SheetHost;
}
