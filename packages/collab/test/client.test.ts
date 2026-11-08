import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import {
  CLOSE_MOVED,
  CollabClient,
  encodeSyncUpdate,
  type CollabEngine,
  type NetworkLike,
  type SocketLike,
} from '../src';

class FakeSocket implements SocketLike {
  static all: FakeSocket[] = [];
  binaryType = 'blob';
  readyState = 0;
  sent: (string | Uint8Array)[] = [];
  onopen: SocketLike['onopen'] = null;
  onmessage: SocketLike['onmessage'] = null;
  onclose: SocketLike['onclose'] = null;
  onerror: SocketLike['onerror'] = null;
  constructor(public url: string) {
    FakeSocket.all.push(this);
  }
  send(d: string | Uint8Array): void {
    this.sent.push(d);
  }
  close(code = 1000): void {
    this.readyState = 3;
    this.onclose?.({ code, reason: '' });
  }
  open(): void {
    this.readyState = 1;
    this.onopen?.({});
  }
  serverClose(code: number): void {
    this.readyState = 3;
    this.onclose?.({ code, reason: '' });
  }
}

function engine(): CollabEngine & { received: Uint8Array[]; outbox: Uint8Array[] } {
  const e = {
    received: [] as Uint8Array[],
    outbox: [] as Uint8Array[],
    start: () => new Uint8Array([0, 0, 1, 0]),
    snapshot: () => new Uint8Array([1, 1, 5, 0, 0]),
    receive(f: Uint8Array) {
      e.received.push(f);
      return [new Uint8Array([9])];
    },
    drainOutbox() {
      const o = e.outbox;
      e.outbox = [];
      return o;
    },
  };
  return e;
}

function fakeNetwork() {
  let online = true;
  const fns = new Set<(o: boolean) => void>();
  const net: NetworkLike & { set(o: boolean): void } = {
    online: () => online,
    onChange: (fn) => {
      fns.add(fn);
      return () => fns.delete(fn);
    },
    set(o) {
      online = o;
      fns.forEach((f) => f(o));
    },
  };
  return net;
}

function client(eng = engine(), network: NetworkLike | null = null) {
  const c = new CollabClient({
    url: 'wss://office.example.vn/collab/F1',
    sessionId: 'S1',
    sessionToken: 't&k',
    engine: eng,
    socket: (u) => new FakeSocket(u),
    backoff: { initialMs: 100, maxMs: 400 },
    network,
  });
  return { c, eng };
}

beforeEach(() => {
  FakeSocket.all = [];
  vi.useFakeTimers();
});
afterEach(() => vi.useRealTimers());

