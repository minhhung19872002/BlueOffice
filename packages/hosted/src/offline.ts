/**
 * Offline saves of host files (prompt §6A.3 "Sửa offline"): saving while the
 * server cannot be reached keeps the file in the platform's offline cache
 * (desktop: local cache folder) with the version it was based on. The next
 * time the file is opened with a session the copy goes to
 * `POST /api/sessions/{id}/sync?baseVersion=` (docs/bohp.md §D): saved as a
 * new version when nobody changed the file meanwhile, else a
 * "(xung đột - <người dùng> - <thời gian>)" copy next to it — never an overwrite.
 * When the live connection comes back in the same session the CRDT replays the
 * edits instead, and the copy is dropped.
 */
import type { DocumentStorage, OfflineCache, Platform, StoredDocument } from '@blueoffice/platform';
import type { FetchLike, HostedSession } from './session';

/** Metadata of a save kept while offline. */
export interface PendingSave {
  host: string;
  fileId: string;
  name: string;
  /** Host version the edits were made on. */
  baseVersion: string;
  savedAt: number;
}

export interface SyncOutcome {
  outcome: 'saved' | 'conflict-copy';
  version?: string;
  conflictCopy?: { id: string; name: string; version?: string } | null;
}

/** The server could not be reached (network down, DNS, connection refused). */
export class OfflineError extends Error {
  constructor(cause: unknown) {
    super('session.offline', { cause });
  }
}

/** `fetch` rejects (instead of resolving with a status) only when the request never got an answer. */
export async function reach<T>(call: () => Promise<T>): Promise<T> {
  try {
    return await call();
  } catch (e) {
    throw new OfflineError(e);
  }
}

const enc = new TextEncoder();
const dec = new TextDecoder();
const safe = (s: string) => encodeURIComponent(s);

/** Pending saves per host file in an {@link OfflineCache}. */
export class OfflineSaves {
  constructor(private readonly cache: OfflineCache) {}

  private key(host: string, fileId: string): string {
    return `hosted.pending.${safe(host)}.${safe(fileId)}`;
  }

  async put(meta: PendingSave, bytes: Uint8Array): Promise<void> {
    const k = this.key(meta.host, meta.fileId);
    await this.cache.put(k, bytes);
    await this.cache.put(`${k}.meta`, enc.encode(JSON.stringify(meta)));
  }

  async get(
    host: string,
    fileId: string,
  ): Promise<{ meta: PendingSave; bytes: Uint8Array } | null> {
    const k = this.key(host, fileId);
    const [m, bytes] = await Promise.all([this.cache.get(`${k}.meta`), this.cache.get(k)]);
    if (!m || !bytes) return null;
    try {
      const meta = JSON.parse(dec.decode(m)) as Partial<PendingSave>;
      if (typeof meta.baseVersion !== 'string' || typeof meta.name !== 'string') return null;
      return {
        meta: {
          host,
          fileId,
          name: meta.name,
          baseVersion: meta.baseVersion,
          savedAt: typeof meta.savedAt === 'number' ? meta.savedAt : 0,
        },
        bytes,
      };
    } catch {
      return null;
    }
  }

  async clear(host: string, fileId: string): Promise<void> {
    const k = this.key(host, fileId);
    await Promise.all([this.cache.delete(k), this.cache.delete(`${k}.meta`)]);
  }

  private copyKey(host: string, fileId: string): string {
    return `hosted.copy.${safe(host)}.${safe(fileId)}`;
  }

  /** Keeps the last downloaded version (opened offline later). */
  async putCopy(meta: CachedCopy, bytes: Uint8Array): Promise<void> {
    const k = this.copyKey(meta.host, meta.fileId);
    await this.cache.put(k, bytes);
    await this.cache.put(`${k}.meta`, enc.encode(JSON.stringify(meta)));
  }

