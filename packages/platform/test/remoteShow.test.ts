import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import {
  FakeSocket,
  RemoteShowError,
  ServerRemoteShow,
  claimRemote,
  connectRemote,
  connectWatch,
  fetchWatchDeck,
  presentSocketUrl,
  type RemoteEvent,
  type RemoteFetch,
} from '../src/remoteShow';

type Call = { url: string; method: string; headers: Record<string, string>; body: unknown };

function fakeServer(answer: (c: Call) => { status: number; body?: unknown; bytes?: Uint8Array }) {
  const calls: Call[] = [];
  const f: RemoteFetch = async (url, init) => {
    const c = {
      url,
      method: init?.method ?? 'GET',
      headers: init?.headers ?? {},
      body: init?.body,
    };
    calls.push(c);
    const a = answer(c);
    return {
      ok: a.status >= 200 && a.status < 300,
      status: a.status,
      json: async () => a.body ?? null,
      arrayBuffer: async () => (a.bytes ?? new Uint8Array()).buffer as ArrayBuffer,
    };
  };
  return { calls, fetch: f };
}

describe('remote presenting client', () => {
  let sockets: FakeSocket[];
  const socket = (url: string) => {
    const s = new FakeSocket(url);
    sockets.push(s);
    return s;
  };
  beforeEach(() => {
    sockets = [];
    vi.useFakeTimers();
  });
  afterEach(() => vi.useRealTimers());

  it('builds WebSocket URLs on the server', () => {
    expect(presentSocketUrl('https://office.example.vn', 'api/present/remote/ws?key=a')).toBe(
      'wss://office.example.vn/api/present/remote/ws?key=a',
    );
    expect(presentSocketUrl('http://10.0.0.5:8080/bo/', 'api/present/x/ws')).toBe(
      'ws://10.0.0.5:8080/bo/api/present/x/ws',
    );
  });

  it('creates a presentation with the session secret, pairs, relays and ends', async () => {
    const srv = fakeServer((c) => {
      if (c.url.endsWith('/api/present/config')) return { status: 200, body: { enabled: true } };
      if (c.url.includes('/api/present?session=s1'))
        return { status: 200, body: { id: 'P1', key: 'K1', canBroadcast: true } };
      if (c.url.endsWith('/P1/remote') && c.method === 'POST')
        return { status: 200, body: { token: 'T1', expiresAt: '2026-11-11T10:05:00Z' } };
      if (c.url.includes('/P1/broadcast?name=') && c.method === 'POST')
        return { status: 200, body: { token: 'V1', refreshed: false } };
      if (c.method === 'DELETE') return { status: 204 };
      return { status: 404 };
    });
    const rs = new ServerRemoteShow({
      baseUrl: 'http://srv',
      session: 's1',
      headers: () => ({ 'x-session-token': 'secret' }),
      fetch: srv.fetch,
      socket,
    });
    expect(await rs.available()).toBe(true);
    const link = await rs.start();
    expect(srv.calls[1]).toMatchObject({
      method: 'POST',
      headers: { 'x-session-token': 'secret' },
    });
    expect(link.canBroadcast).toBe(true);
    expect(sockets[0]!.url).toBe('ws://srv/api/present/P1/ws?key=K1');

    // Sent before the socket opened: delivered on open.
    link.send({ type: 'show', slide: 1 });
    expect(sockets[0]!.sent).toEqual([]);
    sockets[0]!.open();
    expect(sockets[0]!.sent).toEqual([{ type: 'show', slide: 1 }]);

    const events: unknown[] = [];
    link.onEvent((e) => events.push(e));
    sockets[0]!.deliver({ type: 'command', action: 'next' });
    sockets[0]!.deliver({ type: 'peers', remote: true, paired: true, viewers: 0 });
    sockets[0]!.deliver({ type: 'unknown' });
    expect(events).toEqual([
      { type: 'command', action: 'next' },
      { type: 'peers', remote: true, paired: true, viewers: 0 },
    ]);

    expect(await link.pair()).toEqual({ token: 'T1', expiresAt: '2026-11-11T10:05:00Z' });
    expect(srv.calls.at(-1)!.headers).toEqual({ 'X-Present-Key': 'K1' });
    expect(await link.broadcast(new Uint8Array([0x50, 0x4b]), 'Bài giảng.pptx')).toBe('V1');
    expect(srv.calls.at(-1)!.url).toBe(
      'http://srv/api/present/P1/broadcast?name=B%C3%A0i%20gi%E1%BA%A3ng.pptx',
    );

    // A dropped connection reconnects.
    sockets[0]!.drop(1006);
    vi.advanceTimersByTime(1600);
    expect(sockets).toHaveLength(2);
    sockets[1]!.open();
    link.send({ type: 'show', slide: 2 });
    expect(sockets[1]!.sent).toEqual([{ type: 'show', slide: 2 }]);

    await link.end();
    expect(srv.calls.at(-1)).toMatchObject({ method: 'DELETE', url: 'http://srv/api/present/P1' });
    expect(sockets[1]!.closedWith).toBe(1000);
    vi.advanceTimersByTime(5000);
    expect(sockets).toHaveLength(2);
  });

  it('reports why a presentation cannot start', async () => {
    const off = fakeServer(() => ({ status: 404, body: { code: 'unavailable' } }));
    const rs = new ServerRemoteShow({ baseUrl: '', fetch: off.fetch, socket });
    expect(await rs.available()).toBe(false);
    await expect(rs.start()).rejects.toMatchObject({ code: 'unavailable' });
    const anon = fakeServer(() => ({ status: 401 }));
    await expect(
      new ServerRemoteShow({ baseUrl: '', fetch: anon.fetch, socket }).start(),
    ).rejects.toMatchObject({ code: 'unauthorized' });
    const many = fakeServer(() => ({ status: 429, body: { code: 'tooMany' } }));
    await expect(
      new ServerRemoteShow({ baseUrl: '', fetch: many.fetch, socket }).start(),
    ).rejects.toMatchObject({ code: 'tooMany' });
    const down: RemoteFetch = async () => {
      throw new TypeError('Failed to fetch');
    };
    await expect(
      new ServerRemoteShow({ baseUrl: '', fetch: down, socket }).start(),
    ).rejects.toBeInstanceOf(RemoteShowError);
  });

  it('pairs a phone once and sends its commands', async () => {
    const srv = fakeServer((c) =>
      JSON.parse(String(c.body)).token === 'good'
        ? { status: 200, body: { key: 'PHONE' } }
        : { status: 404, body: { code: 'invalidToken' } },
    );
    const o = { baseUrl: 'http://srv', fetch: srv.fetch, socket };
    await expect(claimRemote(o, 'used')).rejects.toMatchObject({ code: 'invalidToken' });
    const key = await claimRemote(o, 'good');
    expect(key).toBe('PHONE');
    const events: RemoteEvent[] = [];
    const remote = connectRemote(o, key, (e) => events.push(e));
    expect(sockets[0]!.url).toBe('ws://srv/api/present/remote/ws?key=PHONE');
    sockets[0]!.open();
    remote.send('next');
    remote.send('black');
    expect(sockets[0]!.sent).toEqual([
      { type: 'command', action: 'next' },
      { type: 'command', action: 'black' },
    ]);
    sockets[0]!.deliver({ type: 'peers', presenter: true });
    expect(events).toEqual([{ type: 'peers', presenter: true }]);

    // Offline for a moment: reconnects; revoked: stops.
    sockets[0]!.drop(1006);
    expect(events.at(-1)).toEqual({ type: 'offline' });
    vi.advanceTimersByTime(1600);
    sockets[1]!.open();
    sockets[1]!.deliver({ type: 'revoked' });
    sockets[1]!.drop(4401);
    expect(events.slice(-2)).toEqual([{ type: 'revoked' }, { type: 'ended' }]);
    vi.advanceTimersByTime(5000);
    expect(sockets).toHaveLength(2);
    remote.close();
  });

  it('a refused connection is not retried', () => {
    const events: RemoteEvent[] = [];
    connectRemote({ baseUrl: 'http://srv', socket }, 'bad', (e) => events.push(e));
    sockets[0]!.drop(1006);
    vi.advanceTimersByTime(5000);
    expect(sockets).toHaveLength(1);
    expect(events).toEqual([{ type: 'ended' }]);
  });

  it('viewers download the deck with the link token and only listen', async () => {
    const srv = fakeServer((c) =>
      c.headers['X-Present-Token'] === 'V1'
        ? { status: 200, bytes: new Uint8Array([0x50, 0x4b, 3, 4]) }
        : { status: 404, body: { code: 'invalidToken' } },
    );
    const o = { baseUrl: 'http://srv', fetch: srv.fetch, socket };
    expect([...(await fetchWatchDeck(o, 'V1'))]).toEqual([0x50, 0x4b, 3, 4]);
    expect(srv.calls[0]!.url).toBe('http://srv/api/present/watch/deck');
    await expect(fetchWatchDeck(o, 'nope')).rejects.toMatchObject({ code: 'invalidToken' });
    const seen: unknown[] = [];
    const w = connectWatch(o, 'V1', (e) => seen.push(e));
    expect(sockets[0]!.url).toBe('ws://srv/api/present/watch/ws?token=V1');
    sockets[0]!.open();
    sockets[0]!.deliver({ type: 'show', slide: 3, black: false, ended: false, click: 1 });
    sockets[0]!.deliver({ type: 'ended' });
    sockets[0]!.drop(4410);
    expect(seen).toEqual([
      { type: 'show', slide: 3, black: false, ended: false, click: 1 },
      { type: 'ended' },
      { type: 'ended' },
    ]);
    w.close();
  });
});
