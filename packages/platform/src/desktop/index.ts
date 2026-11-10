/**
 * Desktop platform (Tauri 2). The engine runs natively in the Tauri process
 * (`apps/desktop/src-tauri`), exposing the same {@link EngineHost} API as the
 * WASM worker. Binary payloads use Tauri's raw IPC bodies (no JSON arrays).
 */
import { invoke } from '@tauri-apps/api/core';
import { emit, listen } from '@tauri-apps/api/event';
import { BrowserClipboard, FileAccessStorage, LocalRecentFiles } from '../shared';
import { DesktopDisplays } from './displays';
import type {
  CollabChange,
  DocInfo,
  DocumentStorage,
  EditResult,
  EngineHost,
  FileAccess,
  FontDescriptor,
  FontSource,
  GlyphOutline,
  HostFileLink,
  MessageBus,
  OfflineCache,
  OpenedFile,
  PageDisplayList,
  PeerRects,
  ShowVideoInfo,
  ShowVideoOptions,
  SlideBox,
  TimedCommand,
  Platform,
  PrintService,
  RecentFile,
  ShellIntegration,
  SignatureReport,
  SignerKind,
  SigningDefaults,
  SigningService,
  SignRequest,
  SlideCommand,
  SlideEditResult,
  UpdateService,
  UpdateStatus,
  WindowManager,
} from '../types';
import { RpcSheetHost, type SheetMethod, type SheetRpc } from '../sheet';
import type { SlideExportOptions } from '../slideExport';
import type { PrintOptions, PrintPreview } from '../print';
import { BundledFontSource } from '../web';
import { WebEngineHost } from '../web/WebEngineHost';
import { webSheetTransport } from '../web/WebSheetHost';
import { DesktopAuth, type DesktopScheme } from './auth';
import { WebCodecsVideo } from '../webcodecs';
import { FetchRemoteImages } from '../remoteImages';
import { webSpeech } from '../speech';
import { fromBase64 } from '../web/signing';
import type { PickedFolder, QueryDataSources } from '../sheetQuery';
import { webDictation } from '../dictation';
import { webHandwriting } from '../handwriting';
import { webMediaCapture } from '../mediaCapture';

export { DesktopAuth, DESKTOP_OIDC_PATH, isOidcCallback } from './auth';
export type { DesktopAuthOptions, DesktopScheme } from './auth';

/** Minimal `invoke` signature so tests can inject a fake. */
export type Invoke = <T>(
  cmd: string,
  args?: Record<string, unknown> | Uint8Array,
  options?: { headers: Record<string, string> },
) => Promise<T>;

const toBytes = (v: ArrayBuffer | Uint8Array | number[]): Uint8Array =>
  v instanceof Uint8Array ? v : v instanceof ArrayBuffer ? new Uint8Array(v) : Uint8Array.from(v);