  /** The newest local content of a host file: a pending save, else the cached download. */
  async getCopy(
    host: string,
    fileId: string,
  ): Promise<{ meta: CachedCopy; bytes: Uint8Array } | null> {
    const pending = await this.get(host, fileId);
    if (pending) {
      const { name, baseVersion } = pending.meta;
      return { meta: { host, fileId, name, version: baseVersion }, bytes: pending.bytes };
    }
    const k = this.copyKey(host, fileId);
    const [m, bytes] = await Promise.all([this.cache.get(`${k}.meta`), this.cache.get(k)]);
    if (!m || !bytes) return null;
    try {
      const meta = JSON.parse(dec.decode(m)) as Partial<CachedCopy>;
      return typeof meta.name === 'string' && typeof meta.version === 'string'
        ? { meta: { host, fileId, name: meta.name, version: meta.version }, bytes }
        : null;
    } catch {
      return null;
    }
  }
}

/** Metadata of a cached download. */
export interface CachedCopy {
  host: string;
  fileId: string;
  name: string;
  version: string;
}

/**
 * Platform for a host file opened from the local cache while the server is
 * unreachable: the editor gets the cached bytes, and saving in the file's own
 * format becomes a pending offline save (synced the next time the file is
 * opened online). Other formats (PDF export) still go to the disk.
 */
export function offlinePlatform(
  base: Platform,
  saves: OfflineSaves,
  copy: { meta: CachedCopy; bytes: Uint8Array },
  onSavedOffline?: () => void,
): Platform {
  const { meta } = copy;
  const ext = meta.name.split('.').pop()?.toLowerCase() ?? '';
  const keep = async (bytes: Uint8Array) => {
    await saves.put(
      {
        host: meta.host,
        fileId: meta.fileId,
        name: meta.name,
        baseVersion: meta.version,
        savedAt: Date.now(),
      },
      bytes,
    );
    onSavedOffline?.();
  };
  const own = (name: string) => name === meta.name || name.toLowerCase().endsWith(`.${ext}`);
  const stored = (bytes: Uint8Array): StoredDocument => ({
    ref: { provider: OFFLINE_STORAGE, id: meta.fileId, name: meta.name },
    bytes,
    version: meta.version,
    permissions: { edit: true, download: true, share: false },
  });
  const keepStored = async (bytes: Uint8Array) => {
    await keep(bytes);
    return stored(bytes);
  };
  const storage: DocumentStorage = {
    id: OFFLINE_STORAGE,
    open: (ref) => base.storage.open(ref),
    save: (doc, bytes) =>
      doc.ref.provider === OFFLINE_STORAGE || own(doc.ref.name)
        ? keepStored(bytes)
        : base.storage.save(doc, bytes),
    saveAs: (name, bytes) => (own(name) ? keepStored(bytes) : base.storage.saveAs(name, bytes)),
  };
  return {
    ...base,
    storage,
    files: {
      openFile: base.files.openFile.bind(base.files),
      saveFile: async (bytes, name, mime) => {
        if (!own(name)) return base.files.saveFile(bytes, name, mime);
        await keep(bytes);
        return meta.name;
      },
    },
    shell: {
      recentFiles: base.shell.recentFiles.bind(base.shell),
      addRecentFile: base.shell.addRecentFile.bind(base.shell),
      onOpenRequest: (handler) => {
        queueMicrotask(() => handler({ name: meta.name, bytes: copy.bytes }));
        return () => {};
      },
    },
  };
}

/** `DocumentStorage.id` of a host file edited offline. */
export const OFFLINE_STORAGE = 'blueoffice-offline';

/** Uploads an offline save through the session (`POST …/sync?baseVersion=`). */
export async function syncPendingSave(
  api: string,
  session: HostedSession,
  pending: { meta: PendingSave; bytes: Uint8Array },
  fetchImpl: FetchLike,
): Promise<SyncOutcome> {
  const res = await reach(() =>
    fetchImpl(
      `${api}/api/sessions/${encodeURIComponent(session.sessionId)}/sync?baseVersion=${encodeURIComponent(pending.meta.baseVersion)}`,
      {
        method: 'POST',
        headers: {
          'x-session-token': session.sessionToken,
          'content-type': 'application/octet-stream',
        },
        body: pending.bytes as BodyInit,
      },
    ),
  );
  if (!res.ok) throw new Error(`session.sync ${res.status}`);
  const body = (await res.json()) as Partial<SyncOutcome>;
  return {
    outcome: body.outcome === 'saved' ? 'saved' : 'conflict-copy',
    ...(typeof body.version === 'string' ? { version: body.version } : {}),
    conflictCopy: body.conflictCopy ?? null,
  };
}
