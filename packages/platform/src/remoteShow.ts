/**
 * Remote control from a phone and online presentation (SLIDE-REMOTE-001), through the
 * BlueOffice server's `/api/present` relay (docs/deploy/security.md). Nothing goes to a
 * third-party service: the phone and the viewers talk to the same server as the editor.
 *
 * - The presenter (`PresenterLink`) creates a presentation, asks for a pairing code shown as a
 *   QR code, sends the phone its state (`RemoteState`) and the viewers the show position
 *   (`AudienceMsg`-shaped `show` messages) and receives the phone's commands.
 * - The phone (`claimRemote` + `connectRemote`) exchanges the code once for its own key, then
 *   sends next / previous / black / end.
 * - Viewers (`fetchWatchDeck` + `connectWatch`) download the deck and follow the presenter;
 *   they cannot send anything that moves the show.
 */

/** Commands the phone can send (the server refuses any other). */
export type RemoteAction = 'next' | 'prev' | 'first' | 'last' | 'black' | 'white' | 'end' | 'start';

export const REMOTE_ACTIONS: readonly RemoteAction[] = [
  'next',
  'prev',
  'first',
  'last',
  'black',
  'white',
  'end',
  'start',
];

/** What the phone shows: sent by the presenter after every change. */
export interface RemoteState {
  type: 'remote';
  /** The slide show runs (otherwise the phone offers to start it). */
  showing: boolean;
  /** Slide index shown (0-based) and slides in the deck. */
  slide: number;
  total: number;
  /** Place in the playing order (1-based) and its length. */
  at: number;
  of: number;
  /** Click step played on the slide and its click steps. */
  click: number;
  clicks: number;
  black: boolean;
  white: boolean;
  /** The end screen after the last slide. */
  ended: boolean;
  notes: string;
  /** Thumbnails (data URLs) of the current and next slides; `null` when none. */
  current: string | null;
  next: string | null;
  /** When the show started (ms since the epoch), for the phone's timer. */
  startedAt: number | null;
}

/** What the presenter's connection reports. */
export type PresenterEvent =
  | { type: 'peers'; remote: boolean; paired: boolean; viewers: number }
  | { type: 'command'; action: RemoteAction }
  | { type: 'ended'; reason?: string };

/** What the phone's connection reports. */
export type RemoteEvent =
  | RemoteState
  | { type: 'peers'; presenter: boolean }
  | { type: 'error'; code: string }
  | { type: 'revoked' }
  | { type: 'ended'; reason?: string }
  /** The connection is down (it reconnects unless revoked or ended). */
  | { type: 'offline' };

/** What a viewer's connection reports. */
export type WatchEvent =
  | { type: 'show'; slide: number; black: boolean; ended: boolean; click?: number; enter?: string }
  | { type: 'reload' }
  | { type: 'ended'; reason?: string }
  | { type: 'offline' };

export type RemoteShowErrorCode =
  | 'unavailable'
  | 'unauthorized'
  | 'invalidToken'
  | 'rateLimited'
  | 'tooMany'
  | 'forbidden'
  | 'tooLarge'
  | 'network';

export class RemoteShowError extends Error {
  constructor(
    readonly code: RemoteShowErrorCode,
    message?: string,
  ) {
    super(message ?? code);
    this.name = 'RemoteShowError';
  }
}

/** The parts of `WebSocket` used here. */
export interface SocketLike {
  readonly readyState: number;
  send(data: string): void;
  close(code?: number, reason?: string): void;
  onopen: ((e: unknown) => void) | null;
  onmessage: ((e: { data: unknown }) => void) | null;
  onclose: ((e: { code: number }) => void) | null;
  onerror: ((e: unknown) => void) | null;
}

export type SocketFactory = (url: string) => SocketLike;

export type RemoteFetch = (
  url: string,
  init?: { method?: string; headers?: Record<string, string>; body?: BodyInit | null },
) => Promise<{
  ok: boolean;
  status: number;
  json(): Promise<unknown>;
  arrayBuffer(): Promise<ArrayBuffer>;
}>;

export interface RemoteShowOptions {
  /** BlueOffice server base URL; `''` = same origin. */
  baseUrl: string;
  /** Editor session id (hosted files): the presentation is created with the session's secret. */
  session?: string;
  /** Extra headers (bearer token, `x-session-token`). */
  headers?: () => Promise<Record<string, string>> | Record<string, string>;
  fetch?: RemoteFetch;
  socket?: SocketFactory;
  /** Delay before reconnecting a dropped connection (ms). */
  retryMs?: number;
}