describe('CollabClient', () => {
  it('connects with session credentials and syncs on open', () => {
    const { c, eng } = client();
    const statuses: string[] = [];
    c.on('status', (s) => statuses.push(s));
    eng.outbox.push(new Uint8Array([7]));
    c.connect();
    const ws = FakeSocket.all[0]!;
    expect(ws.url).toBe('wss://office.example.vn/collab/F1?session=S1&token=t%26k');
    ws.open();
    expect(ws.binaryType).toBe('arraybuffer');
    expect(ws.sent).toEqual([new Uint8Array([0, 0, 1, 0]), new Uint8Array([7])]);
    expect(statuses).toEqual(['connecting', 'online']);
  });

  it('applies binary frames, replies, and forwards control messages', () => {
    const { c, eng } = client();
    const controls: unknown[] = [];
    let remote = 0;
    c.on('control', (m) => controls.push(m));
    c.on('remote', () => remote++);
    c.connect();
    const ws = FakeSocket.all[0]!;
    ws.open();
    ws.onmessage?.({ data: new Uint8Array([0, 2, 1, 5]).buffer });
    ws.onmessage?.({ data: '{"type":"mode-changed","mode":"view"}' });
    ws.onmessage?.({ data: 'not json' });
    expect(eng.received).toHaveLength(1);
    expect(ws.sent.at(-1)).toEqual(new Uint8Array([9]));
    expect(remote).toBe(1);
    expect(controls).toEqual([{ type: 'mode-changed', mode: 'view' }]);
  });

  it('drops frames while offline and reconnects with backoff', () => {
    const { c, eng } = client();
    c.connect();
    FakeSocket.all[0]!.open();
    FakeSocket.all[0]!.serverClose(1006);
    expect(c.currentStatus).toBe('offline');
    eng.outbox.push(new Uint8Array([1]));
    c.flush();
    expect(eng.outbox).toHaveLength(0);
    vi.advanceTimersByTime(99);
    expect(FakeSocket.all).toHaveLength(1);
    vi.advanceTimersByTime(1);
    expect(FakeSocket.all).toHaveLength(2);
    FakeSocket.all[1]!.serverClose(1006);
    vi.advanceTimersByTime(200);
    expect(FakeSocket.all).toHaveLength(3);
    FakeSocket.all[2]!.open();
    expect(c.currentStatus).toBe('online');
    // Resync on reconnect starts with sync step 1.
    expect(FakeSocket.all[2]!.sent[0]).toEqual(new Uint8Array([0, 0, 1, 0]));
  });

  it.each([
    [4403, 'revoked'],
    [4401, 'expired'],
    [4001, 'reload'],
  ])('close code %i ends the session (%s) without reconnecting', (code, reason) => {
    const { c } = client();
    const ended: string[] = [];
    c.on('ended', (r) => ended.push(r));
    c.connect();
    FakeSocket.all[0]!.open();
    FakeSocket.all[0]!.serverClose(code);
    vi.advanceTimersByTime(10_000);
    expect(FakeSocket.all).toHaveLength(1);
    expect(ended).toEqual([reason]);
    expect(c.currentStatus).toBe('closed');
  });

  it('close code 4009 (room on another api instance) reconnects instead of ending', () => {
    const { c } = client();
    const ended: string[] = [];
    c.on('ended', (r) => ended.push(r));
    c.connect();
    FakeSocket.all[0]!.open();
    FakeSocket.all[0]!.serverClose(CLOSE_MOVED);
    expect(c.currentStatus).toBe('offline');
    vi.advanceTimersByTime(10_000);
    expect(FakeSocket.all.length).toBeGreaterThan(1);
    expect(ended).toEqual([]);
  });

  it('pings and requests saves', () => {
    const { c } = client();
    c.connect();
    const ws = FakeSocket.all[0]!;
    ws.open();
    c.requestSave();
    vi.advanceTimersByTime(25_000);
    expect(ws.sent.filter((s) => typeof s === 'string')).toEqual([
      '{"type":"save"}',
      '{"type":"ping"}',
    ]);
    c.close();
    vi.advanceTimersByTime(60_000);
    expect(FakeSocket.all).toHaveLength(1);
  });

  it('re-publishes its state after a reconnect when it changed something', () => {
    const { c, eng } = client();
    c.connect();
    FakeSocket.all[0]!.open();
    // No local change: a reconnect only sends sync step 1.
    FakeSocket.all[0]!.serverClose(1006);
    vi.advanceTimersByTime(100);
    FakeSocket.all[1]!.open();
    expect(FakeSocket.all[1]!.sent).toEqual([new Uint8Array([0, 0, 1, 0])]);
    // A local update (sync frame) is dropped while offline …
    FakeSocket.all[1]!.serverClose(1006);
    eng.outbox.push(new Uint8Array([0, 2, 3, 1, 2, 3]));
    c.flush();
    vi.advanceTimersByTime(100);
    FakeSocket.all[2]!.open();
    // … and the full state follows sync step 1 on the next connection.
    expect(FakeSocket.all[2]!.sent).toEqual([
      new Uint8Array([0, 0, 1, 0]),
      encodeSyncUpdate(new Uint8Array([1, 1, 5, 0, 0])),
    ]);
    // Presence (awareness frames) alone does not trigger a re-publish.
    eng.outbox.push(new Uint8Array([1, 4, 0]));
    c.flush();
    FakeSocket.all[2]!.serverClose(1006);
    vi.advanceTimersByTime(100);
    FakeSocket.all[3]!.open();
    expect(FakeSocket.all[3]!.sent).toEqual([new Uint8Array([0, 0, 1, 0])]);
  });

  it('encodes long updates with a var-uint length', () => {
    const big = new Uint8Array(300).fill(7);
    const f = encodeSyncUpdate(big);
    expect([...f.slice(0, 4)]).toEqual([0, 2, 0xac, 0x02]);
    expect(f.length).toBe(304);
  });

  it('closes the socket when the device goes offline and reconnects when back online', () => {
    const net = fakeNetwork();
    const { c, eng } = client(engine(), net);
    c.connect();
    FakeSocket.all[0]!.open();
    net.set(false);
    expect(c.currentStatus).toBe('offline');
    expect(FakeSocket.all[0]!.readyState).toBe(3);
    // Offline edits; no reconnect attempts while offline.
    eng.outbox.push(new Uint8Array([0, 2, 1, 9]));
    c.flush();
    vi.advanceTimersByTime(60_000);
    expect(FakeSocket.all).toHaveLength(1);
    net.set(true);
    expect(FakeSocket.all).toHaveLength(2);
    FakeSocket.all[1]!.open();
    expect(c.currentStatus).toBe('online');
    expect(FakeSocket.all[1]!.sent.at(-1)).toEqual(
      encodeSyncUpdate(new Uint8Array([1, 1, 5, 0, 0])),
    );
    c.close();
    net.set(false);
    net.set(true);
    expect(FakeSocket.all).toHaveLength(2);
  });

  it('starts offline without a socket and connects on the online event', () => {
    const net = fakeNetwork();
    net.set(false);
    const { c } = client(engine(), net);
    c.connect();
    expect(FakeSocket.all).toHaveLength(0);
    expect(c.currentStatus).toBe('offline');
    net.set(true);
    expect(FakeSocket.all).toHaveLength(1);
  });
});
