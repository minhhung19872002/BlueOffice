/**
 * BlueSheet over the engine binding (bo_engine::sheet::collab): two real
 * WASM sheet engines and replicas behind a fake relay. Row inserts with a
 * concurrent formula, formats, sheets and undo converge.
 */
import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { beforeAll, describe, expect, it } from 'vitest';
import { initSync, WasmCollab, WasmSheet } from '@blueoffice/engine-wasm';
import { collabSheetHost, wasmEngine, type SheetLike } from '../src';

beforeAll(() => {
  const root = join(__dirname, '..', '..', '..');
  initSync({
    module: readFileSync(join(root, 'packages', 'engine-wasm', 'pkg', 'bo_wasm_bg.wasm')),
  });
});

const parse = <T>(s: string): T => JSON.parse(s) as T;

function wasmSheetHost(cells: [number, number, string][]): SheetLike {
  const e = new WasmSheet();
  e.setLocale('en-US');
  e.newWorkbook('chung.xlsx');
  e.setInputs(0, JSON.stringify(cells));
  return {
    info: async () => parse(e.info()),
    setInputs: async (s, list) => e.setInputs(s, JSON.stringify(list)),
    exec: async <T>(cmd: { cmd: string }) => parse<T>(e.exec(JSON.stringify(cmd))),
    cell: async (s, r, c) => parse(e.cell(s, r, c)),
    structural: async (s, rows, at, n, del) => e.structural(s, rows, at, n, del),
    addSheet: async (n?: string) => e.addSheet(n),
    renameSheet: async (s, n) => e.renameSheet(s, n),
    deleteSheet: async (s) => e.deleteSheet(s),
    undo: async () => e.undo(),
    redo: async () => e.redo(),
    collabStart: async (seed, id) => parse(e.collabStart(seed, id)),
    collabLocal: async () => parse(e.collabLocal()),
    collabApply: async (j) => parse(e.collabApply(j)),
  };
}

function peer(id: number, cells: [number, number, string][]) {
  const replica = new WasmCollab(id);
  const engine = wasmEngine(replica);
  const outbox: Uint8Array[] = [];
  const shared = collabSheetHost(wasmSheetHost(cells), replica, () =>
    outbox.push(...engine.drainOutbox()),
  );
  return { replica, engine, shared, outbox };
}

type Peer = ReturnType<typeof peer>;

async function relay(from: Peer, to: Peer) {
  for (const f of from.outbox.splice(0)) {
    for (const reply of to.engine.receive(f)) from.engine.receive(reply);
  }
  await to.shared.remoteApplied();
}

async function settle(a: Peer, b: Peer) {
  for (let i = 0; i < 3; i++) {
    await relay(a, b);
    await relay(b, a);
  }
}

const input = async (p: Peer, r: number, c: number) => (await p.shared.cell(0, r, c)).input;

describe('BlueSheet engine binding', () => {
  it('shares structure, formulas by row id, formats, sheets and undo', async () => {
    const a = peer(31, [
      [0, 0, '1'],
      [1, 0, '2'],
      [2, 0, '3'],
    ]);
    const b = peer(32, [[5, 5, 'khác']]);
    await a.shared.seed();
    for (const r of b.engine.receive(a.engine.start())) a.engine.receive(r);
    for (const r of a.engine.receive(b.engine.start())) b.engine.receive(r);
    await settle(a, b);
    expect(await input(b, 2, 0)).toBe('3');
    expect(await input(b, 5, 5)).toBe('');

    // A inserts a row above row 2 while B writes =A3*10.
    await a.shared.structural!(0, true, 1, 1, false);
    await b.shared.setInputs(0, [[0, 1, '=A3*10']]);
    await settle(a, b);
    expect(await input(a, 0, 1)).toBe('=A4*10');
    expect(await input(b, 0, 1)).toBe('=A4*10');
    expect((await b.shared.cell(0, 0, 1)) as unknown).toMatchObject({ display: '30' });

    // Formats and sheets.
    await a.shared.exec({
      cmd: 'format',
      sheet: 0,
      range: { r0: 0, c0: 0, r1: 0, c1: 0 },
      patch: { bold: true },
    });
    await b.shared.addSheet!('Chi');
    await settle(a, b);
    const names = async (p: Peer) =>
      ((await p.shared.info()) as { sheets: { name: string }[] }).sheets.map((s) => s.name);
    expect(await names(a)).toEqual(await names(b));
    expect(await names(a)).toContain('Chi');

    // B's undo reverts B's own edits only (the sheet it added).
    expect(await b.shared.undo!()).toBe(true);
    await settle(a, b);
    expect(await names(a)).not.toContain('Chi');
    expect(await input(a, 0, 1)).toBe('=A4*10');
  });
});
