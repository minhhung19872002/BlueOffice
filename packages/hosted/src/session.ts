/**
 * A file opened from a host (BlueDrive, …) through a BlueOffice server
 * session (docs/bohp.md §B): the host token creates the session, the file is
 * downloaded from it, and "save" becomes a session checkpoint (a new host
 * version). Used by the web embed (`/edit`, token via `@blueoffice/editor-sdk`)
 * and by the desktop deep link (`bluedoc://open?host=…&file=…&token=…`).
 */
import type { DocumentStorage, OpenedFile, Platform, StoredDocument } from '@blueoffice/platform';
import type { SessionMode } from './mode';
import { OfflineError, reach, type OfflineSaves } from './offline';

/** `DocumentStorage.id` / `DocumentRef.provider` of the session file. */
export const HOSTED_STORAGE = 'blueoffice-session';

export type HostedApp = 'doc' | 'sheet' | 'slide';

/** Subset of the server's `SessionInfo` used by the shells. */
export interface HostedSession {
  sessionId: string;
  sessionToken: string;
  mode: 'edit' | 'comment' | 'view';
  file: { id: string; name: string; version?: string };
  user?: { id: string; name: string; avatarUrl?: string | null };
  allowedOrigins: string[];
  autosaveSeconds?: number;
  /** WebSocket relay for live collaboration (docs/bohp.md §C), relative to the server. */
  collab?: { url: string; protocol?: string };
}

/** What opens a session: the host's name, its file id and the user's host token. */
export interface HostFileRef {
  host: string;
  fileId: string;
  accessToken: string;
}

export type FetchLike = (
  url: string,
  init?: { method?: string; headers?: Record<string, string>; body?: BodyInit },
) => Promise<{
  ok: boolean;
  status: number;
  json(): Promise<unknown>;
  arrayBuffer(): Promise<ArrayBuffer>;
}>;

export const appForFile = (name: string): HostedApp => {
  const ext = name.toLowerCase().split('.').pop() ?? '';
  if (['xlsx', 'xlsm', 'xlsb', 'xls', 'ods', 'csv'].includes(ext)) return 'sheet';
  if (['pptx', 'ppt', 'odp'].includes(ext)) return 'slide';
  return 'doc';
};

/** `""` (same origin) or an absolute server URL without the trailing slash. */
export const serverBase = (url: string): string => url.trim().replace(/\/+$/, '');

/** Creates the server session from the host token and downloads the file. */
export async function openSession(
  api: string,
  ref: HostFileRef,
  fetchImpl: FetchLike,
): Promise<{ session: HostedSession; file: OpenedFile }> {
  const res = await reach(() =>
    fetchImpl(`${api}/api/sessions`, {
      method: 'POST',
      headers: { 'content-type': 'application/json' },
      body: JSON.stringify({ host: ref.host, fileId: ref.fileId, token: ref.accessToken }),
    }),
  );
  if (!res.ok) throw new Error(`session.create ${res.status}`);
  const session = (await res.json()) as HostedSession;
  const contents = await fetchImpl(
    `${api}/api/sessions/${encodeURIComponent(session.sessionId)}/contents`,
    {
      headers: { 'x-session-token': session.sessionToken },
    },
  );
  if (!contents.ok) throw new Error(`session.contents ${contents.status}`);
  const bytes = new Uint8Array(await contents.arrayBuffer());
  return { session, file: { name: session.file.name, bytes } };
}

/**
 * Origins allowed to embed the editor (`GET /api/hosting/origins`, the
 * `AllowedOrigins` of the configured hosts), used before the host is known.
 */
export async function fetchAllowedOrigins(api: string, fetchImpl: FetchLike): Promise<string[]> {
  const res = await fetchImpl(`${api}/api/hosting/origins`);
  if (!res.ok) throw new Error(`hosting.origins ${res.status}`);
  const body = (await res.json()) as { origins?: unknown };
  return Array.isArray(body.origins)
    ? body.origins.filter((o): o is string => typeof o === 'string' && o !== '*')
    : [];
}

