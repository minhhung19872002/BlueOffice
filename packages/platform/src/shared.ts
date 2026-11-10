/** Implementations shared by web and desktop. */
import { variantMime } from './fileVariants';
import type {
  AuthSession,
  ClipboardAccess,
  ClipboardContent,
  DocumentRef,
  DocumentStorage,
  FileAccess,
  MessageBus,
  OfflineCache,
  RecentFile,
  StoredDocument,
  UpdateStatus,
  UserInfo,
} from './types';

/** i18n key (`updates.*`) and parameters describing an update check result. */
export function updateNotice(s: UpdateStatus): { key: string; params: Record<string, string> } {
  switch (s.state) {
    case 'available':
      return { key: 'updates.available', params: { version: s.version, current: s.current } };
    case 'upToDate':
      return { key: 'updates.upToDate', params: { current: s.current } };
    case 'failed':
      return { key: 'updates.failed', params: { message: s.message } };
    case 'notConfigured':
      return { key: 'updates.notConfigured', params: {} };
    default:
      return { key: 'updates.disabled', params: {} };
  }
}

/** Automatic check at start-up: at most once per `intervalMs` (default one day). */
export function shouldCheckForUpdates(
  lastCheck: number | null,
  now: number,
  intervalMs = 24 * 60 * 60 * 1000,
): boolean {
  if (lastCheck === null || !Number.isFinite(lastCheck)) return true;
  // A clock set back also allows a check.
  return now - lastCheck >= intervalMs || now < lastCheck;
}

/** Async Clipboard API (works in browsers and in the Tauri WebView). */
export class BrowserClipboard implements ClipboardAccess {
  readText(): Promise<string> {
    return navigator.clipboard.readText();
  }
  writeText(text: string): Promise<void> {
    return navigator.clipboard.writeText(text);
  }
  async readHtml(): Promise<string> {
    if (!navigator.clipboard.read) return '';
    for (const item of await navigator.clipboard.read()) {
      if (item.types.includes('text/html')) return (await item.getType('text/html')).text();
    }
    return '';
  }
  async writeHtml(html: string, text: string): Promise<void> {
    if (typeof ClipboardItem === 'undefined') return this.writeText(text);
    await navigator.clipboard.write([
      new ClipboardItem({
        'text/html': new Blob([html], { type: 'text/html' }),
        'text/plain': new Blob([text], { type: 'text/plain' }),
      }),
    ]);
  }
  async writeItems(items: { text: string; html?: string; png?: Uint8Array }): Promise<void> {
    if (typeof ClipboardItem === 'undefined') return this.writeText(items.text);
    const data: Record<string, Blob> = {
      'text/plain': new Blob([items.text], { type: 'text/plain' }),
    };
    if (items.html) data['text/html'] = new Blob([items.html], { type: 'text/html' });
    if (items.png) data['image/png'] = new Blob([items.png as BlobPart], { type: 'image/png' });
    try {
      await navigator.clipboard.write([new ClipboardItem(data)]);
    } catch {
      // Some hosts refuse pictures: keep the text at least.
      await this.writeHtml(items.html ?? items.text, items.text);
    }
  }
  async read(): Promise<ClipboardContent> {
    const out: ClipboardContent = {};
    for (const item of await navigator.clipboard.read()) {
      for (const type of item.types) {
        const blob = await item.getType(type);
        if (type === 'image/png' && !out.image) {
          out.image = new Uint8Array(await blob.arrayBuffer());
        } else if (type === 'text/html' && out.html === undefined) out.html = await blob.text();
        else if (type === 'text/plain' && out.text === undefined) out.text = await blob.text();
      }
    }
    return out;
  }
}

/** Phase 1: anonymous session. OIDC (redirect / PKCE + deep link) arrives in Phase 6. */
export class AnonymousSession implements AuthSession {
  readonly user: UserInfo | null = null;
  async getAccessToken(): Promise<string | null> {
    return null;
  }
  async signIn(): Promise<void> {
    throw new Error('auth.notAvailable');
  }
  async signOut(): Promise<void> {}
}

/** Tiny IndexedDB key/value store (falls back to memory when IndexedDB is missing). */
export class IndexedDbCache implements OfflineCache {
  private memory = new Map<string, Uint8Array>();
  private db: Promise<IDBDatabase> | null = null;

  constructor(private readonly dbName = 'blueoffice-offline') {}

  private open(): Promise<IDBDatabase> | null {
    if (typeof indexedDB === 'undefined') return null;
    this.db ??= new Promise((resolve, reject) => {
      const req = indexedDB.open(this.dbName, 1);
      req.onupgradeneeded = () => req.result.createObjectStore('kv');
      req.onsuccess = () => resolve(req.result);
      req.onerror = () => reject(req.error);
    });
    return this.db;
  }

