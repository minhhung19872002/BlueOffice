/**
 * Document sources behind a BlueOffice server (prompt §3.4 `storage`):
 *
 * - {@link BlueOfficeServerProvider} — the server's own file store
 *   (`/api/files`, Phase 6), signed in with OIDC (`AuthSession`);
 * - {@link BlueDriveProvider} — files of a BOHP host (BlueDrive) through the
 *   server: editor sessions (`/api/sessions`, docs/bohp.md §B) and the
 *   browsing proxy `/api/hosts/{host}/…` (§A2); the browser never calls the host;
 * - {@link WopiProvider} — files of a WOPI host ([MS-WOPI]) through a server
 *   session opened with `WOPISrc` + `access_token`.
 *
 * Saves use optimistic concurrency: a stale version answers 409, surfaced as
 * {@link StorageConflictError} (the caller decides: reload or save a copy).
 */
import type { AuthSession, DocumentRef, DocumentStorage, StoredDocument } from '@blueoffice/platform';

export interface ProviderResponse {
  ok: boolean;
  status: number;
  headers?: { get(name: string): string | null };
  json(): Promise<unknown>;
  arrayBuffer(): Promise<ArrayBuffer>;
}

export type ProviderFetch = (
  url: string,
  init?: { method?: string; headers?: Record<string, string>; body?: BodyInit },
) => Promise<ProviderResponse>;

const defaultFetch: ProviderFetch = (url, init) => globalThis.fetch(url, init);

/** The stored version changed since it was opened (HTTP 409). */
export class StorageConflictError extends Error {
  constructor(readonly currentVersion: string | null) {
    super('storage.conflict');
  }
}

/** The server refused the request (status, message). */
export class StorageError extends Error {
  constructor(
    readonly status: number,
    what: string,
  ) {
    super(`storage.${what} ${status}`);
  }
}

const enc = encodeURIComponent;
const trimBase = (api: string) => api.trim().replace(/\/+$/, '');
const bin = (b: Uint8Array) => b as BodyInit;

async function expectOk(res: ProviderResponse, what: string): Promise<ProviderResponse> {
  if (res.ok) return res;
  if (res.status === 409) {
    const body = (await res.json().catch(() => null)) as Record<string, unknown> | null;
    const v = body?.currentVersion ?? body?.version;
    throw new StorageConflictError(typeof v === 'string' || typeof v === 'number' ? String(v) : null);
  }
  throw new StorageError(res.status, what);
}

// ---------------------------------------------------------------- BlueOffice server files

/** `GET /api/files/{id}` (FileDto). */
export interface ServerFile {
  id: string;
  name: string;
  folderId: string | null;
  size: number;
  version: number;
  role: 'none' | 'viewer' | 'commenter' | 'editor' | 'owner';
}

export interface ServerFolder {
  id: string;
  name: string;
  parentId: string | null;
}

export interface BlueOfficeServerOptions {
  /** Server base (`""` = same origin). */
  api: string;
  /** Signed-in user (OIDC); every call carries its access token. */
  auth: Pick<AuthSession, 'getAccessToken'>;
  fetch?: ProviderFetch;
  /** Folder for new files (`saveAs`); `null` = the user's top level. */
  folderId?: string | null;
  /** Chooses a file when `open()` is called without a ref (file browser UI). */
  pick?: () => Promise<DocumentRef | null>;
}

/** Files stored by the BlueOffice server itself (`/api/files`, versions, `If-Match`). */
export class BlueOfficeServerProvider implements DocumentStorage {
  readonly id = 'blueoffice-server';
  private readonly api: string;
  private readonly fetch: ProviderFetch;

  constructor(private readonly o: BlueOfficeServerOptions) {
    this.api = trimBase(o.api);
    this.fetch = o.fetch ?? defaultFetch;
  }

  private async headers(extra: Record<string, string> = {}): Promise<Record<string, string>> {
    const token = await this.o.auth.getAccessToken();
    if (!token) throw new StorageError(401, 'signIn');
    return { authorization: `Bearer ${token}`, ...extra };
  }

  /** Top level (`folderId` absent) or one folder: sub-folders and files. */
  async list(folderId?: string): Promise<{ folders: ServerFolder[]; files: ServerFile[] }> {
    const url = folderId ? `${this.api}/api/folders/${enc(folderId)}` : `${this.api}/api/folders`;
    const res = await expectOk(await this.fetch(url, { headers: await this.headers() }), 'list');
    const b = (await res.json()) as { folders?: ServerFolder[]; files?: ServerFile[] };
    return { folders: b.folders ?? [], files: b.files ?? [] };
  }