const OPEN = 1;
/** Reconnect attempts in a row before a dropped connection is given up. */
const MAX_RETRIES = 20;

function origin(): string {
  return typeof location !== 'undefined' ? location.href : 'http://localhost/';
}

/** Absolute HTTP(S) URL of `path` on the server. */
export function presentUrl(baseUrl: string, path: string): string {
  return new URL(path, baseUrl ? baseUrl.replace(/\/?$/, '/') : origin()).toString();
}

/** WebSocket URL of `path` on the server. */
export function presentSocketUrl(baseUrl: string, path: string): string {
  const u = new URL(presentUrl(baseUrl, path));
  u.protocol = u.protocol === 'https:' ? 'wss:' : 'ws:';
  return u.toString();
}

function defaults(o: RemoteShowOptions): { fetch: RemoteFetch; socket: SocketFactory } {
  return {
    fetch:
      o.fetch ??
      ((url, init) => fetch(url, init as RequestInit) as unknown as ReturnType<RemoteFetch>),
    socket: o.socket ?? ((url) => new WebSocket(url) as unknown as SocketLike),
  };
}

async function errorOf(res: {
  status: number;
  json(): Promise<unknown>;
}): Promise<RemoteShowError> {
  let code: unknown;
  try {
    code = ((await res.json()) as { code?: unknown } | null)?.code;
  } catch {
    code = undefined;
  }
  if (code === 'tooMany') return new RemoteShowError('tooMany');
  if (code === 'invalidToken') return new RemoteShowError('invalidToken');
  if (res.status === 429) return new RemoteShowError('rateLimited');
  if (res.status === 401) return new RemoteShowError('unauthorized');
  if (res.status === 403) return new RemoteShowError('forbidden');
  if (res.status === 413) return new RemoteShowError('tooLarge');
  if (res.status === 404 && code === 'unavailable') return new RemoteShowError('unavailable');
  if (res.status === 404) return new RemoteShowError('invalidToken');
  return new RemoteShowError('network', `HTTP ${res.status}`);
}

async function call<T>(
  f: RemoteFetch,
  url: string,
  init: { method?: string; headers?: Record<string, string>; body?: BodyInit | null },
): Promise<T> {
  let res: Awaited<ReturnType<RemoteFetch>>;
  try {
    res = await f(url, init);
  } catch (e) {
    throw new RemoteShowError('network', e instanceof Error ? e.message : String(e));
  }
  if (!res.ok) throw await errorOf(res);
  if (res.status === 204) return undefined as T;
  return (await res.json()) as T;
}

/**
 * A WebSocket that reconnects after a drop (phones sleep, Wi-Fi changes) until it is closed
 * here or the server ends it (4401 revoked, 4410 ended) or refuses it.
 */
class Relay {
  private socket: SocketLike | null = null;
  private closed = false;
  private timer: ReturnType<typeof setTimeout> | null = null;
  private queue: string[] = [];
  /** Connected at least once; reconnect attempts that failed in a row. */
  private everOpened = false;
  private failures = 0;

  constructor(
    private readonly make: SocketFactory,
    private readonly url: string,
    private readonly onMessage: (m: Record<string, unknown>) => void,
    private readonly onDown: (final: boolean) => void,
    private readonly retryMs: number,
  ) {
    this.connect();
  }

  private connect(): void {
    if (this.closed) return;
    const s = this.make(this.url);
    this.socket = s;
    let opened = false;
    s.onopen = () => {
      opened = true;
      this.everOpened = true;
      this.failures = 0;
      for (const m of this.queue.splice(0)) s.send(m);
    };
    s.onmessage = (e) => {
      if (typeof e.data !== 'string') return;
      try {
        const m: unknown = JSON.parse(e.data);
        if (m && typeof m === 'object' && typeof (m as { type?: unknown }).type === 'string') {
          this.onMessage(m as Record<string, unknown>);
        }
      } catch {
        // Not JSON: ignored.
      }
    };
    s.onerror = () => {};
    s.onclose = (e) => {
      if (this.socket !== s) return;
      this.socket = null;
      if (!opened) this.failures++;
      // Revoked / ended, refused the first time, or gone for good: no retry.
      const final =
        this.closed ||
        e.code === 4401 ||
        e.code === 4410 ||
        this.failures >= (this.everOpened ? MAX_RETRIES : 1);
      this.onDown(final);
      if (!final) this.timer = setTimeout(() => this.connect(), this.retryMs);
      else this.closed = true;
    };
  }