/** Native engine via Tauri commands. */
export class NativeEngineHost implements EngineHost {
  readonly backend = 'native' as const;
  constructor(private readonly call: Invoke = invoke as Invoke) {}
  async init(): Promise<void> {
    // The native engine answers with its face count; none = it did not start.
    const faces = await this.call<number>('engine_init');
    if (!faces) throw new Error('engine.nativeUnavailable');
  }
  open(bytes: Uint8Array, name: string, password?: string): Promise<DocInfo> {
    const headers: Record<string, string> = { 'x-file-name': encodeURIComponent(name) };
    if (password) headers['x-password'] = encodeURIComponent(password);
    return this.call<DocInfo>('engine_open', bytes, { headers });
  }
  async getPage(index: number): Promise<PageDisplayList> {
    return JSON.parse(await this.call<string>('engine_page', { index })) as PageDisplayList;
  }
  async getGlyphOutlines(face: number, glyphs: number[]): Promise<GlyphOutline[]> {
    return JSON.parse(
      await this.call<string>('engine_glyph_outlines', { face, glyphs }),
    ) as GlyphOutline[];
  }
  async exportPdf(): Promise<Uint8Array> {
    return toBytes(await this.call<ArrayBuffer>('engine_export_pdf'));
  }
  newDocument(name: string): Promise<DocInfo> {
    return this.call<DocInfo>('engine_new', { name });
  }
  async command(cmd: TimedCommand): Promise<EditResult> {
    return JSON.parse(
      await this.call<string>('engine_command', { json: JSON.stringify(cmd) }),
    ) as EditResult;
  }
  async insertImage(bytes: Uint8Array, name: string): Promise<EditResult> {
    return JSON.parse(
      await this.call<string>('engine_insert_image', bytes, {
        headers: { 'x-file-name': encodeURIComponent(name) },
      }),
    ) as EditResult;
  }
  async getMedia(name: string): Promise<Uint8Array> {
    return toBytes(await this.call<ArrayBuffer>('engine_media', { name }));
  }
  async saveDocx(): Promise<Uint8Array> {
    return toBytes(await this.call<ArrayBuffer>('engine_save_docx'));
  }
  getSlideNotes(index: number): Promise<string> {
    return this.call<string>('engine_slide_notes', { index });
  }
  async savePptx(): Promise<Uint8Array> {
    return toBytes(await this.call<ArrayBuffer>('engine_save_pptx'));
  }
  async exportSlides(options: SlideExportOptions): Promise<Uint8Array> {
    return toBytes(
      await this.call<ArrayBuffer>('engine_slide_export', { json: JSON.stringify(options) }),
    );
  }
  async printPdf(options: PrintOptions): Promise<Uint8Array> {
    return toBytes(
      await this.call<ArrayBuffer>('engine_print_pdf', { json: JSON.stringify(options) }),
    );
  }
  async printPreview(options: PrintOptions, index: number): Promise<PrintPreview> {
    return JSON.parse(
      await this.call<string>('engine_print_preview', { json: JSON.stringify(options), index }),
    ) as PrintPreview;
  }
  async slideCommand(cmd: SlideCommand): Promise<SlideEditResult> {
    return JSON.parse(
      await this.call<string>('engine_slide_command', { json: JSON.stringify(cmd) }),
    ) as SlideEditResult;
  }
  async slideInsertPicture(bytes: Uint8Array, name: string): Promise<SlideEditResult> {
    return JSON.parse(
      await this.call<string>('engine_slide_insert_picture', bytes, {
        headers: { 'x-file-name': encodeURIComponent(name) },
      }),
    ) as SlideEditResult;
  }
  async markSaved(): Promise<void> {
    await this.call<null>('engine_mark_saved');
  }
  async collabStart(seed: boolean, clientId?: number): Promise<CollabChange> {
    return JSON.parse(
      await this.call<string>('engine_collab_start', { seed, client: clientId ?? 0 }),
    ) as CollabChange;
  }
  async collabApply(change: string): Promise<EditResult> {
    return JSON.parse(
      await this.call<string>('engine_collab_apply', { json: change }),
    ) as EditResult;
  }
  async collabPeers(selections: [number, number][]): Promise<PeerRects[]> {
    return JSON.parse(
      await this.call<string>('engine_collab_peers', { json: JSON.stringify(selections) }),
    ) as PeerRects[];
  }
  async collabStop(): Promise<void> {
    await this.call<null>('engine_collab_stop');
  }
  async slideCollabStart(seed: boolean, clientId: number): Promise<CollabChange> {
    return JSON.parse(
      await this.call<string>('engine_slide_collab_start', { seed, client: clientId }),
    ) as CollabChange;
  }
  async slideCollabApply(change: string): Promise<SlideEditResult> {
    return JSON.parse(
      await this.call<string>('engine_slide_collab_apply', { json: change }),
    ) as SlideEditResult;
  }
  async slideCollabPeers(keys: string[]): Promise<({ slide: number; rect: SlideBox } | null)[]> {
    return JSON.parse(
      await this.call<string>('engine_slide_collab_peers', { json: JSON.stringify(keys) }),
    ) as ({ slide: number; rect: SlideBox } | null)[];
  }
  async videoBegin(opts: ShowVideoOptions): Promise<ShowVideoInfo> {
    return JSON.parse(
      await this.call<string>('engine_video_begin', { json: JSON.stringify(opts) }),
    ) as ShowVideoInfo;
  }
  async videoFrame(index: number, png = false): Promise<Uint8Array> {
    return toBytes(await this.call<ArrayBuffer>('engine_video_frame', { index, png }));
  }
  async videoGifAdd(index: number): Promise<void> {
    await this.call<null>('engine_video_gif_add', { index });
  }
  async videoGifFinish(): Promise<Uint8Array> {
    return toBytes(await this.call<ArrayBuffer>('engine_video_gif_finish'));
  }
  async videoEnd(): Promise<void> {
    await this.call<null>('engine_video_end');
  }
  dispose(): void {}
}

