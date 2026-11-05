import {
  AnonymousSession,
  BrowserClipboard,
  FileAccessStorage,
  IndexedDbCache,
  LocalRecentFiles,
} from '../shared';
import type {
  DocumentStorage,
  FileAccess,
  FontDescriptor,
  FontSource,
  OpenedFile,
  Platform,
  PrintService,
  ShellIntegration,
  WindowManager,
} from '../types';
import { WebEngineHost } from './WebEngineHost';

export { WebEngineHost };

/** Upload/download, with the File System Access API when available. */
export class WebFileAccess implements FileAccess {
  openFile(accept: string[]): Promise<OpenedFile | null> {
    return new Promise((resolve) => {
      const input = document.createElement('input');
      input.type = 'file';
      input.accept = accept.join(',');
      input.style.display = 'none';
      input.addEventListener('change', async () => {
        const file = input.files?.[0];
        input.remove();
        if (!file) return resolve(null);
        resolve({ name: file.name, bytes: new Uint8Array(await file.arrayBuffer()) });
      });
      input.addEventListener('cancel', () => {
        input.remove();
        resolve(null);
      });
      document.body.appendChild(input);
      input.click();
    });
  }

  async saveFile(bytes: Uint8Array, suggestedName: string, mime: string): Promise<string | null> {
    const blob = new Blob([bytes as BlobPart], { type: mime });
    const a = document.createElement('a');
    a.href = URL.createObjectURL(blob);
    a.download = suggestedName;
    document.body.appendChild(a);
    a.click();
    a.remove();
    setTimeout(() => URL.revokeObjectURL(a.href), 30_000);
    return suggestedName;
  }
}

/** Fonts shipped with the app under `<base>/fonts/` (see assets/fonts/manifest.json). */
export class BundledFontSource implements FontSource {
  constructor(private readonly baseUrl: string) {}
  async list(): Promise<FontDescriptor[]> {
    const res = await fetch(`${this.baseUrl}fonts/manifest.json`);
    if (!res.ok) throw new Error(`fonts.manifestMissing (${res.status})`);
    const m = (await res.json()) as { fonts: string[] };
    return m.fonts.map((file) => ({ family: file.split('-')[0] ?? file, file }));
  }
  async load(file: string): Promise<Uint8Array> {
    const res = await fetch(`${this.baseUrl}fonts/${file}`);
    if (!res.ok) throw new Error(`fonts.loadFailed: ${file}`);
    return new Uint8Array(await res.arrayBuffer());
  }
}

/** Web printing: open the engine-generated PDF and use the browser's print. */
export class WebPrint implements PrintService {
  async printPdf(pdf: Uint8Array, title: string): Promise<void> {
    const url = URL.createObjectURL(new Blob([pdf as BlobPart], { type: 'application/pdf' }));
    const frame = document.createElement('iframe');
    frame.style.cssText = 'position:fixed;width:0;height:0;border:0;right:0;bottom:0';
    frame.title = title;
    frame.src = url;
    document.body.appendChild(frame);
    frame.onload = () => {
      frame.contentWindow?.focus();
      frame.contentWindow?.print();
      setTimeout(() => {
        frame.remove();
        URL.revokeObjectURL(url);
      }, 60_000);
    };
  }
}

class WebWindow implements WindowManager {
  setTitle(title: string): void {
    document.title = title;
  }
  async openNew(url: string): Promise<void> {
    window.open(url, '_blank', 'noopener');
  }
}

class WebShell implements ShellIntegration {
  private recent = new LocalRecentFiles();
  async recentFiles() {
    return this.recent.list();
  }
  async addRecentFile(file: Parameters<ShellIntegration['addRecentFile']>[0]) {
    this.recent.add(file);
  }
  onOpenRequest(): () => void {
    return () => {};
  }
}

export interface WebPlatformOptions {
  /** Base URL where `fonts/` and `samples/` are served (default: Vite BASE_URL). */
  baseUrl?: string;
  storage?: (files: FileAccess) => DocumentStorage;
}

/** Build the web platform. */
export function createWebPlatform(opts: WebPlatformOptions = {}): Platform {
  const baseUrl = opts.baseUrl ?? import.meta.env.BASE_URL ?? '/';
  const files = new WebFileAccess();
  const fonts = new BundledFontSource(baseUrl);
  return {
    kind: 'web',
    files,
    storage: opts.storage ? opts.storage(files) : new FileAccessStorage(files),
    clipboard: new BrowserClipboard(),
    fonts,
    print: new WebPrint(),
    engine: new WebEngineHost(fonts),
    auth: new AnonymousSession(),
    window: new WebWindow(),
    offline: new IndexedDbCache(),
    shell: new WebShell(),
  };
}
