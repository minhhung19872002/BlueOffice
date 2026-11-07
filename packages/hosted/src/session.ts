/**
 * A file opened from a host (BlueDrive, …) through a BlueOffice server
 * session (docs/bohp.md §B): the host token creates the session, the file is
 * downloaded from it, and "save" becomes a session checkpoint (a new host
 * version). Used by the web embed (`/edit`, token via `@blueoffice/editor-sdk`)
 * and by the desktop deep link (`bluedoc://open?host=…&file=…&token=…`).
 */
import type { OpenedFile, Platform } from '@blueoffice/platform';
import type { SessionMode } from './mode';

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
  const res = await fetchImpl(`${api}/api/sessions`, {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify({ host: ref.host, fileId: ref.fileId, token: ref.accessToken }),
  });
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

/** Saves bytes as a new host version (`POST …/checkpoint?save=true`). */
export async function checkpoint(
  api: string,
  session: HostedSession,
  bytes: Uint8Array,
  fetchImpl: FetchLike,
): Promise<unknown> {
  const res = await fetchImpl(
    `${api}/api/sessions/${encodeURIComponent(session.sessionId)}/checkpoint?save=true`,
    {
      method: 'POST',
      headers: {
        'x-session-token': session.sessionToken,
        'content-type': 'application/octet-stream',
      },
      body: bytes as BodyInit,
    },
  );
  if (!res.ok) throw new Error(`session.save ${res.status}`);
  return res.json().catch(() => null);
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
  },
): Platform {
  const { api, session, file, app, mode, fetchImpl } = opts;
  const save = async (bytes: Uint8Array): Promise<string | null> => {
    // The mode can change during the session (edit permission revoked).
    if (!mode.editable) throw new Error('session.readOnly');
    const result = await checkpoint(api, session, bytes, fetchImpl);
    opts.onSaved?.(result);
    return savedName(file.name, app);
  };
  const ownExt = savedName(file.name, app).split('.').pop() ?? '';
  return {
    ...base,
    files: {
      openFile: base.files.openFile.bind(base.files),
      // Same-format saves go back to the host; other formats (PDF, CSV) go to the disk.
      saveFile: async (bytes, name, mime) =>
        name.toLowerCase().endsWith(`.${ownExt}`)
          ? save(bytes)
          : base.files.saveFile(bytes, name, mime),
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
