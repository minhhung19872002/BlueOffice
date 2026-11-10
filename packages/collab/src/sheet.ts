/**
 * BlueSheet ↔ `bo-collab` binding. With an engine that implements the
 * collaboration binding (bo_engine::sheet::collab) the whole workbook is
 * shared live — values, formulas with references as stable row/column ids,
 * formats, row/column inserts and deletes, sizes, merges, sheets, rules,
 * charts and pivots — and undo goes through the replica. A host without it
 * shares cell inputs only (last writer wins per cell).
 */
import type { CollabEngine } from './index';
import { isEmptyChange, type ChangeLike, type CollabUser } from './live';
import { cellPresence, sheetPeersOf, type SheetPeerView } from './presence';

/** The `WasmCollab` surface used here (crates/bo-wasm/src/collab.rs). */
export interface WasmCollabLike {
  setTime(unixMs: number): void;
  start(): Uint8Array;
  receive(frame: Uint8Array): Uint8Array;
  drainOutbox(): Uint8Array;
  snapshot?(): Uint8Array;
  setCell(sheet: string, cell: string, input: string): void;
  cellsJson(): string;
  setPresence(json: string): void;
  peersJson(): string;
  clientId?(): number;
  applyLocal?(json: string): void;
  seed?(json: string): void;
  takeRemote?(): string;
  fullState?(): string;
  undo?(): boolean;
  redo?(): boolean;
  stopCapturing?(): void;
}

export interface Presence {
  userId: string;
  name: string;
  color: string;
  selection?: [number, number];
  cell?: string;
  shape?: string;
}

/** Splits `[u32 LE length][bytes]…` frame packs. */
export function unpack(packed: Uint8Array): Uint8Array[] {
  const out: Uint8Array[] = [];
  const view = new DataView(packed.buffer, packed.byteOffset, packed.byteLength);
  let i = 0;
  while (i + 4 <= packed.length) {
    const len = view.getUint32(i, true);
    i += 4;
    if (i + len > packed.length) break;
    out.push(packed.slice(i, i + len));
    i += len;
  }
  return out;
}

/** Adapts a WASM replica to {@link CollabEngine}. */
export function wasmEngine(w: WasmCollabLike): CollabEngine {
  return {
    start: () => {
      w.setTime(Date.now());
      return w.start();
    },
    receive: (frame) => {
      w.setTime(Date.now());
      return unpack(w.receive(frame));
    },
    drainOutbox: () => unpack(w.drainOutbox()),
    ...(w.snapshot ? { snapshot: () => w.snapshot!() } : {}),
  };
}

/** Minimal sheet host surface (subset of `SheetHost` in @blueoffice/platform). */
export interface SheetLike {
  info(): Promise<{ sheets: { name: string }[] }>;
  setInputs(sheet: number, cells: [number, number, string][]): Promise<void>;
  exec<T = unknown>(cmd: { cmd: string; [k: string]: unknown }): Promise<T>;
  cell(sheet: number, row: number, col: number): Promise<{ input: string }>;
  onExternalChange?(listener: () => void): () => void;
  structural?(
    sheet: number,
    rows: boolean,
    at: number,
    count: number,
    remove: boolean,
  ): Promise<void>;
  addSheet?(name?: string): Promise<number>;
  renameSheet?(sheet: number, name: string): Promise<void>;
  deleteSheet?(sheet: number): Promise<void>;
  undo?(): Promise<boolean>;
  redo?(): Promise<boolean>;
  collabStart?(seed: boolean, clientId: number): Promise<unknown>;
  collabLocal?(): Promise<unknown>;
  collabApply?(change: string): Promise<unknown>;
}

const colName = (col: number): string => {
  let n = col + 1;
  let s = '';
  while (n > 0) {
    const m = (n - 1) % 26;
    s = String.fromCharCode(65 + m) + s;
    n = Math.floor((n - 1) / 26);
  }
  return s;
};

export const cellKey = (row: number, col: number): string => `${colName(col)}${row + 1}`;

export function parseCellKey(a1: string): [number, number] | null {
  const m = /^([A-Z]{1,3})([0-9]{1,7})$/.exec(a1);
  if (!m) return null;
  let col = 0;
  for (const ch of m[1]!) col = col * 26 + (ch.charCodeAt(0) - 64);
  return [Number(m[2]) - 1, col - 1];
}

/**
 * Wraps a sheet host so its edits are shared through `replica`.
 *
 * Hosts with the engine binding (`collabStart` / `collabLocal` /
 * `collabApply`, bo_engine::sheet::collab) share everything: cell values and
 * formulas (references as stable row/column ids), formats, row/column
 * inserts and deletes, sizes, merges, sheets, rules, charts, pivots; undo and
 * redo go through the replica (only this user's changes). Other hosts share
 * cell inputs only (last writer wins per cell).
 *
 * Call `remoteApplied()` after the collab client reports remote frames and
 * `seed()` when the server asks this editor to publish the file.
 *
 * With `user`, presence is shared too: `collabSelect(sheet, range)` publishes
 * this user's selection and `onSheetPeers` reports the collaborators' ones.
 */
