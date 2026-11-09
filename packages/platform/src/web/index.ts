import {
  AnonymousSession,
  BroadcastBus,
  BrowserClipboard,
  FileAccessStorage,
  IndexedDbCache,
  LocalRecentFiles,
} from '../shared';
import type {
  AuthSession,
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
import { createWebSheetHost, webSheetTransport } from './WebSheetHost';
import { WebSigning } from './signing';
import { WebCodecsVideo } from '../webcodecs';
import { WebDisplays, presentationReceiverBus } from './displays';

export { WebSigning } from './signing';
export type { SignatureBackend, WebSigningOptions } from './signing';

export { WebEngineHost, createWebSheetHost, webSheetTransport };
export { createWebAuth, AUTH_CALLBACK_PATH } from './auth';
export { registerServiceWorker } from './pwa';

/** A file handle of the File System Access API (WICG spec; only the parts used here). */
export interface FsFileHandle {
  readonly name: string;
  getFile(): Promise<{ name: string; arrayBuffer(): Promise<ArrayBuffer> }>;
  createWritable(): Promise<{ write(data: Blob): Promise<void>; close(): Promise<void> }>;
  queryPermission?(d: { mode: 'readwrite' }): Promise<'granted' | 'denied' | 'prompt'>;
  requestPermission?(d: { mode: 'readwrite' }): Promise<'granted' | 'denied' | 'prompt'>;
}

interface PickerType {
  description: string;
  accept: Record<string, string[]>;
}

/** `window.showOpenFilePicker` / `showSaveFilePicker` (Chromium-based browsers). */
export interface FsPickers {
  showOpenFilePicker(o: { multiple?: boolean; types?: PickerType[] }): Promise<FsFileHandle[]>;
  showSaveFilePicker(o: { suggestedName?: string; types?: PickerType[] }): Promise<FsFileHandle>;
}

/** The pickers when the File System Access API is usable (not under automation: native dialogs cannot be driven). */
export function fileSystemAccess(w: unknown = globalThis): FsPickers | null {
  const g = w as Partial<FsPickers> & { navigator?: { webdriver?: boolean } };
  return typeof g.showOpenFilePicker === 'function' &&
    typeof g.showSaveFilePicker === 'function' &&
    !g.navigator?.webdriver
    ? (g as FsPickers)
    : null;
}

const isAbort = (e: unknown) => e instanceof Error && e.name === 'AbortError';

/** Picker filter for a file name ("Tài liệu (.docx)"). */
function typesFor(name: string, mime: string): PickerType[] {
  const ext = name.includes('.') ? `.${name.split('.').pop()!.toLowerCase()}` : '';
  return ext
    ? [
        {
          description: ext.slice(1).toUpperCase(),
          accept: { [mime || 'application/octet-stream']: [ext] },
        },
      ]
    : [];
}

/**
 * Opening and saving files on the web: the File System Access API when the
 * browser has it (open picker; Save writes back into the file that was
 * opened, other names ask with the save picker), else `<input type=file>`
 * and a download.
 */
export class WebFileAccess implements FileAccess {
  /** Handles of files opened with the picker, by name (Save writes back in place). */
  private readonly handles = new Map<string, FsFileHandle>();

  constructor(private readonly fs: FsPickers | null = fileSystemAccess()) {}

  async openFile(accept: string[]): Promise<OpenedFile | null> {
    if (this.fs) {
      try {
        const [h] = await this.fs.showOpenFilePicker({
          multiple: false,
          types: [
            {
              description: accept.join(', '),
              accept: { 'application/octet-stream': accept.filter((a) => a.startsWith('.')) },
            },
          ],
        });
        if (!h) return null;
        const file = await h.getFile();
        this.handles.set(file.name, h);
        return { name: file.name, bytes: new Uint8Array(await file.arrayBuffer()) };
      } catch (e) {
        if (isAbort(e)) return null;
        // Picker refused (e.g. cross-origin iframe): fall back to the input element.
      }
    }
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
    if (this.fs) {
      try {
        let h = this.handles.get(suggestedName);
        if (h?.requestPermission) {
          const p = (await h.queryPermission?.({ mode: 'readwrite' })) ?? 'prompt';
          if (p !== 'granted' && (await h.requestPermission({ mode: 'readwrite' })) !== 'granted')
            h = undefined;
        }
        h ??= await this.fs.showSaveFilePicker({
          suggestedName,
          types: typesFor(suggestedName, mime),
        });
        const w = await h.createWritable();
        await w.write(blob);
        await w.close();
        this.handles.set(h.name, h);
        return h.name;
      } catch (e) {
        if (isAbort(e)) return null;
        // Not allowed here: download instead.
      }
    }
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
  /** Signed-in session (OIDC, see {@link createWebAuth}); anonymous when absent. */
  auth?: AuthSession;
}

/** Build the web platform. */
export function createWebPlatform(opts: WebPlatformOptions = {}): Platform {
  const baseUrl = opts.baseUrl ?? import.meta.env.BASE_URL ?? '/';
  const files = new WebFileAccess();
  const fonts = new BundledFontSource(baseUrl);
  const engine = new WebEngineHost(fonts, undefined, async () => {
    const get = async (f: string) => {
      const res = await fetch(`${baseUrl}dictionaries/en_US/${f}`);
      if (!res.ok) throw new Error(`dictionary ${f}: ${res.status}`);
      return new Uint8Array(await res.arrayBuffer());
    };
    return [{ aff: await get('en_US.aff'), dic: await get('en_US.dic') }];
  });
  const env = import.meta.env as Record<string, string | undefined>;
  const signing = new WebSigning(engine, {
    defaults: {
      remoteUrl: env.VITE_BO_REMOTE_SIGN_URL ?? null,
      tsaUrl: env.VITE_BO_TSA_URL ?? null,
    },
    anchorsUrl: env.VITE_BO_TRUST_ANCHORS_URL ?? `${baseUrl}trust/anchors.pem`,
    // CRL / OCSP through the BlueOffice server (browsers cannot reach CAs: CORS).
    revocationProxy: env.VITE_BO_REVOCATION_PROXY_URL
      ? {
          url: env.VITE_BO_REVOCATION_PROXY_URL,
          auth: async () => {
            const token = await (opts.auth?.getAccessToken() ?? Promise.resolve(null));
            return token ? { headers: { Authorization: `Bearer ${token}` } } : {};
          },
        }
      : undefined,
  });
  return {
    kind: 'web',
    files,
    storage: opts.storage ? opts.storage(files) : new FileAccessStorage(files),
    clipboard: new BrowserClipboard(),
    fonts,
    print: new WebPrint(),
    engine,
    auth: opts.auth ?? new AnonymousSession(),
    window: new WebWindow(),
    offline: new IndexedDbCache(),
    shell: new WebShell(),
    // A page shown through the Presentation API talks over its presentation connection.
    channel: (name: string) => presentationReceiverBus() ?? new BroadcastBus(name),
    sheet: createWebSheetHost(fonts, () => document.documentElement.lang || 'vi-VN'),
    signing,
    video: new WebCodecsVideo(),
    displays: new WebDisplays(),
  };
}
