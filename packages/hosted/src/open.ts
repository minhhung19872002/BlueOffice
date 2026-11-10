/**
 * The whole "open a host file" flow shared by the web embed and the desktop
 * deep link: server session → download → hosted platform (saves become host
 * versions) → collaboration room → view-mode guard (outermost).
 */
import type { CloseReason, CollabClient, SocketFactory } from '@blueoffice/collab';
import type { Platform } from '@blueoffice/platform';
import { loadWasmReplica, startCollab, type Replica } from './collab';
import { DirtyState, trackDirty } from './dirty';
import { HostedStatus, SessionMode, parseMode, type SessionModeValue } from './mode';
import { syncPendingSave, type OfflineSaves, type SyncOutcome } from './offline';
import { guardPlatform } from './readonly';
import {
  appForFile,
  commandHandler,
  hostedPlatform,
  openSession,
  type FetchLike,
  type HostCommand,
  type HostFileRef,
  type HostedOffline,
  type HostedApp,
  type HostedSession,
} from './session';
import type { OpenedFile } from '@blueoffice/platform';

export interface OpenHostedOptions {
  /** The shell's platform (web or desktop). */
  platform: Platform;
  /** Server base: `""` for same origin, else e.g. `https://office.example.vn`. */
  api: string;
  /** What the relative `collab.url` resolves against (`api`, or the page URL when same origin). */
  base: string;
  ref: HostFileRef;
  /**
   * Mode the host asked for (`BlueOffice.open({ mode })`); the session never
   * goes above it nor above the user's permissions (default `edit`).
   */
  requestedMode?: SessionModeValue;
  fetchImpl: FetchLike;
  /** Replica factory (default: the WASM `bo-collab`). */
  replica?: () => Promise<Replica>;
  socket?: SocketFactory;
  onSaved?: (result: unknown) => void;
  /** A save failed (not offline: the server answered with an error). */
  onSaveError?: (e: unknown) => void;
  onCollaborators?: (collaborators: unknown) => void;
  onEnded?: (r: CloseReason) => void;
  /**
   * Offline saves (desktop): saving while the server is unreachable keeps the
   * file locally; a copy left from an earlier session is synced when the file
   * is opened again (`…/sync`, possibly a conflict copy).
   */
  offline?: OfflineSaves;
  /** A pending offline save was synced while opening. */
  onOfflineSync?: (r: SyncOutcome) => void;
  /** A save was kept locally because the server was unreachable. */
  onSavedOffline?: () => void;
}

export interface HostedEditor {
  app: HostedApp;
  session: HostedSession;
  file: OpenedFile;
  mode: SessionMode;
  status: HostedStatus;
  /** Unsaved local changes (SDK `dirty` event). */
  dirty: DirtyState;
  /** Render the editor with this platform. */
  platform: Platform;
  client: CollabClient | null;
  /** Host commands (`save`, `print`, `export-pdf`; `close` = {@link HostedEditor.close}). */
  command(cmd: HostCommand): Promise<unknown>;
  /**
   * Leaves the session: unsaved local changes are saved first as a new host
   * version (when the session is editable); resolves whether a save happened.
   */
  close(): Promise<{ saved: boolean }>;
}

export async function openHosted(o: OpenHostedOptions): Promise<HostedEditor> {
  let { session, file } = await openSession(o.api, o.ref, o.fetchImpl);
  const pending = o.offline ? await o.offline.get(o.ref.host, o.ref.fileId) : null;
  if (o.offline && pending) {
    // Edits saved offline in an earlier session: never overwrite a newer host version.
    const r = await syncPendingSave(o.api, session, pending, o.fetchImpl);
    await o.offline.clear(o.ref.host, o.ref.fileId);
    if (r.outcome === 'saved') {
      // The host now has the offline edits: open that version in a fresh session.
      await o
        .fetchImpl(`${o.api}/api/sessions/${encodeURIComponent(session.sessionId)}`, {
          method: 'DELETE',
          headers: { 'x-session-token': session.sessionToken },
        })
        .catch(() => null);
      ({ session, file } = await openSession(o.api, o.ref, o.fetchImpl));
    }
    o.onOfflineSync?.(r);
  }
  // Last downloaded version, for opening the file while offline.
  await o.offline
    ?.putCopy(
      {
        host: o.ref.host,
        fileId: o.ref.fileId,
        name: file.name,
        version: session.file.version ?? '',
      },
      file.bytes,
    )
    .catch(() => {});
  const app = appForFile(file.name);
  const mode = new SessionMode(parseMode(session.mode), o.requestedMode ?? 'edit');
  const status = new HostedStatus();
  const dirty = new DirtyState();
  // Offline = the live connection dropped after it had been up.
  let wasOnline = false;
  let wentOffline = false;
  const offline: HostedOffline | undefined = o.offline
    ? {
        saves: o.offline,
        host: o.ref.host,
        isOffline: () => wentOffline,
        ...(o.onSavedOffline ? { onSavedOffline: o.onSavedOffline } : {}),
      }
    : undefined;
  let platform = hostedPlatform(o.platform, {
    api: o.api,
    session,
    file,
    app,
    mode,
    fetchImpl: o.fetchImpl,
    onSaved: (r) => {
      dirty.set(false);
      o.onSaved?.(r);
    },
    ...(o.onSaveError ? { onSaveError: o.onSaveError } : {}),
    ...(offline ? { offline } : {}),
    // A restore resets the collaboration room (every participant reloads); alone, reload here.
    onRestored: () => {
      if (!client) o.onEnded?.('reload');
    },
  });
  // Commands run against the final platform (collaborative + view-mode guard).
  let final: Platform = platform;
  const command = (cmd: HostCommand) => commandHandler(final, app, file.name)(cmd);
  let client: CollabClient | null = null;
  if (session.collab?.url) {
    const h = startCollab({
      platform,
      app,
      session,
      base: o.base,
      replica: await (o.replica ?? loadWasmReplica)(),
      mode,
      saveNow: () => void command('save').catch(() => {}),
      onStatus: (s) => {
        if (s === 'online') {
          if (wentOffline && o.offline) {
            // Back online in the same session: the replica resends the offline edits
            // (CRDT); the local copy is not needed any more and a new version is written.
            void o.offline.clear(o.ref.host, o.ref.fileId);
            if (mode.editable) void command('save').catch(() => {});
          }
          wasOnline = true;
          wentOffline = false;
        } else if (s === 'offline' && wasOnline) {
          wentOffline = true;
        }
        status.setStatus(s);
      },
      onEnded: (r) => {
        status.setEnded(r);
        o.onEnded?.(r);
      },
      ...(o.socket ? { socket: o.socket } : {}),
    });
    if (o.onCollaborators) {
      const report = o.onCollaborators;
      h.client.on('control', (m) => {
        if (m.type === 'collaborators-changed') report(m.collaborators);
      });
    }
    client = h.client;
    platform = h.platform;
  }
  // Outermost: the view/comment guard, then dirty tracking of what really runs.
  final = guardPlatform(trackDirty(platform, dirty), mode);
  let closed: Promise<{ saved: boolean }> | null = null;
  const close = (): Promise<{ saved: boolean }> =>
    (closed ??= (async () => {
      let saved = false;
      if (dirty.dirty && mode.editable) {
        try {
          await command('save');
          saved = true;
        } catch (e) {
          o.onSaveError?.(e);
        }
      }
      client?.close();
      return { saved };
    })());
  return {
    app,
    session,
    file,
    mode,
    status,
    dirty,
    platform: final,
    client,
    command: (cmd) => (cmd === 'close' ? close() : command(cmd)),
    close,
  };
}
