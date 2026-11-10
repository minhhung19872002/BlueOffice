/**
 * Local window session (CORE-UI-007, View › New Window): two windows of this device edit
 * the same document. There is no server: the collaboration layer (`@blueoffice/collab`,
 * the same bindings as a hosted session) runs over the platform's window channel
 * (BroadcastChannel on the web, Tauri events on the desktop) through {@link LocalRoomSocket},
 * which stands in for the relay's WebSocket. The document itself is handed over through the
 * offline cache (`openDocumentWindow` in `@blueoffice/platform`).
 *
 * What is shared is what the collaboration bindings share: BlueDoc body text with its
 * formatting, BlueSheet cell inputs and sheet structure, BlueSlide slides and objects.
 */
import { overlay, type SocketLike } from '@blueoffice/collab';
import {
  takeWindowDocument,
  type MessageBus,
  type OpenedFile,
  type Platform,
  type WindowApp,
} from '@blueoffice/platform';
import { loadWasmReplica, startCollab, type Replica } from './collab';
import { SessionMode } from './mode';
import type { HostedSession } from './session';

type RoomMessage =
  | { t: 'join'; id: string }
  | { t: 'member'; id: string; seeded: boolean }
  | { t: 'frame'; id: string; b64: string };

const isRoomMessage = (m: unknown): m is RoomMessage =>
  !!m && typeof m === 'object' && typeof (m as { t?: unknown }).t === 'string';

function toBase64(bytes: Uint8Array): string {
  let s = '';
  for (let i = 0; i < bytes.length; i += 0x8000)
    s += String.fromCharCode(...bytes.subarray(i, i + 0x8000));
  return btoa(s);
}

function fromBase64(b64: string): Uint8Array {
  const s = atob(b64);
  const out = new Uint8Array(s.length);
  for (let i = 0; i < s.length; i++) out[i] = s.charCodeAt(i);
  return out;
}

/** How long a joining window waits for the others to answer before it seeds the room. */
export const ROOM_SETTLE_MS = 300;

/**
 * The relay of a local room as a socket: binary frames go to the other windows as they
 * are, JSON pings are answered here, and the `welcome` the server would send is decided
 * between the windows — the first one in an unseeded room publishes its document
 * (`seed: true`), the others receive it.
 */
export class LocalRoomSocket implements SocketLike {
  binaryType = 'arraybuffer';
  readyState = 0;
  onopen: ((ev: unknown) => void) | null = null;
  onmessage: ((ev: { data: unknown }) => void) | null = null;
  onclose: ((ev: { code: number; reason: string }) => void) | null = null;
  onerror: ((ev: unknown) => void) | null = null;
  private readonly id: string;
  private seeded = false;
  private decided = false;
  /** Members seen while joining: id → has the room's content. */
  private readonly members = new Map<string, boolean>();
  private readonly off: () => void;
  private timer: ReturnType<typeof setTimeout> | null = null;

  constructor(
    private readonly bus: MessageBus,
    opts: { id?: string; settleMs?: number } = {},
  ) {
    this.id = opts.id ?? `${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 10)}`;
    this.off = bus.onMessage((m) => this.receive(m));
    // Like a WebSocket, open asynchronously (the client sets its handlers first).
    queueMicrotask(() => {
      if (this.readyState !== 0) return;
      this.readyState = 1;
      this.bus.post({ t: 'join', id: this.id } satisfies RoomMessage);
      this.onopen?.({});
      this.timer = setTimeout(() => this.decide(), opts.settleMs ?? ROOM_SETTLE_MS);
    });
  }

  send(data: string | Uint8Array): void {
    if (this.readyState !== 1) return;
    if (typeof data === 'string') {
      // Control messages are for the relay: only the keep-alive needs an answer.
      if (data.includes('"ping"')) this.onmessage?.({ data: JSON.stringify({ type: 'pong' }) });
      return;
    }
    this.bus.post({ t: 'frame', id: this.id, b64: toBase64(data) } satisfies RoomMessage);
  }

