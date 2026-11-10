import { describe, expect, it, vi } from 'vitest';
import {
  stashWindowDocument,
  type MessageBus,
  type OfflineCache,
  type OpenedFile,
  type Platform,
} from '@blueoffice/platform';
import { LocalRoomSocket, joinWindowSession, platformWithFile, type Replica } from '../src';

/** Buses of one channel: a message goes to every other bus (structured clone ≈ JSON). */
function hub() {
  const handlers = new Set<{ bus: MessageBus; fn: (m: unknown) => void }>();
  return (): MessageBus => {
    const self: MessageBus = {
      post: (m) => {
        const copy: unknown = JSON.parse(JSON.stringify(m));
        for (const h of [...handlers]) if (h.bus !== self) h.fn(copy);
      },
      onMessage: (fn) => {
        const h = { bus: self, fn };
        handlers.add(h);
        return () => handlers.delete(h);
      },
      close: () => {
        for (const h of [...handlers]) if (h.bus === self) handlers.delete(h);
      },
    };
    return self;
  };
}

const tick = () => new Promise<void>((r) => setTimeout(r, 0));

function listen(s: LocalRoomSocket) {
  const control: { type: string; seed?: boolean }[] = [];
  const frames: Uint8Array[] = [];
  let opened = 0;
  s.onopen = () => {
    opened += 1;
  };
  s.onmessage = (ev) => {
    if (typeof ev.data === 'string') control.push(JSON.parse(ev.data) as { type: string });
    else frames.push(ev.data as Uint8Array);
  };
  return { control, frames, opened: () => opened };
}

describe('LocalRoomSocket', () => {
  it('the first window seeds the room; a later one receives', async () => {
    vi.useFakeTimers();
    try {
      const bus = hub();
      const a = new LocalRoomSocket(bus(), { id: 'a', settleMs: 100 });
      const la = listen(a);
      await vi.advanceTimersByTimeAsync(0);
      expect(a.readyState).toBe(1);
      expect(la.opened()).toBe(1);
      await vi.advanceTimersByTimeAsync(100);
      expect(la.control).toEqual([{ type: 'welcome', mode: 'edit', seed: true }]);

      const b = new LocalRoomSocket(bus(), { id: 'b', settleMs: 100 });
      const lb = listen(b);
      await vi.advanceTimersByTimeAsync(100);
      expect(lb.control).toEqual([{ type: 'welcome', mode: 'edit', seed: false }]);

      // Binary frames reach the other window unchanged, never the sender.
      const frame = new Uint8Array([0, 2, 255, 128, 7]);
      a.send(frame);
      expect(lb.frames).toEqual([frame]);
      expect(la.frames).toEqual([]);
      // The keep-alive is answered locally.
      b.send(JSON.stringify({ type: 'ping' }));
      expect(lb.control.at(-1)).toEqual({ type: 'pong' });

      // A reloads (new socket): the room already has content, so it does not seed again.
      a.close();
      const a2 = new LocalRoomSocket(bus(), { id: 'a2', settleMs: 100 });
      const la2 = listen(a2);
      await vi.advanceTimersByTimeAsync(100);
      expect(la2.control).toEqual([{ type: 'welcome', mode: 'edit', seed: false }]);
    } finally {
      vi.useRealTimers();
    }
  });

  it('two windows joining an empty room at once: exactly one seeds', async () => {
    vi.useFakeTimers();
    try {
      const bus = hub();
      const a = new LocalRoomSocket(bus(), { id: 'k2', settleMs: 100 });
      const b = new LocalRoomSocket(bus(), { id: 'k1', settleMs: 100 });
      const la = listen(a);
      const lb = listen(b);
      await vi.advanceTimersByTimeAsync(150);
      const seeds = [la.control[0]?.seed, lb.control[0]?.seed];
      expect(seeds.filter(Boolean)).toHaveLength(1);
      // The smaller id publishes.
      expect(lb.control[0]?.seed).toBe(true);
    } finally {
      vi.useRealTimers();
    }
  });

  it('a closed socket neither sends nor receives', async () => {
    const bus = hub();
    const a = new LocalRoomSocket(bus(), { id: 'a', settleMs: 5 });
    const b = new LocalRoomSocket(bus(), { id: 'b', settleMs: 5 });
    const lb = listen(b);
    const closed = vi.fn();
    a.onclose = closed;
    await tick();
    a.close();
    expect(closed).toHaveBeenCalledWith({ code: 1000, reason: '' });
    a.send(new Uint8Array([1]));
    expect(lb.frames).toEqual([]);
    b.close();
  });
});