/**
 * Saves bytes as a new host version (`POST …/checkpoint?save=true`); throws
 * {@link OfflineError} when the server cannot be reached.
 */
export async function checkpoint(
  api: string,
  session: HostedSession,
  bytes: Uint8Array,
  fetchImpl: FetchLike,
): Promise<unknown> {
  const res = await reach(() =>
    fetchImpl(`${api}/api/sessions/${encodeURIComponent(session.sessionId)}/checkpoint?save=true`, {
      method: 'POST',
      headers: {
        'x-session-token': session.sessionToken,
        'content-type': 'application/octet-stream',
      },
      body: bytes as BodyInit,
    }),
  );
  if (!res.ok) throw new Error(`session.save ${res.status}`);
  return res.json().catch(() => null);
}

/** Editor actions reported to the host's audit log (docs/bohp.md §B `…/audit`). */
export type AuditAction = 'print' | 'export' | 'download';

/** Best effort: an audit report never blocks or fails the user's action. */
export async function reportAudit(
  api: string,
  session: HostedSession,
  action: AuditAction,
  detail: string,
  fetchImpl: FetchLike,
): Promise<void> {
  try {
    await fetchImpl(`${api}/api/sessions/${encodeURIComponent(session.sessionId)}/audit`, {
      method: 'POST',
      headers: { 'x-session-token': session.sessionToken, 'content-type': 'application/json' },
      body: JSON.stringify({ action, detail }),
    });
  } catch {
    // Offline: the action itself already happened locally.
  }
}

/** Offline behaviour of a hosted platform (desktop). */
export interface HostedOffline {
  saves: OfflineSaves;
  host: string;
  /** The live connection is down (saves go to the offline cache without trying). */
  isOffline: () => boolean;
  /** A save was kept locally. */
  onSavedOffline?: () => void;
}

/** Bytes of the open file in its own format, from the engine of `app`. */
export async function currentBytes(platform: Platform, app: HostedApp): Promise<Uint8Array> {
  if (app === 'sheet') {
    if (!platform.sheet) throw new Error('engine.noSheet');
    return platform.sheet.save('xlsx', 0);
  }
  return app === 'slide' ? platform.engine.savePptx() : platform.engine.saveDocx();
}

export const savedName = (name: string, app: HostedApp): string => {
  const base = name.replace(/\.[^.]+$/, '');
  return `${base}.${app === 'sheet' ? 'xlsx' : app === 'slide' ? 'pptx' : 'docx'}`;
};

/**
 * Platform for a hosted file: the session file is "opened by the OS", saving
 * in the file's own format goes to the session instead of the disk / a
 * download, and `onSaved` hears about each new version.
 */