export function collabSheetHost<H extends SheetLike>(
  host: H,
  replica: WasmCollabLike,
  flush: () => void,
  user?: CollabUser,
): Wrapped<H> {
  const w =
    host.collabStart && host.collabLocal && host.collabApply && replica.applyLocal
      ? engineSheetHost(host, replica, flush)
      : cellSheetHost(host, replica, flush);
  return withPresence(w, replica, flush, user);
}

/** What the wrapped sheet host adds. */
export interface SheetCollab {
  remoteApplied(): Promise<void>;
  seed(): Promise<void>;
  dispose(): void;
  /** Publishes this user's selected cell / range. */
  collabSelect(sheet: string, range: string): void;
  /** Collaborators' selected cells / ranges, on every remote change and presence update. */
  onSheetPeers(listener: (peers: SheetPeerView[]) => void): () => void;
}

type Wrapped<H> = H & SheetCollab;
type Bound<H> = H & { remoteApplied(): Promise<void>; seed(): Promise<void>; dispose(): void };

/** Adds selection presence (awareness `cell`) to a wrapped sheet host. */
function withPresence<H>(
  w: Bound<H>,
  replica: WasmCollabLike,
  flush: () => void,
  user: CollabUser | undefined,
): Wrapped<H> {
  const listeners = new Set<(peers: SheetPeerView[]) => void>();
  let last = '';
  let lastPeers = '[]';
  const report = () => {
    if (!listeners.size) return;
    const json = replica.peersJson();
    if (json === lastPeers) return;
    lastPeers = json;
    const peers = sheetPeersOf(json);
    listeners.forEach((l) => l(peers));
  };
  const remoteApplied = w.remoteApplied;
  const dispose = w.dispose;
  const out = w as Wrapped<H>;
  out.remoteApplied = async () => {
    await remoteApplied();
    report();
  };
  out.collabSelect = (sheet: string, range: string) => {
    if (!user) return;
    const cell = cellPresence(sheet, range);
    if (cell === last) return;
    last = cell;
    const p: Presence = { ...user, cell };
    replica.setPresence(JSON.stringify(p));
    flush();
  };
  out.onSheetPeers = (l) => {
    listeners.add(l);
    l(sheetPeersOf(replica.peersJson()));
    return () => listeners.delete(l);
  };
  out.dispose = () => {
    listeners.clear();
    dispose();
  };
  return out;
}

function engineSheetHost<H extends SheetLike>(
  host: H,
  replica: WasmCollabLike,
  flush: () => void,
): Bound<H> {
  const listeners = new Set<() => void>();
  let phase: 'closed' | 'live' = 'closed';
  let tail: Promise<unknown> = Promise.resolve();
  /** Runs engine work one task at a time (local edits and remote changes keep their order). */
  const run = <T>(f: () => Promise<T>): Promise<T> => {
    const p = tail.then(f, f);
    tail = p.catch(() => undefined);
    return p;
  };
  const empty = (json: string) => !json || isEmptyChange(JSON.parse(json) as ChangeLike);
  const share = async () => {
    if (phase !== 'live' || !host.collabLocal) return;
    const json = JSON.stringify(await host.collabLocal());
    if (!empty(json)) {
      // Every sheet command is its own undo step.
      replica.stopCapturing?.();
      replica.applyLocal?.(json);
    }
    flush();
  };
  const applyRemote = async (json: string) => {
    if (empty(json) || !host.collabApply) return;
    await host.collabApply(json);
    // Applying never creates local changes; anything pending is shared.
    await share();
    listeners.forEach((l) => l());
  };
  const bindJoiner = async () => {
    if (phase === 'live' || !host.collabStart) return;
    await host.collabStart(false, replica.clientId?.() ?? 0);
    phase = 'live';
    await applyRemote(replica.fullState?.() ?? '');
  };

  const wrapped = Object.create(host) as Bound<H>;
  const local =
    <A extends unknown[], R>(f: (...a: A) => Promise<R>) =>
    (...a: A): Promise<R> =>
      run(async () => {
        const r = await f.apply(host, a);
        await share();
        return r;
      });
  wrapped.setInputs = local(host.setInputs);
  wrapped.exec = local(host.exec) as H['exec'];
  if (host.structural) wrapped.structural = local(host.structural);
  if (host.addSheet) wrapped.addSheet = local(host.addSheet);
  if (host.renameSheet) wrapped.renameSheet = local(host.renameSheet);
  if (host.deleteSheet) wrapped.deleteSheet = local(host.deleteSheet);
  const history = (redo: boolean) => () =>
    run(async () => {
      if (phase !== 'live' || !replica.undo || !replica.redo) {
        return (redo ? host.redo?.() : host.undo?.()) ?? false;
      }
      const done = redo ? replica.redo() : replica.undo();
      if (done) await applyRemote(replica.takeRemote?.() ?? '');
      flush();
      return done;
    });
  if (host.undo) wrapped.undo = history(false);
  if (host.redo) wrapped.redo = history(true);
  wrapped.onExternalChange = (l: () => void) => {
    listeners.add(l);
    return () => listeners.delete(l);
  };
  wrapped.remoteApplied = () =>
    run(async () => {
      if (phase !== 'live') return bindJoiner();
      await applyRemote(replica.takeRemote?.() ?? '');
    });
  wrapped.seed = () =>
    run(async () => {
      if (!host.collabStart) return;
      // Seed only an empty room (a joiner bound before the room filled up re-binds).
      if (!empty(replica.fullState?.() ?? '')) {
        if (phase !== 'live') await bindJoiner();
        return;
      }
      const change = await host.collabStart(true, replica.clientId?.() ?? 0);
      phase = 'live';
      const json = JSON.stringify(change);
      if (!empty(json)) replica.seed?.(json);
      flush();
    });
  wrapped.dispose = () => {
    listeners.clear();
    phase = 'closed';
  };
  return wrapped;
}