/** Transport to the native BlueSheet engine (apps/desktop/src-tauri/src/sheet_cmds.rs). */
function nativeSheetRpc(call: Invoke): SheetRpc {
  return async (method: SheetMethod, args: unknown[]) => {
    if (method === 'open') {
      const headers: Record<string, string> = {
        'x-file-name': encodeURIComponent(String(args[1])),
      };
      if (typeof args[2] === 'string' && args[2])
        headers['x-password'] = encodeURIComponent(args[2]);
      return call('sheet_open', args[0] as Uint8Array, { headers });
    }
    if (method === 'save' || method === 'exportPdf' || method === 'printPdf') {
      return toBytes(await call<ArrayBuffer>('sheet_binary', { method, args }));
    }
    return call('sheet_call', { method, args });
  };
}

/** BlueSheet engine running natively (apps/desktop/src-tauri/src/sheet_cmds.rs). */
export function createNativeSheetHost(call: Invoke = invoke as Invoke): RpcSheetHost {
  return new RpcSheetHost(nativeSheetRpc(call));
}

const need = <T>(f: T | null | undefined, name: string): T => {
  if (!f) throw new Error(`engine.unsupported: ${name}`);
  return f;
};

/**
 * Desktop engine (prompt §3.4 `engine`: "desktop = native, fallback WASM"):
 * the native engine of the Tauri process, or — when it does not start (no IPC,
 * no fonts) — the WASM engine of the web build in a Web Worker, with the
 * bundled fonts served from the app's `fonts/` folder.
 */
export class FallbackEngineHost implements EngineHost {
  private active: EngineHost;
  private ready: Promise<void> | null = null;

  constructor(
    private readonly native: EngineHost,
    private readonly wasm: () => EngineHost,
    private readonly onFallback: (reason: unknown) => void = (e) =>
      console.warn('native engine unavailable, using WASM', e),
  ) {
    this.active = native;
  }

  get backend(): 'wasm' | 'native' {
    return this.active.backend;
  }

  init(): Promise<void> {
    this.ready ??= (async () => {
      try {
        await this.native.init();
      } catch (e) {
        this.onFallback(e);
        this.active = this.wasm();
        await this.active.init();
      }
    })();
    return this.ready;
  }

  private async on(): Promise<EngineHost> {
    await this.init();
    return this.active;
  }