export function hostedPlatform(
  base: Platform,
  opts: {
    api: string;
    session: HostedSession;
    file: OpenedFile;
    app: HostedApp;
    mode: SessionMode;
    fetchImpl: FetchLike;
    onSaved?: (result: unknown) => void;
    /** Keep saves locally while the server is unreachable (desktop). */
    offline?: HostedOffline;
  },
): Platform {
  const { api, session, file, app, mode, fetchImpl, offline } = opts;
  // Host version the editor content is based on (offline saves sync against it).
  let baseVersion = session.file.version ?? '';
  const keepOffline = async (bytes: Uint8Array): Promise<unknown> => {
    if (!offline) throw new OfflineError(null);
    await offline.saves.put(
      {
        host: offline.host,
        fileId: session.file.id,
        name: file.name,
        baseVersion,
        savedAt: Date.now(),
      },
      bytes,
    );
    offline.onSavedOffline?.();
    return { outcome: 'offline' };
  };
  const saveVersion = async (bytes: Uint8Array): Promise<unknown> => {
    // The mode can change during the session (edit permission revoked).
    if (!mode.editable) throw new Error('session.readOnly');
    if (offline?.isOffline()) return keepOffline(bytes);
    let result: unknown;
    try {
      result = await checkpoint(api, session, bytes, fetchImpl);
    } catch (e) {
      if (e instanceof OfflineError && offline) return keepOffline(bytes);
      throw e;
    }
    const v = (result as { version?: unknown } | null)?.version;
    if (typeof v === 'string') baseVersion = v;
    opts.onSaved?.(result);
    return result;
  };
  const audit = (action: AuditAction, detail: string) =>
    void reportAudit(api, session, action, detail, fetchImpl);
  const save = async (bytes: Uint8Array): Promise<string | null> => {
    await saveVersion(bytes);
    return savedName(file.name, app);
  };
  const ownExt = savedName(file.name, app).split('.').pop() ?? '';
  const ownName = savedName(file.name, app);
  const version = (r: unknown): string | undefined => {
    const v = (r as { version?: unknown } | null)?.version;
    return typeof v === 'string' ? v : undefined;
  };
  const stored = (bytes: Uint8Array, v: string | undefined): StoredDocument => ({
    ref: { provider: HOSTED_STORAGE, id: session.file.id, name: ownName },
    bytes,
    ...(v ? { version: v } : {}),
    permissions: { edit: mode.editable, download: true, share: false },
  });
  // The editors save through `storage` (Ctrl+S, Lưu): the session file goes back to the host.
  const storage: DocumentStorage = {
    id: HOSTED_STORAGE,
    open: (ref) => base.storage.open(ref),
    save: async (doc, bytes) =>
      doc.ref.provider === HOSTED_STORAGE || doc.ref.name === ownName
        ? stored(bytes, version(await saveVersion(bytes)))
        : base.storage.save(doc, bytes),
    saveAs: async (name, bytes) =>
      name === ownName
        ? stored(bytes, version(await saveVersion(bytes)))
        : base.storage.saveAs(name, bytes),
  };
  return {
    ...base,
    storage,
    files: {
      openFile: base.files.openFile.bind(base.files),
      // Same-format saves go back to the host; other formats (PDF, CSV) go to the disk.
      saveFile: async (bytes, name, mime) => {
        if (name.toLowerCase().endsWith(`.${ownExt}`)) return save(bytes);
        const saved = await base.files.saveFile(bytes, name, mime);
        // Exports leave the host: the host's audit log hears about them (prompt §6A.6).
        if (saved !== null) audit('export', name.split('.').pop()?.toLowerCase() ?? '');
        return saved;
      },
    },
    print: {
      printPdf: async (pdf, title) => {
        await base.print.printPdf(pdf, title);
        audit('print', title);
      },
    },
    // Class instances: copy the methods explicitly (a spread only copies own fields).
    shell: {
      recentFiles: base.shell.recentFiles.bind(base.shell),
      addRecentFile: base.shell.addRecentFile.bind(base.shell),
      ...(base.shell.onOpenLink ? { onOpenLink: base.shell.onOpenLink.bind(base.shell) } : {}),
      onOpenRequest: (handler) => {
        queueMicrotask(() => handler(file));
        return () => {};
      },
    },
  };
}

export type HostCommand = 'save' | 'print' | 'export-pdf' | 'close';

/** Answers host commands (`save`, `print`, `export-pdf`, `close`). */
export function commandHandler(
  platform: Platform,
  app: HostedApp,
  name: string,
): (cmd: HostCommand) => Promise<unknown> {
  return async (cmd) => {
    const pdf = () =>
      app === 'sheet' && platform.sheet ? platform.sheet.exportPdf() : platform.engine.exportPdf();
    switch (cmd) {
      case 'save':
        await platform.files.saveFile(await currentBytes(platform, app), savedName(name, app), '');
        return { saved: true };
      case 'print':
        await platform.print.printPdf(await pdf(), name);
        return null;
      case 'export-pdf':
        await platform.files.saveFile(
          await pdf(),
          name.replace(/\.[^.]+$/, '') + '.pdf',
          'application/pdf',
        );
        return null;
      case 'close':
        return null;
    }
  };
}