  send(msg: unknown): void {
    const data = JSON.stringify(msg);
    if (this.socket && this.socket.readyState === OPEN) this.socket.send(data);
    else if (!this.closed) {
      // The latest state matters: keep a short queue.
      this.queue.push(data);
      if (this.queue.length > 8) this.queue.shift();
    }
  }

  close(): void {
    this.closed = true;
    if (this.timer) clearTimeout(this.timer);
    const s = this.socket;
    this.socket = null;
    s?.close(1000, '');
  }
}

/** The presenting editor's side of a presentation. */
export class PresenterLink {
  private readonly handlers = new Set<(e: PresenterEvent) => void>();
  private readonly relay: Relay;
  private viewerToken: string | null = null;
  private ended = false;

  constructor(
    private readonly opts: RemoteShowOptions,
    readonly id: string,
    private readonly key: string,
    /** The deck may be handed to viewers (hosted files: the download permission). */
    readonly canBroadcast: boolean,
  ) {
    const d = defaults(opts);
    this.relay = new Relay(
      d.socket,
      presentSocketUrl(
        opts.baseUrl,
        `api/present/${encodeURIComponent(id)}/ws?key=${encodeURIComponent(key)}`,
      ),
      (m) => {
        if (m.type === 'peers' || m.type === 'command' || m.type === 'ended') {
          if (m.type === 'ended') this.ended = true;
          for (const h of this.handlers) h(m as unknown as PresenterEvent);
        }
      },
      () => {},
      opts.retryMs ?? 1500,
    );
  }

  private get http() {
    return defaults(this.opts).fetch;
  }

  private url(path: string): string {
    return presentUrl(this.opts.baseUrl, `api/present/${encodeURIComponent(this.id)}${path}`);
  }

  private get keyHeader(): Record<string, string> {
    return { 'X-Present-Key': this.key };
  }

  /** Has the presentation ended (on the server)? */
  get isEnded(): boolean {
    return this.ended;
  }

  onEvent(h: (e: PresenterEvent) => void): () => void {
    this.handlers.add(h);
    return () => this.handlers.delete(h);
  }

  /** A new pairing code (the previous one and its phone are revoked). */
  async pair(): Promise<{ token: string; expiresAt: string }> {
    return call(this.http, this.url('/remote'), { method: 'POST', headers: this.keyHeader });
  }

  /** Revokes the pairing code and the paired phone. */
  async revokeRemote(): Promise<void> {
    await call<void>(this.http, this.url('/remote'), {
      method: 'DELETE',
      headers: this.keyHeader,
    });
  }

  /** Starts (or refreshes) the online presentation; resolves the viewer link token. */
  async broadcast(deck: Uint8Array, name: string): Promise<string> {
    const copy = new Uint8Array(deck.byteLength);
    copy.set(deck);
    const r = await call<{ token: string | null; refreshed: boolean }>(
      this.http,
      this.url(`/broadcast?name=${encodeURIComponent(name)}`),
      {
        method: 'POST',
        headers: {
          ...this.keyHeader,
          'Content-Type':
            'application/vnd.openxmlformats-officedocument.presentationml.presentation',
        },
        body: new Blob([copy.buffer]),
      },
    );
    if (r.token) this.viewerToken = r.token;
    if (!this.viewerToken) throw new RemoteShowError('network', 'no viewer token');
    return this.viewerToken;
  }

  /** Ends the online presentation (viewers are told). */
  async endBroadcast(): Promise<void> {
    this.viewerToken = null;
    await call<void>(this.http, this.url('/broadcast'), {
      method: 'DELETE',
      headers: this.keyHeader,
    });
  }

  /** Sends the phone's state or the viewers' show position. */
  send(msg: RemoteState | { type: 'show'; [k: string]: unknown }): void {
    this.relay.send(msg);
  }

  /** Ends the presentation on the server (phone and viewers are told) and closes. */
  async end(): Promise<void> {
    this.relay.close();
    this.handlers.clear();
    if (this.ended) return;
    this.ended = true;
    await call<void>(
      this.http,
      presentUrl(this.opts.baseUrl, `api/present/${encodeURIComponent(this.id)}`),
      {
        method: 'DELETE',
        headers: this.keyHeader,
      },
    ).catch(() => {});
  }
}

/** Remote presenting through a BlueOffice server. */
export class ServerRemoteShow {
  constructor(readonly opts: RemoteShowOptions) {}