  async open(bytes: Uint8Array, name: string, password?: string): Promise<DocInfo> {
    return (await this.on()).open(bytes, name, password);
  }
  async getPage(index: number): Promise<PageDisplayList> {
    return (await this.on()).getPage(index);
  }
  async getGlyphOutlines(face: number, glyphs: number[]): Promise<GlyphOutline[]> {
    return (await this.on()).getGlyphOutlines(face, glyphs);
  }
  async exportPdf(): Promise<Uint8Array> {
    return (await this.on()).exportPdf();
  }
  async newDocument(name: string): Promise<DocInfo> {
    return (await this.on()).newDocument(name);
  }
  async command(cmd: TimedCommand): Promise<EditResult> {
    return (await this.on()).command(cmd);
  }
  async insertImage(bytes: Uint8Array, name: string): Promise<EditResult> {
    return (await this.on()).insertImage(bytes, name);
  }
  async getMedia(name: string): Promise<Uint8Array> {
    return (await this.on()).getMedia(name);
  }
  async saveDocx(): Promise<Uint8Array> {
    return (await this.on()).saveDocx();
  }
  async markSaved(): Promise<void> {
    return (await this.on()).markSaved();
  }
  async getSlideNotes(index: number): Promise<string> {
    return (await this.on()).getSlideNotes(index);
  }
  async savePptx(): Promise<Uint8Array> {
    return (await this.on()).savePptx();
  }
  async exportSlides(options: SlideExportOptions): Promise<Uint8Array> {
    const h = await this.on();
    return need(h.exportSlides, 'exportSlides').call(h, options);
  }
  async printPdf(options: PrintOptions): Promise<Uint8Array> {
    const h = await this.on();
    return need(h.printPdf, 'printPdf').call(h, options);
  }
  async printPreview(options: PrintOptions, index: number): Promise<PrintPreview> {
    const h = await this.on();
    return need(h.printPreview, 'printPreview').call(h, options, index);
  }
  async slideCommand(cmd: SlideCommand): Promise<SlideEditResult> {
    return (await this.on()).slideCommand(cmd);
  }
  async slideInsertPicture(bytes: Uint8Array, name: string): Promise<SlideEditResult> {
    return (await this.on()).slideInsertPicture(bytes, name);
  }
  async collabStart(seed: boolean, clientId?: number): Promise<CollabChange> {
    const h = await this.on();
    return need(h.collabStart, 'collabStart').call(h, seed, clientId);
  }
  async collabApply(change: string): Promise<EditResult> {
    const h = await this.on();
    return need(h.collabApply, 'collabApply').call(h, change);
  }
  async collabPeers(selections: [number, number][]): Promise<PeerRects[]> {
    const h = await this.on();
    return need(h.collabPeers, 'collabPeers').call(h, selections);
  }
  async collabStop(): Promise<void> {
    const h = await this.on();
    return need(h.collabStop, 'collabStop').call(h);
  }
  async slideCollabStart(seed: boolean, clientId: number): Promise<CollabChange> {
    const h = await this.on();
    return need(h.slideCollabStart, 'slideCollabStart').call(h, seed, clientId);
  }
  async slideCollabApply(change: string): Promise<SlideEditResult> {
    const h = await this.on();
    return need(h.slideCollabApply, 'slideCollabApply').call(h, change);
  }
  async slideCollabPeers(keys: string[]): Promise<({ slide: number; rect: SlideBox } | null)[]> {
    const h = await this.on();
    return need(h.slideCollabPeers, 'slideCollabPeers').call(h, keys);
  }
  async videoBegin(opts: ShowVideoOptions): Promise<ShowVideoInfo> {
    const h = await this.on();
    return need(h.videoBegin, 'videoBegin').call(h, opts);
  }
  async videoFrame(index: number, png?: boolean): Promise<Uint8Array> {
    const h = await this.on();
    return need(h.videoFrame, 'videoFrame').call(h, index, png);
  }
  async videoGifAdd(index: number): Promise<void> {
    const h = await this.on();
    return need(h.videoGifAdd, 'videoGifAdd').call(h, index);
  }
  async videoGifFinish(): Promise<Uint8Array> {
    const h = await this.on();
    return need(h.videoGifFinish, 'videoGifFinish').call(h);
  }
  async videoEnd(): Promise<void> {
    const h = await this.on();
    return need(h.videoEnd, 'videoEnd').call(h);
  }
  dispose(): void {
    this.active.dispose();
  }
}

/**
 * BlueSheet engine of the desktop: native, or the WASM worker when the
 * native engine did not start (decided by `useNative`, the doc engine's probe).
 */