  async open(ref?: DocumentRef): Promise<StoredDocument | null> {
    const r = ref ?? (await this.o.pick?.()) ?? null;
    if (!r) return null;
    const h = await this.headers();
    const meta = (await (
      await expectOk(await this.fetch(`${this.api}/api/files/${enc(r.id)}`, { headers: h }), 'open')
    ).json()) as ServerFile;
    const res = await expectOk(
      await this.fetch(`${this.api}/api/files/${enc(r.id)}/content`, { headers: h }),
      'open',
    );
    const etag = res.headers?.get('etag')?.replace(/^W\//, '').replace(/"/g, '');
    return this.stored(meta, new Uint8Array(await res.arrayBuffer()), etag || String(meta.version));
  }

  async save(doc: StoredDocument, bytes: Uint8Array): Promise<StoredDocument> {
    const h = await this.headers({
      'content-type': 'application/octet-stream',
      ...(doc.version ? { 'if-match': `"${doc.version}"` } : {}),
    });
    const res = await expectOk(
      await this.fetch(`${this.api}/api/files/${enc(doc.ref.id)}/content`, {
        method: 'PUT',
        headers: h,
        body: bin(bytes),
      }),
      'save',
    );
    return this.stored((await res.json()) as ServerFile, bytes);
  }

  async saveAs(name: string, bytes: Uint8Array): Promise<StoredDocument | null> {
    const q = new URLSearchParams({ name });
    if (this.o.folderId) q.set('folderId', this.o.folderId);
    const res = await expectOk(
      await this.fetch(`${this.api}/api/files?${q.toString()}`, {
        method: 'POST',
        headers: await this.headers({ 'content-type': 'application/octet-stream' }),
        body: bin(bytes),
      }),
      'saveAs',
    );
    return this.stored((await res.json()) as ServerFile, bytes);
  }

  private stored(f: ServerFile, bytes: Uint8Array, version = String(f.version)): StoredDocument {
    const role = String(f.role).toLowerCase();
    return {
      ref: { provider: this.id, id: String(f.id), name: f.name },
      bytes,
      version,
      permissions: {
        edit: role === 'editor' || role === 'owner',
        download: role !== 'none',
        share: role === 'owner',
      },
    };
  }
}

// ---------------------------------------------------------------- host files through server sessions

/** Subset of `SessionInfo` (docs/bohp.md §B). */
export interface ProviderSession {
  sessionId: string;
  sessionToken: string;
  mode: 'edit' | 'comment' | 'view';
  file: { id: string; name: string; version?: string | null; folderId?: string | null };
  permissions?: { edit?: boolean; download?: boolean; share?: boolean };
}

/**
 * A host file opened through a server editor session: `…/contents` to read,
 * `…/checkpoint?save=true` to write a new host version, `…/save-as` for a new
 * file next to it. Sessions are kept per file id until {@link close}.
 */
abstract class SessionProvider implements DocumentStorage {
  abstract readonly id: string;
  protected readonly api: string;
  protected readonly fetch: ProviderFetch;
  private readonly sessions = new Map<string, ProviderSession>();
  private last: string | null = null;

  constructor(api: string, fetchImpl: ProviderFetch | undefined) {
    this.api = trimBase(api);
    this.fetch = fetchImpl ?? defaultFetch;
  }

  /** Body of `POST /api/sessions` for `ref`. */
  protected abstract sessionRequest(ref: DocumentRef): Promise<Record<string, string>>;

  /** Chooses a file when `open()` has no ref. */
  protected pick(): Promise<DocumentRef | null> {
    return Promise.resolve(null);
  }

  /** The open session of a file (`ref.id`), e.g. for live collaboration. */
  session(id: string): ProviderSession | undefined {
    return this.sessions.get(id);
  }

  private url(s: ProviderSession, path: string): string {
    return `${this.api}/api/sessions/${enc(s.sessionId)}${path}`;
  }

  async open(ref?: DocumentRef): Promise<StoredDocument | null> {
    const r = ref ?? (await this.pick());
    if (!r) return null;
    const res = await expectOk(
      await this.fetch(`${this.api}/api/sessions`, {
        method: 'POST',
        headers: { 'content-type': 'application/json' },
        body: JSON.stringify(await this.sessionRequest(r)),
      }),
      'session',
    );
    const s = (await res.json()) as ProviderSession;
    this.sessions.set(r.id, s);
    this.last = r.id;
    const contents = await expectOk(
      await this.fetch(this.url(s, '/contents'), { headers: { 'x-session-token': s.sessionToken } }),
      'open',
    );
    return this.stored(r.id, s, new Uint8Array(await contents.arrayBuffer()), s.file.version ?? undefined);
  }

  async save(doc: StoredDocument, bytes: Uint8Array): Promise<StoredDocument> {
    const s = this.sessions.get(doc.ref.id);
    if (!s) throw new StorageError(404, 'noSession');
    const res = await expectOk(
      await this.fetch(this.url(s, '/checkpoint?save=true'), {
        method: 'POST',
        headers: { 'x-session-token': s.sessionToken, 'content-type': 'application/octet-stream' },
        body: bin(bytes),
      }),
      'save',
    );
    const b = (await res.json().catch(() => null)) as { version?: unknown } | null;
    const v = typeof b?.version === 'string' ? b.version : doc.version;
    if (v) s.file = { ...s.file, version: v };
    return { ...doc, bytes, ...(v ? { version: v } : {}) };
  }

