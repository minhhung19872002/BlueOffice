/** Implementations shared by web and desktop. */
import type {
  AuthSession,
  ClipboardAccess,
  DocumentRef,
  DocumentStorage,
  FileAccess,
  OfflineCache,
  RecentFile,
  StoredDocument,
  UserInfo,
} from './types';

/** Async Clipboard API (works in browsers and in the Tauri WebView). */
export class BrowserClipboard implements ClipboardAccess {
  readText(): Promise<string> {
    return navigator.clipboard.readText();
  }
  writeText(text: string): Promise<void> {
    return navigator.clipboard.writeText(text);
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
    private readonly accept: string[] = ['.docx', '.txt', '.md'],
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
  switch (ext) {
    case 'docx':
      return 'application/vnd.openxmlformats-officedocument.wordprocessingml.document';
    case 'pdf':
      return 'application/pdf';
    case 'png':
      return 'image/png';
    default:
      return 'application/octet-stream';
  }
}