export function createFallbackSheetHost(
  call: Invoke,
  useNative: () => Promise<boolean>,
  wasm: () => { rpc: SheetRpc; init: () => Promise<void> },
): RpcSheetHost {
  let rpc: SheetRpc | null = null;
  let ready: Promise<void> | null = null;
  const init = (): Promise<void> => {
    ready ??= (async () => {
      if (await useNative()) {
        rpc = nativeSheetRpc(call);
        return;
      }
      const t = wasm();
      await t.init();
      rpc = t.rpc;
    })();
    return ready;
  };
  return new RpcSheetHost(async (method, args) => {
    await init();
    return need(rpc, 'sheet')(method, args);
  }, init);
}

/** Real disk access through native dialogs. */
export class DesktopFileAccess implements FileAccess {
  constructor(private readonly call: Invoke = invoke as Invoke) {}
  async openFile(accept: string[]): Promise<OpenedFile | null> {
    const picked = await this.call<{ name: string; path: string } | null>('pick_file', {
      extensions: accept.map((a) => a.replace(/^\./, '')),
    });
    if (!picked) return null;
    const bytes = toBytes(await this.call<ArrayBuffer>('read_file', { path: picked.path }));
    return { name: picked.name, path: picked.path, bytes };
  }
  async saveFile(bytes: Uint8Array, suggestedName: string): Promise<string | null> {
    return this.call<string | null>('save_file', bytes, {
      headers: { 'x-file-name': encodeURIComponent(suggestedName) },
    });
  }
  async openPath(path: string): Promise<OpenedFile> {
    const bytes = toBytes(await this.call<ArrayBuffer>('read_file', { path }));
    return { name: path.split(/[\\/]/).pop() ?? path, path, bytes };
  }
}

/** A folder as the `pick_folder` / `read_folder` commands return it. */
interface NativeFolder {
  name: string;
  path: string;
  files: { name: string; data: string }[];
}

const toFolder = (f: NativeFolder): PickedFolder => ({
  name: f.name,
  path: f.path,
  files: f.files.map((x) => ({ name: x.name, bytes: fromBase64(x.data) })),
});

/** Get & Transform sources on the desktop: native folder dialog, direct web requests. */
export class DesktopQueryData implements QueryDataSources {
  constructor(private readonly call: Invoke = invoke as Invoke) {}
  async pickFolder(): Promise<PickedFolder | null> {
    const f = await this.call<NativeFolder | null>('pick_folder');
    return f ? toFolder(f) : null;
  }
  async readFolder(path: string): Promise<PickedFolder> {
    return toFolder(await this.call<NativeFolder>('read_folder', { path }));
  }
  async fetchWeb(url: string): Promise<Uint8Array> {
    try {
      return toBytes(await this.call<ArrayBuffer>('fetch_web', { url }));
    } catch (e) {
      throw new Error(typeof e === 'string' ? e : 'network', { cause: e });
    }
  }
}

/**
 * Offline cache in a local folder (`<app data>/offline`, prompt §3.4
 * "desktop = thư mục cache cục bộ"): survives restarts and web view resets,
 * holds offline saves of host files and the crash-recovery copy.
 */
export class DesktopDiskCache implements OfflineCache {
  constructor(private readonly call: Invoke = invoke as Invoke) {}
  async get(key: string): Promise<Uint8Array | null> {
    try {
      return toBytes(await this.call<ArrayBuffer>('cache_get', { key }));
    } catch (e) {
      if (String(e).includes('cache:missing')) return null;
      throw e;
    }
  }
  async put(key: string, value: Uint8Array): Promise<void> {
    await this.call('cache_put', value, { headers: { 'x-cache-key': encodeURIComponent(key) } });
  }
  async delete(key: string): Promise<void> {
    await this.call('cache_delete', { key });
  }
}

/** Desktop uses the fonts compiled into the native engine (same list/order as the web). */
export class NativeFontSource implements FontSource {
  constructor(private readonly call: Invoke = invoke as Invoke) {}
  async list(): Promise<FontDescriptor[]> {
    const files = await this.call<string[]>('bundled_fonts');
    return files.map((file) => ({ family: file.split('-')[0] ?? file, file }));
  }
  async load(): Promise<Uint8Array> {
    throw new Error('fonts.nativeOnly');
  }
}

