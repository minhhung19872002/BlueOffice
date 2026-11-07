/**
 * Joins a hosted session's collaboration room (`@blueoffice/collab`,
 * docs/bohp.md §C): BlueDoc text, BlueSlide objects and BlueSheet cell inputs
 * sync live with presence; every app answers the server's autosave requests,
 * and the session mode follows `mode-changed` / revocation.
 */
import {
  CollabClient,
  collabDocHost,
  collabSheetHost,
  collabSlideHost,
  colorFor,
  wasmEngine,
  type CloseReason,
  type CollabEngine,
  type CollabStatus,
  type ReplicaLike,
  type SocketFactory,
  type WasmCollabLike,
} from '@blueoffice/collab';
import type { EngineHost, Platform, SheetHost } from '@blueoffice/platform';
import { parseMode, type SessionMode } from './mode';
import type { HostedApp, HostedSession } from './session';

/** A `bo-collab` replica usable by all three bindings (`WasmCollab`). */
export type Replica = ReplicaLike & WasmCollabLike;

/** Loads the WASM engine module and creates a replica with a random client id. */
export async function loadWasmReplica(): Promise<Replica> {
  const wasm = await import('@blueoffice/engine-wasm');
  const wasmUrl = (await import('@blueoffice/engine-wasm/wasm?url')).default;
  await wasm.default({ module_or_path: wasmUrl });
  const id = crypto.getRandomValues(new Uint32Array(1))[0] ?? 1;
  return new wasm.WasmCollab(id) as unknown as Replica;
}

/** `ws(s)://` URL of the room from the session's (relative) `collab.url`. */
export function collabSocketUrl(collabUrl: string, base: string): string {
  const url = new URL(collabUrl, base);
  url.protocol =
    url.protocol === 'https:' ? 'wss:' : url.protocol === 'http:' ? 'ws:' : url.protocol;
  return url.toString();
}

export interface StartCollabOptions {
  platform: Platform;
  app: HostedApp;
  session: HostedSession;
  /** URL the session's relative `collab.url` resolves against (the server). */
  base: string;
  replica: Replica;
  mode: SessionMode;
  /** The server asks for a checkpoint (autosave / last participant leaving). */
  saveNow: () => void;
  onStatus?: (s: CollabStatus) => void;
  /** The connection ended for good (revoked, expired, reload after a restore). */
  onEnded?: (r: CloseReason) => void;
  socket?: SocketFactory;
}

export interface CollabHandle {
  /** The platform with collaborative engines (render the editor with it). */
  platform: Platform;
  client: CollabClient;
  stop(): void;
}

export function startCollab(o: StartCollabOptions): CollabHandle {
  const { replica, app, session, mode } = o;
  let hosted = o.platform;
  let client: CollabClient | null = null;
  let shared: { remoteApplied(): Promise<void>; seed(): Promise<void> } | null = null;
  let engine: CollabEngine = wasmEngine(replica);
  const flush = () => client?.flush();
  const id = replica.clientId();
  const user = {
    userId: session.user?.id ?? hosted.auth.user?.id ?? `guest-${id}`,
    name: session.user?.name ?? hosted.auth.user?.name ?? '',
    color: colorFor(id),
  };
  if (app === 'sheet' && hosted.sheet) {
    const sheet = collabSheetHost<SheetHost>(hosted.sheet, replica, flush);
    shared = sheet;
    hosted = { ...hosted, sheet };
  } else if (app === 'slide') {
    // BlueSlide: slides and objects (last writer wins per object), selected object.
    const live = collabSlideHost<EngineHost>(hosted.engine, replica, flush, user);
    shared = live;
    engine = live.engine;
    hosted = { ...hosted, engine: live };
  } else {
    // BlueDoc: the body text with formatting, collaborators' carets.
    const live = collabDocHost<EngineHost>(hosted.engine, replica, flush, user);
    shared = live;
    engine = live.engine;
    hosted = { ...hosted, engine: live };
  }
  const c = new CollabClient({
    url: collabSocketUrl(session.collab?.url ?? `/collab/${session.file.id}`, o.base),
    sessionId: session.sessionId,
    sessionToken: session.sessionToken,
    engine,
    ...(o.socket ? { socket: o.socket } : {}),
  });
  client = c;
  // BlueDoc / BlueSlide pull remote changes through their frame gate.
  if (app === 'sheet') c.on('remote', () => void shared?.remoteApplied());
  c.on('control', (m) => {
    switch (m.type) {
      case 'welcome':
        if (m.mode !== undefined) mode.set(parseMode(m.mode));
        // The first editor in an empty room publishes the file (later seeders get `seed`).
        if (m.seed === true) void shared?.seed();
        break;
      case 'mode-changed':
        if (m.mode !== undefined) mode.set(parseMode(m.mode));
        break;
      case 'session-revoked':
        mode.set('revoked');
        break;
      case 'seed':
        void shared?.seed();
        break;
      case 'save-request':
        if (mode.editable) o.saveNow();
        break;
      default:
        break;
    }
  });
  if (o.onStatus) c.on('status', o.onStatus);
  c.on('ended', (r) => {
    if (r === 'revoked') mode.set('revoked');
    o.onEnded?.(r);
  });
  c.connect();
  return { platform: hosted, client: c, stop: () => c.close() };
}
