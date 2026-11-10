/**
 * Presence (who is editing which cell / slide) without the WASM engine: a fake
 * replica records what is published on the awareness channel and returns the
 * collaborators' states.
 */
import { describe, expect, it } from 'vitest';
import {
  cellPresence,
  collabSheetHost,
  collabSlideHost,
  parseCellPresence,
  sheetPeersOf,
  slidePresence,
  type Presence,
  type ReplicaLike,
  type SheetLike,
  type SheetPeerView,
  type WasmCollabLike,
} from '../src';

function fakeReplica() {
  const published: Presence[] = [];
  let peers: [number, Presence][] = [];
  const r = {
    setTime: () => {},
    start: () => new Uint8Array(),
    receive: () => new Uint8Array(),
    drainOutbox: () => new Uint8Array(),
    setCell: () => {},
    cellsJson: () => '{}',
    setPresence: (json: string) => published.push(JSON.parse(json) as Presence),
    peersJson: () => JSON.stringify(peers),
    clientId: () => 7,
    applyLocal: () => {},
    seed: () => {},
    takeRemote: () => '',
    fullState: () => '{}',
    undo: () => false,
    redo: () => false,
    canUndo: () => false,
    canRedo: () => false,
  };
  return {
    replica: r as unknown as WasmCollabLike & ReplicaLike,
    published,
    setPeers: (p: [number, Presence][]) => {
      peers = p;
    },
  };
}

const user = { userId: 'u1', name: 'An', color: '#c2410c' };
const other = (cell?: string, shape?: string): Presence => ({
  userId: 'u2',
  name: 'Bình',
  color: '#15803d',
  ...(cell ? { cell } : {}),
  ...(shape ? { shape } : {}),
});

describe('presence helpers', () => {
  it('encodes and splits sheet selections (sheet names may hold "!")', () => {
    expect(cellPresence('Doanh thu!2026', 'B2:C4')).toBe('Doanh thu!2026!B2:C4');
    expect(parseCellPresence('Doanh thu!2026!B2:C4')).toEqual({
      sheet: 'Doanh thu!2026',
      range: 'B2:C4',
    });
    expect(parseCellPresence('B2')).toBeNull();
    expect(parseCellPresence('Sheet1!')).toBeNull();
    expect(sheetPeersOf('not json')).toEqual([]);
    expect(
      sheetPeersOf(
        JSON.stringify([
          [3, other('Sheet1!A1')],
          [4, other(undefined, 's1/2')],
        ]),
      ),
    ).toEqual([{ clientId: 3, name: 'Bình', color: '#15803d', sheet: 'Sheet1', range: 'A1' }]);
    expect(slidePresence('256')).toBe('256/');
  });
});

describe('BlueSheet presence', () => {
  const host = (): SheetLike => ({
    info: () => Promise.resolve({ sheets: [{ name: 'Sheet1' }] }),
    setInputs: () => Promise.resolve(),
    exec: <T>() => Promise.resolve(null as T),
    cell: () => Promise.resolve({ input: '' }),
  });

  it('publishes the own selection once per change and reports collaborators', async () => {
    const f = fakeReplica();
    let flushed = 0;
    const s = collabSheetHost(host(), f.replica, () => (flushed += 1), user);
    s.collabSelect('Sheet1', 'B2');
    s.collabSelect('Sheet1', 'B2');
    s.collabSelect('Sheet1', 'B2:C3');
    expect(f.published.map((p) => p.cell)).toEqual(['Sheet1!B2', 'Sheet1!B2:C3']);
    expect(f.published[0]).toMatchObject(user);
    expect(flushed).toBe(2);
    const seen: SheetPeerView[][] = [];
    const off = s.onSheetPeers((p) => seen.push(p));
    expect(seen).toEqual([[]]);
    f.setPeers([[9, other('Sheet1!D4:E5')]]);
    await s.remoteApplied();
    expect(seen.at(-1)).toEqual([
      { clientId: 9, name: 'Bình', color: '#15803d', sheet: 'Sheet1', range: 'D4:E5' },
    ]);
    // Unchanged presence is not reported again.
    await s.remoteApplied();
    expect(seen).toHaveLength(2);
    off();
    f.setPeers([]);
    await s.remoteApplied();
    expect(seen).toHaveLength(2);
  });

  it('shares nothing without a user', () => {
    const f = fakeReplica();
    const s = collabSheetHost(host(), f.replica, () => {});
    s.collabSelect('Sheet1', 'A1');
    expect(f.published).toEqual([]);
  });
});

describe('BlueSlide presence', () => {
  it('publishes the current slide when no object is selected and flags such peers', async () => {
    const f = fakeReplica();
    let keys: string[] = [];
    const engine = {
      open: (_bytes: Uint8Array, _name: string) => Promise.resolve({}),
      slideCommand: (cmd: { cmd: string }) =>
        Promise.resolve({
          canUndo: false,
          canRedo: false,
          collab:
            cmd.cmd === 'select'
              ? { change: {}, shape: '256/3', slide: '256' }
              : { change: {}, slide: '257' },
        }),
      slideInsertPicture: () => Promise.resolve({ canUndo: false, canRedo: false }),
      slideCollabStart: () => Promise.resolve({}),
      slideCollabApply: () => Promise.resolve({ canUndo: false, canRedo: false }),
      slideCollabPeers: (k: string[]) => {
        keys = k;
        return Promise.resolve(
          k.map((x) => ({ slide: x.startsWith('256') ? 0 : 1, rect: { cx: 1 } })),
        );
      },
    };
    const live = collabSlideHost(engine, f.replica, () => {}, user);
    await live.open(new Uint8Array(), 'a.pptx');
    await live.slideCommand({ cmd: 'goTo' });
    await live.slideCommand({ cmd: 'select' });
    expect(f.published.map((p) => p.shape)).toEqual(['257/', '256/3']);
    const views: { slide: number; slideOnly?: boolean }[][] = [];
    live.onSlidePeers((v) => views.push(v));
    await live.idle();
    f.setPeers([
      [3, other(undefined, '257/')],
      [4, other(undefined, '256/9')],
    ]);
    // A command refreshes the peers when someone is there.
    await live.slideCommand({ cmd: 'goTo' });
    expect(keys).toEqual(['257/', '256/9']);
    const last = views.at(-1)!;
    expect(last.map((v) => [v.slide, v.slideOnly ?? false])).toEqual([
      [1, true],
      [0, false],
    ]);
    live.dispose();
  });
});