/**
 * OS print UI (apps/desktop/src-tauri/src/print.rs): the PDF opens in a print
 * window whose web view print dialog starts (Windows WebView2, macOS
 * WKWebView); elsewhere the default PDF viewer prints it.
 */
export class DesktopPrint implements PrintService {
  /** Label of the last print window (`null`: the default viewer was used). */
  lastWindow: string | null = null;
  constructor(private readonly call: Invoke = invoke as Invoke) {}
  async printPdf(pdf: Uint8Array, title: string): Promise<void> {
    this.lastWindow = await this.call<string | null>('print_pdf', pdf, {
      headers: { 'x-file-name': encodeURIComponent(title) },
    });
  }
}

/** Auto-update through the Tauri updater (apps/desktop/src-tauri/src/update.rs). */
export class DesktopUpdates implements UpdateService {
  constructor(private readonly call: Invoke = invoke as Invoke) {}
  async check(): Promise<UpdateStatus> {
    try {
      return await this.call<UpdateStatus>('update_check');
    } catch (e) {
      return { state: 'failed', message: e instanceof Error ? e.message : String(e) };
    }
  }
  async install(): Promise<void> {
    await this.call('update_install');
  }
}

/** Digital signatures in the native process (`sign_cmds.rs`, ADR-0007). */
export class DesktopSigning implements SigningService {
  readonly kinds: SignerKind[] = ['file', 'token', 'remote'];
  constructor(private readonly call: Invoke = invoke as Invoke) {}
  defaults(): Promise<SigningDefaults> {
    return this.call<SigningDefaults>('sign_defaults');
  }
  listTokenCertificates(module: string) {
    return this.call<import('../types').TokenCertificate[]>('sign_token_certificates', { module });
  }
  async sign(bytes: Uint8Array, request: SignRequest): Promise<Uint8Array> {
    return toBytes(
      await this.call<ArrayBuffer>('sign_document', bytes, {
        headers: { 'x-sign-request': encodeURIComponent(JSON.stringify(request)) },
      }),
    );
  }
  async verify(bytes: Uint8Array): Promise<SignatureReport> {
    return JSON.parse(await this.call<string>('verify_document', bytes)) as SignatureReport;
  }
}

class DesktopWindow implements WindowManager {
  constructor(private readonly call: Invoke) {}
  setTitle(title: string): void {
    document.title = title;
    void this.call('set_title', { title }).catch(() => {});
  }
  /** Opens a native window; `url` is an app query such as `?app=sheet`. */
  async openNew(url: string): Promise<void> {
    const query = url.includes('?') ? url.slice(url.indexOf('?') + 1) : url;
    await this.call<string>('open_window', { query });
  }
  /** Tiles the document windows on this window's monitor (View › Arrange All). */
  arrangeAll(): Promise<number> {
    return this.call<number>('arrange_windows');
  }
}

type WindowApp = 'doc' | 'sheet' | 'slide';

/** Which editor opens a file (by extension). */
export function appForPath(name: string): WindowApp {
  if (/\.(xlsx|xlsm|xltx|xltm|xlsb|xls|ods|csv)$/i.test(name)) return 'sheet';
  if (/\.(pptx|pptm|potx|potm|ppsx|ppsm|ppt|odp)$/i.test(name)) return 'slide';
  return 'doc';
}

/** Stored BlueOffice server setting (deep links to host files). */
const SERVER_KEY = 'blueoffice.desktop.server';

