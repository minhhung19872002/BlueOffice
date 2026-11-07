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
  | { op: 'line'; x1: number; y1: number; x2: number; y2: number; width: number; color: Rgba }
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
