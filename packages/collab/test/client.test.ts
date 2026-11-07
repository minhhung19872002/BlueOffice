import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { CollabClient, type CollabEngine, type SocketLike } from '../src';

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

function client(eng = engine()) {
  const c = new CollabClient({
    url: 'wss://office.example.vn/collab/F1',
    sessionId: 'S1',
    sessionToken: 't&k',
    engine: eng,
    socket: (u) => new FakeSocket(u),
    backoff: { initialMs: 100, maxMs: 400 },
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
});
