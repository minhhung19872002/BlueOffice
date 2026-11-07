/**
 * Desktop platform (Tauri 2). The engine runs natively in the Tauri process
 * (`apps/desktop/src-tauri`), exposing the same {@link EngineHost} API as the
 * WASM worker. Binary payloads use Tauri's raw IPC bodies (no JSON arrays).
 */
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import {
  AnonymousSession,
  BrowserClipboard,
  FileAccessStorage,
  IndexedDbCache,
  LocalRecentFiles,
} from '../shared';
import type {
  DocInfo,
  DocumentStorage,
  EditCommand,
  EditResult,
  EngineHost,
  FileAccess,
  FontDescriptor,
  FontSource,
  GlyphOutline,
  OpenedFile,
  PageDisplayList,
  Platform,
  PrintService,
  RecentFile,
  ShellIntegration,
  WindowManager,
} from '../types';

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
    await this.call<number>('engine_init');
  }
  open(bytes: Uint8Array, name: string): Promise<DocInfo> {
    return this.call<DocInfo>('engine_open', bytes, {
      headers: { 'x-file-name': encodeURIComponent(name) },
    });
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
  async command(cmd: EditCommand): Promise<EditResult> {
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
  async markSaved(): Promise<void> {
    await this.call<null>('engine_mark_saved');
  }
  dispose(): void {}
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

/** Phase 1: hand the PDF to the OS default viewer; native print dialog arrives in Phase 2. */
export class DesktopPrint implements PrintService {
  constructor(private readonly call: Invoke = invoke as Invoke) {}
  async printPdf(pdf: Uint8Array, title: string): Promise<void> {
    await this.call('print_pdf', pdf, { headers: { 'x-file-name': encodeURIComponent(title) } });
  }
}

class DesktopWindow implements WindowManager {
  constructor(private readonly call: Invoke) {}
  setTitle(title: string): void {
    document.title = title;
    void this.call('set_title', { title }).catch(() => {});
  }
  async openNew(url: string): Promise<void> {
    window.open(url, '_blank');
  }
}

class DesktopShell implements ShellIntegration {
  private recent = new LocalRecentFiles('blueoffice.desktop.recent');
  constructor(private readonly call: Invoke) {}
  async recentFiles(): Promise<RecentFile[]> {
    return this.recent.list();
  }
  async addRecentFile(file: RecentFile): Promise<void> {
    this.recent.add(file);
  }
  onOpenRequest(handler: (file: OpenedFile) => void): () => void {
    let disposed = false;
    const openPath = async (path: string) => {
      const bytes = toBytes(await this.call<ArrayBuffer>('read_file', { path }));
      if (!disposed) handler({ name: path.split(/[\\/]/).pop() ?? path, path, bytes });
    };
    // File passed on the command line (file association) …
    void this.call<string | null>('startup_file').then((p) => {
      if (p) void openPath(p);
    });
    // … or forwarded later by a second instance / deep link.
    const un = listen<string>('open-file', (e) => void openPath(e.payload));
    return () => {
      disposed = true;
      void un.then((f) => f());
    };
  }
}

export interface DesktopPlatformOptions {
  storage?: (files: FileAccess) => DocumentStorage;
  invoke?: Invoke;
}

/** Build the desktop platform. */
export function createDesktopPlatform(opts: DesktopPlatformOptions = {}): Platform {
  const call = opts.invoke ?? (invoke as Invoke);
  const files = new DesktopFileAccess(call);
  return {
    kind: 'desktop',
    files,
    storage: opts.storage ? opts.storage(files) : new FileAccessStorage(files),
    clipboard: new BrowserClipboard(),
    fonts: new NativeFontSource(call),
    print: new DesktopPrint(call),
    engine: new NativeEngineHost(call),
    auth: new AnonymousSession(),
    window: new DesktopWindow(call),
    offline: new IndexedDbCache('blueoffice-desktop-offline'),
    shell: new DesktopShell(call),
  };
}
