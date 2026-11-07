/**
 * `@blueoffice/collab` — connects an editor's CRDT replica (`bo-collab`,
 * compiled to WASM or run natively) to the BlueOffice server relay
 * (`WS /collab/{fileId}?session=&token=`, docs/bohp.md §C).
 *
 * Binary frames are y-protocols messages produced/consumed by the engine;
 * text frames are JSON control messages. Offline edits need no queue: on
 * reconnect the engine's sync step 1/2 exchange sends everything missing.
 */

/** The replica side (implemented by `bo-collab::Session`). */
export interface CollabEngine {
  /** Sync step 1 + own awareness, sent right after connecting. */
  start(): Uint8Array;
  /** Handles one binary frame; returns replies for the server. */
  receive(frame: Uint8Array): Uint8Array[];
  /** Pending broadcasts (local updates, presence). */
  drainOutbox(): Uint8Array[];
}

/** JSON control message from the server. */
export interface ControlMessage {
  type:
    | 'welcome'
    | 'collaborators-changed'
    | 'mode-changed'
    | 'save-request'
    | 'saved'
    | 'save-conflict'
    | 'save-error'
    | 'session-revoked'
    | 'reload'
    | 'seed'
    | 'pong';
  [key: string]: unknown;
}

export type CollabStatus = 'connecting' | 'online' | 'offline' | 'closed';

/** Why the connection ended for good (no automatic reconnect). */
export type CloseReason = 'reload' | 'expired' | 'revoked' | 'closed';

/** Close codes (docs/bohp.md §C). */
export const CLOSE_RELOAD = 4001;
export const CLOSE_EXPIRED = 4401;
export const CLOSE_REVOKED = 4403;

/** The subset of the browser `WebSocket` used here (injectable for tests). */
export interface SocketLike {
  binaryType: string;
  readonly readyState: number;
  onopen: ((ev: unknown) => void) | null;
  onmessage: ((ev: { data: unknown }) => void) | null;
  onclose: ((ev: { code: number; reason: string }) => void) | null;
  onerror: ((ev: unknown) => void) | null;
  send(data: string | Uint8Array): void;
  close(code?: number, reason?: string): void;
}

export type SocketFactory = (url: string) => SocketLike;

export interface CollabOptions {
  /** `collab.url` from `SessionInfo`, e.g. `wss://office.x.vn/collab/F1`. */
  url: string;
  sessionId: string;
  sessionToken: string;
  engine: CollabEngine;
  socket?: SocketFactory;
  /** Ping interval in ms (default 25 s). */
  pingMs?: number;
  /** Reconnect backoff: first delay and maximum, ms (defaults 500 / 30 000). */
  backoff?: { initialMs: number; maxMs: number };
}

export interface CollabEvents {
  status: (s: CollabStatus) => void;
  control: (m: ControlMessage) => void;
  /** Remote changes were applied to the engine; re-render. */
  remote: () => void;
  ended: (r: CloseReason) => void;
}

const OPEN = 1;

export class CollabClient {
  private socket: SocketLike | null = null;
  private status: CollabStatus = 'offline';
  private retry = 0;
  private reconnectTimer: ReturnType<typeof setTimeout> | null = null;
  private pingTimer: ReturnType<typeof setInterval> | null = null;
  private stopped = false;
  private readonly listeners: { [K in keyof CollabEvents]: Set<CollabEvents[K]> } = {
    status: new Set(),
    control: new Set(),
    remote: new Set(),
    ended: new Set(),
  };

  constructor(private readonly o: CollabOptions) {}

  on<K extends keyof CollabEvents>(event: K, fn: CollabEvents[K]): () => void {
    this.listeners[event].add(fn);
    return () => this.listeners[event].delete(fn);
  }

  get currentStatus(): CollabStatus {
    return this.status;
  }

  /** WebSocket URL with session credentials (query string, as the server expects). */
  socketUrl(): string {
    const u = new URL(this.o.url);
    u.searchParams.set('session', this.o.sessionId);
    u.searchParams.set('token', this.o.sessionToken);
    return u.toString();
  }