class DesktopShell implements ShellIntegration {
  private recent = new LocalRecentFiles('blueoffice.desktop.recent');
  constructor(private readonly call: Invoke) {}
  async recentFiles(): Promise<RecentFile[]> {
    return this.recent.list();
  }
  async addRecentFile(file: RecentFile): Promise<void> {
    this.recent.add(file);
  }
  onOpenLink(handler: (link: HostFileLink) => void): () => void {
    let disposed = false;
    void this.call<HostFileLink | null>('startup_link').then((l) => {
      if (l && !disposed) handler(l);
    });
    const un = listen<HostFileLink>('open-link', (e) => {
      if (!disposed) handler(e.payload);
    });
    return () => {
      disposed = true;
      void un.then((f) => f());
    };
  }
  async serverUrl(): Promise<string | null> {
    // The environment (deployment / tests) wins over the user's setting.
    const env = await this.call<string | null>('server_url').catch(() => null);
    if (env) return env;
    try {
      return localStorage.getItem(SERVER_KEY);
    } catch {
      return null;
    }
  }
  async setServerUrl(url: string | null): Promise<void> {
    try {
      if (url) localStorage.setItem(SERVER_KEY, url.trim().replace(/\/+$/, ''));
      else localStorage.removeItem(SERVER_KEY);
    } catch {
      // Storage unavailable: the environment variable still works.
    }
  }
  async serverFromEnvironment(): Promise<boolean> {
    return !!(await this.call<string | null>('server_url').catch(() => null));
  }
  hasLaunchRequest(): Promise<boolean> {
    // A window opened for a forwarded file (`?path=`) shows that file, not the Start screen.
    if (typeof window !== 'undefined' && new URLSearchParams(window.location.search).has('path')) {
      return Promise.resolve(true);
    }
    return this.call<boolean>('has_launch_request').catch(() => false);
  }
  async getSetting(key: string): Promise<string | null> {
    try {
      return localStorage.getItem(`blueoffice.desktop.${key}`);
    } catch {
      return null;
    }
  }
  async setSetting(key: string, value: string | null): Promise<void> {
    try {
      if (value === null) localStorage.removeItem(`blueoffice.desktop.${key}`);
      else localStorage.setItem(`blueoffice.desktop.${key}`, value);
    } catch {
      // Storage unavailable: the setting lasts for this run only.
    }
  }
  async openExternal(url: string): Promise<void> {
    await this.call('open_url', { url });
  }
  onAuthCallback(handler: (url: string) => void): () => void {
    let disposed = false;
    const un = listen<string>('auth-callback', (e) => {
      if (!disposed) handler(e.payload);
    });
    return () => {
      disposed = true;
      void un.then((f) => f());
    };
  }
  onOpenRequest(handler: (file: OpenedFile) => void): () => void {
    let disposed = false;
    const query = new URLSearchParams(typeof window === 'undefined' ? '' : window.location.search);
    // The editor shown in this window (the shell picks it from the file's type).
    let shown: WindowApp | null = null;
    const openPath = async (path: string) => {
      const name = path.split(/[\\/]/).pop() ?? path;
      const target = appForPath(name);
      if (shown && shown !== target) {
        // This window already edits another kind of document: keep it and
        // open the file in a new window of the right app.
        const q = `app=${target}&path=${encodeURIComponent(path)}`;
        await this.call<string>('open_window', { query: q });
        return;
      }
      const bytes = toBytes(await this.call<ArrayBuffer>('read_file', { path }));
      if (disposed) return;
      shown = target;
      handler({ name, path, bytes });
    };
    const routed = query.get('path');
    if (routed) {
      // A window opened for a file forwarded to another window.
      void openPath(routed);
    } else {
      // File passed on the command line (file association) …
      void this.call<string | null>('startup_file').then((p) => {
        if (p) void openPath(p);
      });
    }
    // … or forwarded later by a second instance / deep link.
    const un = listen<string>('open-file', (e) => void openPath(e.payload));
    return () => {
      disposed = true;
      void un.then((f) => f());
    };
  }
}

/** Window channel over Tauri events (every window of the app hears them). */
export class TauriBus implements MessageBus {
  private readonly me = Math.random().toString(36).slice(2);
  private readonly event: string;
  private readonly unlisten: (() => void)[] = [];
  constructor(name: string) {
    this.event = `bo-channel-${name.replace(/[^A-Za-z0-9_-]/g, '_')}`;
  }
  post(msg: unknown): void {
    void emit(this.event, { from: this.me, msg }).catch(() => {});
  }
  onMessage(handler: (msg: unknown) => void): () => void {
    let un: (() => void) | null = null;
    let done = false;
    void listen<{ from: string; msg: unknown }>(this.event, (e) => {
      if (e.payload.from !== this.me) handler(e.payload.msg);
    }).then((f) => {
      if (done) f();
      else un = f;
    });
    const off = () => {
      done = true;
      un?.();
    };
    this.unlisten.push(off);
    return off;
  }
  close(): void {
    for (const f of this.unlisten.splice(0)) f();
  }
}