  close(code = 1000, reason = ''): void {
    if (this.readyState === 3) return;
    this.readyState = 3;
    if (this.timer) clearTimeout(this.timer);
    this.off();
    this.onclose?.({ code, reason });
  }

  private decide(): void {
    this.timer = null;
    if (this.decided || this.readyState !== 1) return;
    this.decided = true;
    const someoneSeeded = [...this.members.values()].some(Boolean);
    // Windows joining an unseeded room at the same time: the smallest id publishes.
    const first = [...this.members.keys()].every((id) => this.id < id);
    this.seeded = someoneSeeded || first;
    this.onmessage?.({
      data: JSON.stringify({ type: 'welcome', mode: 'edit', seed: !someoneSeeded && first }),
    });
    if (this.seeded)
      this.bus.post({ t: 'member', id: this.id, seeded: true } satisfies RoomMessage);
  }

  private receive(raw: unknown): void {
    if (!isRoomMessage(raw) || raw.id === this.id || this.readyState !== 1) return;
    switch (raw.t) {
      case 'join':
        this.members.set(raw.id, false);
        this.bus.post({ t: 'member', id: this.id, seeded: this.seeded } satisfies RoomMessage);
        break;
      case 'member':
        this.members.set(raw.id, raw.seeded);
        // Content arrived in the room: this window has it too once synced.
        if (raw.seeded && this.decided) this.seeded = true;
        break;
      case 'frame':
        this.onmessage?.({ data: fromBase64(raw.b64) });
        break;
    }
  }
}

/** A platform whose editor opens `file` once on start. */
export function platformWithFile(p: Platform, file: OpenedFile): Platform {
  let given = false;
  const shell = overlay(p.shell, {
    onOpenRequest: (handler: (f: OpenedFile) => void) => {
      if (!given) {
        given = true;
        queueMicrotask(() => handler(file));
        return () => {};
      }
      return p.shell.onOpenRequest(handler);
    },
  });
  return { ...p, shell };
}

export interface WindowSessionOptions {
  platform: Platform;
  app: WindowApp;
  /** Session id (the `window` URL parameter). */
  id: string;
  /** Replica factory (default: the WASM `bo-collab`). */
  replica?: () => Promise<Replica>;
  settleMs?: number;
}

export interface WindowSession {
  /** Render the editor with this platform: it opens the document and shares the edits. */
  platform: Platform;
  file: OpenedFile;
  stop(): void;
}

/**
 * Joins the local session `id`: `null` when its document is gone (the session was started
 * in another browser profile, or the cache was cleared) — open the app normally then.
 */
export async function joinWindowSession(o: WindowSessionOptions): Promise<WindowSession | null> {
  const doc = await takeWindowDocument(o.platform.offline, o.id).catch(() => null);
  if (!doc) return null;
  const file: OpenedFile = { bytes: doc.bytes, name: doc.name };
  const bus = o.platform.channel?.(`window-${o.id}`);
  if (!bus) return { platform: platformWithFile(o.platform, file), file, stop: () => {} };
  const session: HostedSession = {
    sessionId: o.id,
    sessionToken: 'local',
    mode: 'edit',
    file: { id: o.id, name: doc.name },
    allowedOrigins: [],
    collab: { url: `/window/${o.id}` },
  };
  const h = startCollab({
    platform: o.platform,
    app: o.app,
    session,
    base: 'ws://window.local',
    replica: await (o.replica ?? loadWasmReplica)(),
    mode: new SessionMode('edit', 'edit'),
    saveNow: () => {},
    socket: () => new LocalRoomSocket(bus, { settleMs: o.settleMs }),
  });
  return {
    platform: platformWithFile(h.platform, file),
    file,
    stop: () => {
      h.stop();
      bus.close();
    },
  };
}