  private async tx<T>(mode: IDBTransactionMode, f: (s: IDBObjectStore) => IDBRequest<T>) {
    const db = await this.open();
    if (!db) return null;
    return new Promise<T>((resolve, reject) => {
      const req = f(db.transaction('kv', mode).objectStore('kv'));
      req.onsuccess = () => resolve(req.result);
      req.onerror = () => reject(req.error);
    });
  }

  async get(key: string): Promise<Uint8Array | null> {
    if (!this.open()) return this.memory.get(key) ?? null;
    const v = await this.tx<Uint8Array | undefined>('readonly', (s) => s.get(key));
    return v ?? null;
  }
  async put(key: string, value: Uint8Array): Promise<void> {
    if (!this.open()) {
      this.memory.set(key, value);
      return;
    }
    await this.tx('readwrite', (s) => s.put(value, key));
  }
  async delete(key: string): Promise<void> {
    if (!this.open()) {
      this.memory.delete(key);
      return;
    }
    await this.tx('readwrite', (s) => s.delete(key));
  }
}

/** Recent files kept in localStorage (per origin / per desktop install). */
export class LocalRecentFiles {
  constructor(private readonly key = 'blueoffice.recent') {}
  list(): RecentFile[] {
    try {
      const raw = globalThis.localStorage?.getItem(this.key);
      return raw ? (JSON.parse(raw) as RecentFile[]) : [];
    } catch {
      return [];
    }
  }
  add(file: RecentFile): void {
    const next = [file, ...this.list().filter((f) => f.path !== file.path)].slice(0, 20);
    try {
      globalThis.localStorage?.setItem(this.key, JSON.stringify(next));
    } catch {
      /* storage unavailable: ignore */
    }
  }
}

/**
 * Placeholder storage used until the host injects a real provider
 * (e.g. `LocalStorageProvider` from `@blueoffice/storage-providers`).
 */
export class FileAccessStorage implements DocumentStorage {
  readonly id = 'local';
  constructor(
    private readonly files: FileAccess,
    private readonly accept: string[] = [
      '.docx',
      '.docm',
      '.dotx',
      '.dotm',
      '.doc',
      '.odt',
      '.txt',
      '.md',
      // Web pages and Flat OPC (CORE-FMT-003).
      '.html',
      '.htm',
      '.mht',
      '.mhtml',
      '.xml',
    ],
  ) {}
  async open(_ref?: DocumentRef): Promise<StoredDocument | null> {
    const f = await this.files.openFile(this.accept);
    if (!f) return null;
    return {
      ref: { provider: this.id, id: f.path ?? f.name, name: f.name },
      bytes: f.bytes,
      permissions: { edit: true, download: true, share: false },
    };
  }
  async save(doc: StoredDocument, bytes: Uint8Array): Promise<StoredDocument> {
    const saved = await this.files.saveFile(bytes, doc.ref.name, mimeFor(doc.ref.name));
    return { ...doc, bytes, ref: { ...doc.ref, id: saved ?? doc.ref.id } };
  }
  async saveAs(name: string, bytes: Uint8Array): Promise<StoredDocument | null> {
    const saved = await this.files.saveFile(bytes, name, mimeFor(name));
    if (!saved) return null;
    return {
      ref: { provider: this.id, id: saved, name },
      bytes,
      permissions: { edit: true, download: true, share: false },
    };
  }
}

export function mimeFor(name: string): string {
  const ext = name.toLowerCase().split('.').pop();
  // Documents, templates, macro-enabled variants, slide shows, theme files (CORE-FMT-001).
  const variant = variantMime(name);
  if (variant) return variant;
  switch (ext) {
    case 'pdf':
      return 'application/pdf';
    case 'png':
      return 'image/png';
    default:
      return 'application/octet-stream';
  }
}

/** Window channel over `BroadcastChannel` (same-origin windows and tabs). */
export class BroadcastBus implements MessageBus {
  private readonly ch: BroadcastChannel;
  constructor(name: string) {
    this.ch = new BroadcastChannel(`blueoffice:${name}`);
  }
  post(msg: unknown): void {
    this.ch.postMessage(msg);
  }
  onMessage(handler: (msg: unknown) => void): () => void {
    const f = (e: MessageEvent) => handler(e.data);
    this.ch.addEventListener('message', f);
    return () => this.ch.removeEventListener('message', f);
  }
  close(): void {
    this.ch.close();
  }
}
