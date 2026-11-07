/**
 * Embedded mode (`/edit?host=…&file=…&embed=1`, prompt §6A.2): the editor runs
 * in an iframe of a host such as BlueDrive. The host hands over its access
 * token through `@blueoffice/editor-sdk` (never in the URL); this module
 * opens a BlueOffice server session with it (docs/bohp.md §B), loads the
 * file into the right editor and turns "save" into session checkpoints.
 */
import type { CommandHandler, HostConnection, InitPayload } from '@blueoffice/editor-sdk';
import type { OpenedFile, Platform } from '@blueoffice/platform';
import type { AppId } from './routes';

/** Subset of the server's `SessionInfo` used by the shell. */
export interface EmbedSession {
  sessionId: string;
  sessionToken: string;
  mode: 'edit' | 'comment' | 'view';
  file: { id: string; name: string; version?: string };
  allowedOrigins: string[];
  autosaveSeconds?: number;
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

export const appForFile = (name: string): AppId => {
  const ext = name.toLowerCase().split('.').pop() ?? '';
  if (['xlsx', 'xlsm', 'xlsb', 'xls', 'ods', 'csv'].includes(ext)) return 'sheet';
  if (['pptx', 'ppt', 'odp'].includes(ext)) return 'slide';
  return 'doc';
};

/** Creates the server session from the host token and downloads the file. */
export async function openSession(
  api: string,
  init: InitPayload,
  fetchImpl: FetchLike,
): Promise<{ session: EmbedSession; file: OpenedFile }> {
  const res = await fetchImpl(`${api}/api/sessions`, {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify({ host: init.host, fileId: init.fileId, token: init.accessToken }),
  });
  if (!res.ok) throw new Error(`session.create ${res.status}`);
  const session = (await res.json()) as EmbedSession;
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

/** Saves bytes as a new host version (`POST …/checkpoint?save=true`). */
export async function checkpoint(
  api: string,
  session: EmbedSession,
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
export async function currentBytes(platform: Platform, app: AppId): Promise<Uint8Array> {
  if (app === 'sheet') {
    if (!platform.sheet) throw new Error('engine.noSheet');
    return platform.sheet.save('xlsx', 0);
  }
  return app === 'slide' ? platform.engine.savePptx() : platform.engine.saveDocx();
}

const savedName = (name: string, app: AppId): string => {
  const base = name.replace(/\.[^.]+$/, '');
  return `${base}.${app === 'sheet' ? 'xlsx' : app === 'slide' ? 'pptx' : 'docx'}`;
};

/**
 * Platform for the embedded editor: the session file is "opened by the OS",
 * saving goes to the session instead of a download, and the host is told.
 */
export function embedPlatform(
  base: Platform,
  opts: {
    api: string;
    session: EmbedSession;
    file: OpenedFile;
    app: AppId;
    conn: HostConnection;
    fetchImpl: FetchLike;
  },
): Platform {
  const { api, session, file, app, conn, fetchImpl } = opts;
  const readOnly = session.mode !== 'edit';
  const save = async (bytes: Uint8Array): Promise<string | null> => {
    if (readOnly) throw new Error('session.readOnly');
    const result = await checkpoint(api, session, bytes, fetchImpl);
    conn.emit('saved', result);
    return savedName(file.name, app);
  };
  return {
    ...base,
    files: {
      openFile: base.files.openFile.bind(base.files),
      // Same-format saves go back to the host; other formats (PDF, CSV) download.
      saveFile: async (bytes, name, mime) =>
        name.toLowerCase().endsWith(savedName(file.name, app).split('.').pop() ?? '')
          ? save(bytes)
          : base.files.saveFile(bytes, name, mime),
    },
    shell: {
      ...base.shell,
      onOpenRequest: (handler) => {
        queueMicrotask(() => handler(file));
        return () => {};
      },
    },
  };
}

/** Answers host commands (`save`, `print`, `export-pdf`, `close`). */
export function commandHandler(platform: Platform, app: AppId, name: string): CommandHandler {
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