export interface DesktopPlatformOptions {
  storage?: (files: FileAccess) => DocumentStorage;
  invoke?: Invoke;
  /** App of this shell: picks the sign-in redirect scheme (`bluedoc://auth/oidc`). */
  scheme?: DesktopScheme;
  /** Base URL of the bundled `fonts/` (WASM fallback engine; default Vite `BASE_URL`). */
  baseUrl?: string;
  /** WASM engines used when the native engine does not start (tests inject fakes). */
  wasmEngine?: () => EngineHost;
  wasmSheet?: () => { rpc: SheetRpc; init: () => Promise<void> };
}

/** The desktop platform; `auth` is configured with the BlueOffice server once known. */
export interface DesktopPlatform extends Platform {
  auth: DesktopAuth;
  updates: UpdateService;
}

/** Build the desktop platform. */
export function createDesktopPlatform(opts: DesktopPlatformOptions = {}): DesktopPlatform {
  const call = opts.invoke ?? (invoke as Invoke);
  const files = new DesktopFileAccess(call);
  const shell = new DesktopShell(call);
  const bundled = new BundledFontSource(opts.baseUrl ?? import.meta.env?.BASE_URL ?? '/');
  const engine = new FallbackEngineHost(
    new NativeEngineHost(call),
    opts.wasmEngine ?? (() => new WebEngineHost(bundled)),
  );
  const locale = () =>
    (typeof document === 'undefined' ? '' : document.documentElement.lang) || 'vi-VN';
  return {
    kind: 'desktop',
    files,
    storage: opts.storage ? opts.storage(files) : new FileAccessStorage(files),
    clipboard: new BrowserClipboard(),
    fonts: new NativeFontSource(call),
    print: new DesktopPrint(call),
    engine,
    auth: new DesktopAuth({ scheme: opts.scheme ?? 'bluedoc', shell }),
    window: new DesktopWindow(call),
    offline: new DesktopDiskCache(call),
    shell,
    channel: (name: string) => new TauriBus(name),
    sheet: createFallbackSheetHost(
      call,
      async () => {
        await engine.init();
        return engine.backend === 'native';
      },
      opts.wasmSheet ?? (() => webSheetTransport(bundled, locale)),
    ),
    signing: new DesktopSigning(call),
    updates: new DesktopUpdates(call),
    // The webview (WebView2 / WebKitGTK) encodes when it has WebCodecs; else GIF / PNG only.
    video: new WebCodecsVideo(),
    // The webview's Web Speech API (WebView2: the Windows voices) for read aloud.
    speech: webSpeech(),
    // The webview's recogniser when it has one (WebView2 may not; CORE-LANG-002); the
    // dictation UI says so when it is missing or fails to start.
    ...((): Pick<Platform, 'dictation'> => {
      const dictation = webDictation();
      return dictation ? { dictation } : {};
    })(),
    // The webview's Handwriting Recognition API when it has one (CORE-INK-002); without it the
    // ink → text / math commands say they are unavailable.
    ...((): Pick<Platform, 'handwriting'> => {
      const handwriting = webHandwriting(
        globalThis,
        () => document.documentElement.lang || 'vi-VN',
      );
      return handwriting ? { handwriting } : {};
    })(),
    // Microphone / camera through getUserMedia + MediaRecorder (recorded shows, SLIDE-REC-001).
    media: webMediaCapture(),
    displays: new DesktopDisplays(call),
    images: new FetchRemoteImages(),
    dataSources: new DesktopQueryData(call),
  };
}