function memoryCache(): OfflineCache {
  const map = new Map<string, Uint8Array>();
  return {
    get: async (k) => map.get(k) ?? null,
    put: async (k, v) => void map.set(k, v),
    delete: async (k) => void map.delete(k),
  };
}

const replica = (id: number): Replica =>
  ({
    setTime() {},
    start: () => new Uint8Array([0, 0, 1, 0]),
    receive: () => new Uint8Array(),
    drainOutbox: () => new Uint8Array(),
    setCell: vi.fn(),
    cellsJson: () => '{}',
    setPresence() {},
    peersJson: () => '[]',
    clientId: () => id,
  }) as unknown as Replica;

function sheetPlatform(offline: OfflineCache, bus: () => MessageBus) {
  const opened: OpenedFile[] = [];
  const shell = {
    recentFiles: async () => [],
    addRecentFile: async () => {},
    onOpenRequest: vi.fn(() => () => {}),
    getSetting: async (k: string) => `value of ${k}`,
  };
  const sheet = {
    info: async () => ({ sheets: [{ name: 'Sheet1', lastRow: 0, lastCol: 0 }] }),
    setInputs: async () => {},
    exec: async () => null,
    cell: async () => ({ input: '' }),
  };
  const p = {
    kind: 'web',
    offline,
    shell,
    sheet,
    engine: {},
    auth: { user: null },
    channel: () => bus(),
  } as unknown as Platform;
  return { p, opened, shell };
}

describe('joinWindowSession', () => {
  it('opens the handed-over document once and keeps the rest of the shell', async () => {
    const offline = memoryCache();
    await stashWindowDocument(offline, 'w1', { bytes: new Uint8Array([9, 9]), name: 'Sổ.xlsx' });
    const bus = hub();
    const { p, shell } = sheetPlatform(offline, bus);
    const s = await joinWindowSession({
      platform: p,
      app: 'sheet',
      id: 'w1',
      replica: async () => replica(3),
      settleMs: 1,
    });
    expect(s).not.toBeNull();
    expect(s!.file.name).toBe('Sổ.xlsx');
    const got: OpenedFile[] = [];
    const off = s!.platform.shell.onOpenRequest((f) => got.push(f));
    await tick();
    expect(got).toEqual([{ bytes: new Uint8Array([9, 9]), name: 'Sổ.xlsx' }]);
    off();
    // Later subscribers get the platform's own requests, not the file again.
    s!.platform.shell.onOpenRequest(() => {});
    expect(shell.onOpenRequest).toHaveBeenCalledTimes(1);
    // Other shell services still work (settings: status bar, ribbon customisation).
    expect(await s!.platform.shell.getSetting?.('k')).toBe('value of k');
    // The sheet host is the collaborative one.
    expect(s!.platform.sheet).not.toBe(p.sheet);
    s!.stop();
  });

  it('an unknown session opens nothing', async () => {
    const { p } = sheetPlatform(memoryCache(), hub());
    expect(
      await joinWindowSession({
        platform: p,
        app: 'sheet',
        id: 'gone',
        replica: async () => replica(1),
      }),
    ).toBeNull();
  });

  it('platformWithFile gives the file to the first subscriber only', async () => {
    const { p, shell } = sheetPlatform(memoryCache(), hub());
    const file = { bytes: new Uint8Array([1]), name: 'a.docx' };
    const w = platformWithFile(p, file);
    const first = vi.fn();
    w.shell.onOpenRequest(first);
    w.shell.onOpenRequest(vi.fn());
    await tick();
    expect(first).toHaveBeenCalledWith(file);
    expect(shell.onOpenRequest).toHaveBeenCalledTimes(1);
  });
});