  connect(): void {
    if (this.stopped || this.socket) return;
    this.setStatus('connecting');
    const factory: SocketFactory =
      this.o.socket ?? ((url) => new WebSocket(url) as unknown as SocketLike);
    const ws = factory(this.socketUrl());
    ws.binaryType = 'arraybuffer';
    this.socket = ws;
    ws.onopen = () => {
      this.retry = 0;
      this.setStatus('online');
      ws.send(this.o.engine.start());
      this.flush();
      this.pingTimer = setInterval(() => this.sendJson({ type: 'ping' }), this.o.pingMs ?? 25_000);
    };
    ws.onmessage = (ev) => this.onMessage(ev.data);
    ws.onerror = () => {};
    ws.onclose = (ev) => this.onClose(ev.code);
  }

  /** Sends the engine's pending broadcasts (call after each local edit). */
  flush(): void {
    const frames = this.o.engine.drainOutbox();
    const ws = this.socket;
    // While offline the frames are dropped on purpose: reconnect re-syncs by state vector.
    if (!ws || ws.readyState !== OPEN) return;
    for (const f of frames) ws.send(f);
  }

  /** Asks the server to save to the host now (`{"type":"save"}`). */
  requestSave(): void {
    this.sendJson({ type: 'save' });
  }

  close(): void {
    this.stopped = true;
    this.clearTimers();
    this.socket?.close(1000, 'closed');
    this.socket = null;
    this.setStatus('closed');
    this.emit('ended', 'closed');
  }

  private sendJson(m: Record<string, unknown>): void {
    if (this.socket?.readyState === OPEN) this.socket.send(JSON.stringify(m));
  }

  private onMessage(data: unknown): void {
    if (typeof data === 'string') {
      let msg: ControlMessage;
      try {
        msg = JSON.parse(data) as ControlMessage;
      } catch {
        return;
      }
      if (typeof msg === 'object' && msg !== null && typeof msg.type === 'string') {
        this.emit('control', msg);
      }
      return;
    }
    const frame =
      data instanceof Uint8Array ? data : data instanceof ArrayBuffer ? new Uint8Array(data) : null;
    if (!frame) return;
    const replies = this.o.engine.receive(frame);
    for (const r of replies) this.socket?.send(r);
    this.emit('remote');
  }

  private onClose(code: number): void {
    this.clearTimers();
    this.socket = null;
    if (this.stopped) return;
    const final: CloseReason | null =
      code === CLOSE_RELOAD
        ? 'reload'
        : code === CLOSE_EXPIRED
          ? 'expired'
          : code === CLOSE_REVOKED
            ? 'revoked'
            : null;
    if (final) {
      this.stopped = true;
      this.setStatus('closed');
      this.emit('ended', final);
      return;
    }
    this.setStatus('offline');
    const { initialMs, maxMs } = this.o.backoff ?? { initialMs: 500, maxMs: 30_000 };
    const delay = Math.min(maxMs, initialMs * 2 ** this.retry);
    this.retry += 1;
    this.reconnectTimer = setTimeout(() => {
      this.reconnectTimer = null;
      this.connect();
    }, delay);
  }

  private clearTimers(): void {
    if (this.pingTimer) clearInterval(this.pingTimer);
    if (this.reconnectTimer) clearTimeout(this.reconnectTimer);
    this.pingTimer = null;
    this.reconnectTimer = null;
  }

  private setStatus(s: CollabStatus): void {
    if (this.status === s) return;
    this.status = s;
    this.emit('status', s);
  }

  private emit<K extends keyof CollabEvents>(event: K, ...args: Parameters<CollabEvents[K]>): void {
    for (const fn of this.listeners[event]) {
      (fn as (...a: Parameters<CollabEvents[K]>) => void)(...args);
    }
  }
}
export * from './sheet';
export * from './live';
export * from './doc';
export * from './slide';
