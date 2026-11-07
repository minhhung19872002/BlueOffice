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
      /** Media part name, bytes via `EngineHost.getMedia`. */
      media: string;
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
  | { op: 'popClip' };

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

/** Editing command sent to the engine (JSON bridge, ADR-0007). */
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
  | { cmd: 'paste'; internal?: string; html?: string; text?: string };

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
  command(cmd: EditCommand): Promise<EditResult>;
  /** Insert a PNG/JPEG picture at the selection. */
  insertImage(bytes: Uint8Array, name: string): Promise<EditResult>;
  /** Bytes of a media part referenced by an `image` display command. */
  getMedia(name: string): Promise<Uint8Array>;
  /** Serialize the open document as .docx. */
  saveDocx(): Promise<Uint8Array>;
  /** Mark the current state as saved. */
  markSaved(): Promise<void>;
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

export interface ShellIntegration {
  recentFiles(): Promise<RecentFile[]>;
  addRecentFile(file: RecentFile): Promise<void>;
  /** Register a handler for files opened by the OS (file association, deep link). */
  onOpenRequest(handler: (file: OpenedFile) => void): () => void;
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
}