function cellSheetHost<H extends SheetLike>(
  host: H,
  replica: WasmCollabLike,
  flush: () => void,
): Bound<H> {
  const listeners = new Set<() => void>();
  let known = new Map<string, string>(
    Object.entries(JSON.parse(replica.cellsJson()) as Record<string, string>),
  );
  let applying = false;

  const sheetName = async (i: number) => (await host.info()).sheets[i]?.name ?? `Sheet${i + 1}`;

  const share = async (sheet: number, cells: [number, number, string][]) => {
    if (applying) return;
    const name = await sheetName(sheet);
    for (const [r, c, input] of cells) {
      const key = `${name}!${cellKey(r, c)}`;
      replica.setCell(name, cellKey(r, c), input);
      if (input) known.set(key, input);
      else known.delete(key);
    }
    flush();
  };

  const wrapped = Object.create(host) as Bound<H>;
  wrapped.setInputs = async (sheet, cells) => {
    await host.setInputs(sheet, cells);
    await share(sheet, cells);
  };
  wrapped.exec = (async (cmd: { cmd: string; [k: string]: unknown }) => {
    const r = await host.exec(cmd);
    if (cmd.cmd === 'setInputs' && Array.isArray(cmd.cells) && typeof cmd.sheet === 'number') {
      await share(cmd.sheet, cmd.cells as [number, number, string][]);
    }
    return r;
  }) as H['exec'];
  wrapped.onExternalChange = (l: () => void) => {
    listeners.add(l);
    return () => listeners.delete(l);
  };

  /** Applies the replica's cell map to the engine (diff against what we know). */
  wrapped.remoteApplied = async () => {
    const now = new Map(Object.entries(JSON.parse(replica.cellsJson()) as Record<string, string>));
    const sheets = (await host.info()).sheets.map((s) => s.name);
    const bySheet = new Map<number, [number, number, string][]>();
    const push = (key: string, input: string) => {
      const bang = key.lastIndexOf('!');
      const idx = sheets.indexOf(key.slice(0, bang));
      const pos = parseCellKey(key.slice(bang + 1));
      if (idx < 0 || !pos) return;
      const list = bySheet.get(idx) ?? [];
      list.push([pos[0], pos[1], input]);
      bySheet.set(idx, list);
    };
    for (const [k, v] of now) if (known.get(k) !== v) push(k, v);
    for (const k of known.keys()) if (!now.has(k)) push(k, '');
    known = now;
    if (!bySheet.size) return;
    applying = true;
    try {
      for (const [sheet, cells] of bySheet) await host.setInputs(sheet, cells);
    } finally {
      applying = false;
    }
    listeners.forEach((l) => l());
  };

  /** First editor in the session: publishes the file's cells (server `seed`). */
  wrapped.seed = async () => {
    const info = await host.info();
    for (let i = 0; i < info.sheets.length; i++) {
      const s = info.sheets[i] as {
        name: string;
        lastRow?: number | null;
        lastCol?: number | null;
      };
      for (let r = 0; r <= (s.lastRow ?? -1); r++) {
        for (let c = 0; c <= (s.lastCol ?? -1); c++) {
          const { input } = await host.cell(i, r, c);
          if (input) {
            replica.setCell(s.name, cellKey(r, c), input);
            known.set(`${s.name}!${cellKey(r, c)}`, input);
          }
        }
      }
    }
    flush();
  };
  wrapped.dispose = () => listeners.clear();
  return wrapped;
}
