/**
 * Two real `bo-collab` replicas (WASM) behind a fake relay: a cell typed in
 * one BlueSheet reaches the other, concurrent edits converge.
 */
import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { beforeAll, describe, expect, it } from 'vitest';
import { initSync, WasmCollab } from '@blueoffice/engine-wasm';
import { cellKey, collabSheetHost, parseCellKey, unpack, wasmEngine, type SheetLike } from '../src';

beforeAll(() => {
  const root = join(__dirname, '..', '..', '..');
  initSync({
    module: readFileSync(join(root, 'packages', 'engine-wasm', 'pkg', 'bo_wasm_bg.wasm')),
  });
});

/** In-memory sheet host: one sheet "Bảng 1". */
function memoryHost(): SheetLike & { cells: Map<string, string> } {
  const cells = new Map<string, string>();
  return {
    cells,
    info: async () => ({ sheets: [{ name: 'Bảng 1', lastRow: 1, lastCol: 1 }] }),
    setInputs: async (_s, list) => {
      for (const [r, c, v] of list) {
        if (v) cells.set(cellKey(r, c), v);
        else cells.delete(cellKey(r, c));
      }
    },
    exec: async <T>() => null as T,
    cell: async (_s, r, c) => ({ input: cells.get(cellKey(r, c)) ?? '' }),
  };
}

function peer(id: number) {
  const replica = new WasmCollab(id);
  const engine = wasmEngine(replica);
  const host = memoryHost();
  const outbox: Uint8Array[] = [];
  const shared = collabSheetHost(host, replica, () => outbox.push(...engine.drainOutbox()));
  return { replica, engine, host, shared, outbox };
}

type Peer = ReturnType<typeof peer>;

/** Delivers every queued frame of `from` to `to` (and the replies back). */
async function relay(from: Peer, to: Peer) {
  const frames = from.outbox.splice(0);
  for (const f of frames) {
    for (const reply of to.engine.receive(f)) from.engine.receive(reply);
  }
  await to.shared.remoteApplied();
}

async function connect(a: Peer, b: Peer) {
  for (const r of b.engine.receive(a.engine.start())) a.engine.receive(r);
  for (const r of a.engine.receive(b.engine.start())) b.engine.receive(r);
  await a.shared.remoteApplied();
  await b.shared.remoteApplied();
}

describe('BlueSheet collaboration over bo-collab', () => {
  it('packs and parses helpers', () => {
    expect(unpack(new Uint8Array([2, 0, 0, 0, 7, 8, 0, 0, 0, 0]))).toEqual([
      new Uint8Array([7, 8]),
      new Uint8Array([]),
    ]);
    expect(cellKey(9, 27)).toBe('AB10');
    expect(parseCellKey('AB10')).toEqual([9, 27]);
    expect(parseCellKey('A0x')).toBeNull();
  });

  it('shares typed cells and seeds late joiners', async () => {
    const a = peer(1);
    const b = peer(2);
    a.host.cells.set('A1', 'Có sẵn');
    await a.shared.seed();
    a.outbox.length = 0;
    await connect(a, b);
    expect(b.host.cells.get('A1')).toBe('Có sẵn');

    let redraws = 0;
    b.shared.onExternalChange?.(() => redraws++);
    await a.shared.setInputs(0, [[1, 0, '=SUM(A1:A1)']]);
    await relay(a, b);
    expect(b.host.cells.get('A2')).toBe('=SUM(A1:A1)');
    expect(redraws).toBe(1);

    // Clearing a cell propagates too.
    await b.shared.setInputs(0, [[0, 0, '']]);
    await relay(b, a);
    expect(a.host.cells.has('A1')).toBe(false);
  });

  it('concurrent edits of different and same cells converge', async () => {
    const a = peer(11);
    const b = peer(12);
    await connect(a, b);
    await a.shared.setInputs(0, [
      [0, 0, 'A'],
      [0, 1, 'x'],
    ]);
    await b.shared.setInputs(0, [
      [0, 0, 'B'],
      [1, 1, 'y'],
    ]);
    await relay(a, b);
    await relay(b, a);
    expect([...a.host.cells.entries()].sort()).toEqual([...b.host.cells.entries()].sort());
    expect(a.host.cells.get('B1')).toBe('x');
    expect(a.host.cells.get('B2')).toBe('y');
    expect(['A', 'B']).toContain(a.host.cells.get('A1'));
  });

  it('shares presence', async () => {
    const a = peer(21);
    const b = peer(22);
    await connect(a, b);
    a.replica.setPresence(
      JSON.stringify({ userId: 'u1', name: 'Trần Thị Bình', color: '#2b6cb0', cell: 'Bảng 1!B2' }),
    );
    a.outbox.push(...a.engine.drainOutbox());
    await relay(a, b);
    const peers = JSON.parse(b.replica.peersJson()) as [number, { name: string; cell: string }][];
    expect(peers).toEqual([
      [21, expect.objectContaining({ name: 'Trần Thị Bình', cell: 'Bảng 1!B2' })],
    ]);
  });
});
