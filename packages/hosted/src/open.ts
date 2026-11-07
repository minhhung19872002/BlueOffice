/**
 * The whole "open a host file" flow shared by the web embed and the desktop
 * deep link: server session → download → hosted platform (saves become host
 * versions) → collaboration room → view-mode guard (outermost).
 */
import type { CloseReason, CollabClient, SocketFactory } from '@blueoffice/collab';
import type { Platform } from '@blueoffice/platform';
import { loadWasmReplica, startCollab, type Replica } from './collab';
import { HostedStatus, SessionMode, parseMode } from './mode';
import { guardPlatform } from './readonly';
import {
  appForFile,
  commandHandler,
  hostedPlatform,
  openSession,
  type FetchLike,
  type HostCommand,
  type HostFileRef,
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
  fetchImpl: FetchLike;
  /** Replica factory (default: the WASM `bo-collab`). */
  replica?: () => Promise<Replica>;
  socket?: SocketFactory;
  onSaved?: (result: unknown) => void;
  onCollaborators?: (collaborators: unknown) => void;
  onEnded?: (r: CloseReason) => void;
}

export interface HostedEditor {
  app: HostedApp;
  session: HostedSession;
  file: OpenedFile;
  mode: SessionMode;
  status: HostedStatus;
  /** Render the editor with this platform. */
  platform: Platform;
  client: CollabClient | null;
  /** Host commands (`save`, `print`, `export-pdf`, `close`). */
  command(cmd: HostCommand): Promise<unknown>;
  close(): void;
}

export async function openHosted(o: OpenHostedOptions): Promise<HostedEditor> {
  const { session, file } = await openSession(o.api, o.ref, o.fetchImpl);
  const app = appForFile(file.name);
  const mode = new SessionMode(parseMode(session.mode));
  const status = new HostedStatus();
  let platform = hostedPlatform(o.platform, {
    api: o.api,
    session,
    file,
    app,
    mode,
    fetchImpl: o.fetchImpl,
    ...(o.onSaved ? { onSaved: o.onSaved } : {}),
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
      onStatus: (s) => status.setStatus(s),
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
  final = guardPlatform(platform, mode);
  return {
    app,
    session,
    file,
    mode,
    status,
    platform: final,
    client,
    command,
    close: () => client?.close(),
  };
}
