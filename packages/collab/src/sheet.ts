/**
 * BlueSheet ↔ `bo-collab` binding: cell inputs (values and formulas) are
 * shared live through the replica's `cells` Y.Map (last writer wins per cell,
 * convergent). Formatting and structural edits are not shared live; they
 * reach collaborators with the next saved version.
 */
import type { CollabEngine } from './index';

/** The `WasmCollab` surface used here (crates/bo-wasm/src/collab.rs). */
export interface WasmCollabLike {
  setTime(unixMs: number): void;
  start(): Uint8Array;
  receive(frame: Uint8Array): Uint8Array;
  drainOutbox(): Uint8Array;
  setCell(sheet: string, cell: string, input: string): void;
  cellsJson(): string;
  setPresence(json: string): void;
  peersJson(): string;
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
  };
}

/** Minimal sheet host surface (subset of `SheetHost` in @blueoffice/platform). */
export interface SheetLike {
  info(): Promise<{ sheets: { name: string }[] }>;
  setInputs(sheet: number, cells: [number, number, string][]): Promise<void>;
  exec<T = unknown>(cmd: { cmd: string; [k: string]: unknown }): Promise<T>;
  cell(sheet: number, row: number, col: number): Promise<{ input: string }>;
  onExternalChange?(listener: () => void): () => void;
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
 * Wraps a sheet host so local cell edits go into the replica, and remote cell
 * changes are written into the engine and announced via `onExternalChange`.
 * Call `remoteApplied()` after the collab client reports remote frames and
 * `flush` after local edits (the client's `flush`).
 */
export function collabSheetHost<H extends SheetLike>(
  host: H,
  replica: WasmCollabLike,
  flush: () => void,
): H & { remoteApplied(): Promise<void>; seed(): Promise<void>; dispose(): void } {
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

  const wrapped = Object.create(host) as H & {
    remoteApplied(): Promise<void>;
    seed(): Promise<void>;
    dispose(): void;
  };
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