  /** New file in the folder of the last opened file (`…/save-as`, Save As / BOHP `POST /folders/{id}/files`). */
  async saveAs(name: string, bytes: Uint8Array, folderId?: string): Promise<StoredDocument | null> {
    const s = this.last ? this.sessions.get(this.last) : undefined;
    if (!s) throw new StorageError(404, 'noSession');
    const q = new URLSearchParams({ name });
    const folder = folderId ?? s.file.folderId;
    if (folder) q.set('folderId', folder);
    const res = await expectOk(
      await this.fetch(this.url(s, `/save-as?${q.toString()}`), {
        method: 'POST',
        headers: { 'x-session-token': s.sessionToken, 'content-type': 'application/octet-stream' },
        body: bin(bytes),
      }),
      'saveAs',
    );
    const c = (await res.json()) as { id: string; name: string; version?: string | null };
    return {
      ref: { provider: this.id, id: c.id, name: c.name },
      bytes,
      ...(c.version ? { version: c.version } : {}),
      permissions: { edit: true, download: true, share: false },
    };
  }

  /** Ends the session of a file (the host lock is released when the room empties). */
  async close(id: string): Promise<void> {
    const s = this.sessions.get(id);
    if (!s) return;
    this.sessions.delete(id);
    if (this.last === id) this.last = null;
    await this.fetch(this.url(s, ''), {
      method: 'DELETE',
      headers: { 'x-session-token': s.sessionToken },
    }).catch(() => null);
  }

  private stored(id: string, s: ProviderSession, bytes: Uint8Array, version?: string): StoredDocument {
    const p = s.permissions ?? {};
    return {
      ref: { provider: this.id, id, name: s.file.name },
      bytes,
      ...(version ? { version } : {}),
      permissions: {
        edit: s.mode === 'edit' && p.edit !== false,
        download: p.download !== false,
        share: p.share === true,
      },
    };
  }
}

/** `Item` of the BOHP browsing proxy (docs/bohp.md §A2). */
export interface DriveItem {
  kind: 'folder' | 'file';
  id: string;
  name: string;
  size?: number;
  version?: string;
  lastModified?: string;
}

export interface BlueDriveOptions {
  api: string;
  /** Host name in the server configuration (`Hosts:N:Name`), default `bluedrive`. */
  host?: string;
  /** The user's host access token (BOHP JWT, e.g. from the host's sign-in). */
  token: () => Promise<string | null>;
  fetch?: ProviderFetch;
  pick?: () => Promise<DocumentRef | null>;
}

/** BlueDrive (BOHP) files through the BlueOffice server. */
export class BlueDriveProvider extends SessionProvider {
  readonly id = 'bluedrive';
  private readonly host: string;

  constructor(private readonly o: BlueDriveOptions) {
    super(o.api, o.fetch);
    this.host = o.host ?? 'bluedrive';
  }

  protected override pick(): Promise<DocumentRef | null> {
    return this.o.pick?.() ?? Promise.resolve(null);
  }

  protected override async sessionRequest(ref: DocumentRef): Promise<Record<string, string>> {
    return { host: this.host, fileId: ref.id, token: await this.requireToken() };
  }

  private async requireToken(): Promise<string> {
    const t = await this.o.token();
    if (!t) throw new StorageError(401, 'signIn');
    return t;
  }

  private async browse<T>(path: string): Promise<T> {
    const res = await expectOk(
      await this.fetch(`${this.api}/api/hosts/${enc(this.host)}${path}`, {
        headers: { authorization: `Bearer ${await this.requireToken()}` },
      }),
      'browse',
    );
    return (await res.json()) as T;
  }

  /** `GET …/folders/{id}/children` (`root` = the top folder). */
  list(folderId = 'root') {
    return this.browse<{ folder: { id: string; name: string; parentId: string | null }; items: DriveItem[] }>(
      `/folders/${enc(folderId)}/children`,
    );
  }

  async recent(limit = 20): Promise<DriveItem[]> {
    return (await this.browse<{ items: DriveItem[] }>(`/recent?limit=${limit}`)).items;
  }

  async shared(): Promise<DriveItem[]> {
    return (await this.browse<{ items: DriveItem[] }>('/shared')).items;
  }

  me() {
    return this.browse<{ id: string; name: string; avatarUrl?: string | null }>('/me');
  }
}

export interface WopiOptions {
  api: string;
  /** `access_token` of a file (`ref.id` is its `WOPISrc`), given by the WOPI host. */
  token: (wopiSrc: string) => Promise<string | null>;
  fetch?: ProviderFetch;
}

/** Files of a WOPI host ([MS-WOPI]); `DocumentRef.id` is the file's `WOPISrc` URL. */
export class WopiProvider extends SessionProvider {
  readonly id = 'wopi';

  constructor(private readonly o: WopiOptions) {
    super(o.api, o.fetch);
  }

  protected override async sessionRequest(ref: DocumentRef): Promise<Record<string, string>> {
    const token = await this.o.token(ref.id);
    if (!token) throw new StorageError(401, 'signIn');
    return { wopiSrc: ref.id, token };
  }
}