  /** Is it turned on there? */
  async available(): Promise<boolean> {
    try {
      const r = await call<{ enabled?: boolean }>(
        defaults(this.opts).fetch,
        presentUrl(this.opts.baseUrl, 'api/present/config'),
        { headers: await this.headers() },
      );
      return r?.enabled === true;
    } catch {
      return false;
    }
  }

  private async headers(): Promise<Record<string, string>> {
    return (await this.opts.headers?.()) ?? {};
  }

  /** Creates a presentation (signed-in user or editor session). */
  async start(): Promise<PresenterLink> {
    const q = this.opts.session ? `?session=${encodeURIComponent(this.opts.session)}` : '';
    const r = await call<{ id: string; key: string; canBroadcast: boolean }>(
      defaults(this.opts).fetch,
      presentUrl(this.opts.baseUrl, `api/present${q}`),
      { method: 'POST', headers: await this.headers() },
    );
    return new PresenterLink(this.opts, r.id, r.key, r.canBroadcast);
  }
}

// ------------------------------------------------------------------------------ the phone

/** Exchanges the pairing code (once) for the phone's key. */
export async function claimRemote(opts: RemoteShowOptions, token: string): Promise<string> {
  const r = await call<{ key: string }>(
    defaults(opts).fetch,
    presentUrl(opts.baseUrl, 'api/present/remote/claim'),
    {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ token }),
    },
  );
  return r.key;
}

export interface RemoteLink {
  send(action: RemoteAction): void;
  close(): void;
}

/** The phone's connection: its events and its commands. */
export function connectRemote(
  opts: RemoteShowOptions,
  key: string,
  onEvent: (e: RemoteEvent) => void,
): RemoteLink {
  const relay = new Relay(
    defaults(opts).socket,
    presentSocketUrl(opts.baseUrl, `api/present/remote/ws?key=${encodeURIComponent(key)}`),
    (m) => onEvent(m as unknown as RemoteEvent),
    (final) => onEvent(final ? { type: 'ended' } : { type: 'offline' }),
    opts.retryMs ?? 1500,
  );
  return {
    send: (action) => relay.send({ type: 'command', action }),
    close: () => relay.close(),
  };
}

// ------------------------------------------------------------------------------ viewers

/** The deck of an online presentation. */
export async function fetchWatchDeck(opts: RemoteShowOptions, token: string): Promise<Uint8Array> {
  let res: Awaited<ReturnType<RemoteFetch>>;
  try {
    res = await defaults(opts).fetch(presentUrl(opts.baseUrl, 'api/present/watch/deck'), {
      headers: { 'X-Present-Token': token },
    });
  } catch (e) {
    throw new RemoteShowError('network', e instanceof Error ? e.message : String(e));
  }
  if (!res.ok) throw await errorOf(res);
  return new Uint8Array(await res.arrayBuffer());
}

/** A viewer's connection (receive only). */
export function connectWatch(
  opts: RemoteShowOptions,
  token: string,
  onEvent: (e: WatchEvent) => void,
): { close(): void } {
  const relay = new Relay(
    defaults(opts).socket,
    presentSocketUrl(opts.baseUrl, `api/present/watch/ws?token=${encodeURIComponent(token)}`),
    (m) => onEvent(m as unknown as WatchEvent),
    (final) => onEvent(final ? { type: 'ended' } : { type: 'offline' }),
    opts.retryMs ?? 1500,
  );
  return { close: () => relay.close() };
}

// ------------------------------------------------------------------------------ tests

/** An in-memory socket for tests: `deliver` plays the server, `sent` records what was sent. */
export class FakeSocket implements SocketLike {
  readyState = 0;
  sent: unknown[] = [];
  closedWith: number | null = null;
  onopen: ((e: unknown) => void) | null = null;
  onmessage: ((e: { data: unknown }) => void) | null = null;
  onclose: ((e: { code: number }) => void) | null = null;
  onerror: ((e: unknown) => void) | null = null;

  constructor(readonly url: string) {}

  open(): void {
    this.readyState = OPEN;
    this.onopen?.({});
  }

  deliver(msg: unknown): void {
    this.onmessage?.({ data: JSON.stringify(msg) });
  }

  /** The server closes the connection. */
  drop(code = 1006): void {
    this.readyState = 3;
    this.onclose?.({ code });
  }

  send(data: string): void {
    this.sent.push(JSON.parse(data));
  }

  close(code = 1000): void {
    this.closedWith = code;
    this.readyState = 3;
  }
}
