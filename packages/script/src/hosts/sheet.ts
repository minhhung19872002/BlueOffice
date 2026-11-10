/**
 * BlueSheet side of BlueScript (CORE-SCRIPT-001): `workbook` / `sheet` / ranges
 * translated into `SheetHost` calls (`SheetCommand` JSON). Pass the host the
 * editor uses — while collaborating that is the `collabSheetHost` wrapper, so
 * script edits are shared like typed ones.
 */
import type {
  BorderPreset,
  SheetCommand,
  SheetHost,
  SheetRange,
  StylePatch,
} from '@blueoffice/platform';
import {
  dispatcher,
  hex,
  int,
  obj,
  rawCommand,
  ScriptApiError,
  str,
  type ScriptHost,
} from '../host';

/** What the script host needs from a sheet host. */
export type SheetScriptTarget = Pick<SheetHost, 'info' | 'cell' | 'exec'> &
  Partial<Pick<SheetHost, 'addSheet' | 'renameSheet'>>;

export interface SheetScriptHostOptions {
  host: SheetScriptTarget;
  /** The sheet shown when the script starts (`sheet` in scripts). */
  activeSheet: () => number;
  /** Decimal separator of the input locale (numbers are entered as typed). */
  decimal?: ',' | '.';
  /** After the run (the editor re-reads the workbook and redraws). */
  onEnd?: () => void | Promise<void>;
}

/** Cells a single call may read or write. */
export const MAX_RANGE_CELLS = 250_000;

export type CellValue = string | number | boolean | null;

const BORDERS: readonly BorderPreset[] = [
  'outline',
  'all',
  'inside',
  'none',
  'top',
  'bottom',
  'left',
  'right',
];

const PATCH_KEYS: readonly (keyof StylePatch)[] = [
  'numFmt',
  'fontName',
  'fontSize',
  'bold',
  'italic',
  'underline',
  'strike',
  'fontColor',
  'fillColor',
  'hAlign',
  'vAlign',
  'wrap',
  'indent',
  'rotation',
  'shrink',
  'locked',
  'formulaHidden',
];

/** `B`, `AA` → 0-based column. */
function colIndex(letters: string): number {
  let n = 0;
  for (const ch of letters.toUpperCase()) n = n * 26 + (ch.charCodeAt(0) - 64);
  return n - 1;
}

/** `A1`, `$A$1:B3`, `Sheet1!A1:B3` → sheet name (if any) and 0-based range. */
export function parseAddress(address: string): { sheet?: string; range: SheetRange } {
  let a = address.trim();
  let sheet: string | undefined;
  const bang = a.lastIndexOf('!');
  if (bang >= 0) {
    sheet = a
      .slice(0, bang)
      .replace(/^'(.*)'$/, '$1')
      .replace(/''/g, "'");
    a = a.slice(bang + 1);
  }
  const m = /^\$?([A-Za-z]{1,3})\$?(\d{1,7})(?::\$?([A-Za-z]{1,3})\$?(\d{1,7}))?$/.exec(a);
  if (!m) throw new ScriptApiError(`bad range address "${address}"`);
  const c0 = colIndex(m[1]!);
  const r0 = Number(m[2]) - 1;
  const c1 = m[3] ? colIndex(m[3]) : c0;
  const r1 = m[4] ? Number(m[4]) - 1 : r0;
  if (r0 < 0 || r1 < 0 || c0 > 16383 || c1 > 16383) {
    throw new ScriptApiError(`bad range address "${address}"`);
  }
  const range = {
    r0: Math.min(r0, r1),
    c0: Math.min(c0, c1),
    r1: Math.max(r0, r1),
    c1: Math.max(c0, c1),
  };
  return sheet === undefined ? { range } : { sheet, range };
}

const cellCount = (r: SheetRange) => (r.r1 - r.r0 + 1) * (r.c1 - r.c0 + 1);

export function sheetScriptHost({
  host,
  activeSheet,
  decimal = ',',
  onEnd,
}: SheetScriptHostOptions): ScriptHost {
  let depth: number | null = null;

  const sheets = async () =>
    (await host.info()).sheets.map((s, index) => ({ index, name: s.name }));
  const sheetIndex = async (v: unknown, named?: string): Promise<number> => {
    const list = await sheets();
    if (named !== undefined) {
      const hit = list.find((s) => s.name.toLowerCase() === named.toLowerCase());
      if (!hit) throw new ScriptApiError(`no sheet "${named}"`);
      return hit.index;
    }
    const i = v === -1 || v === null || v === undefined ? activeSheet() : int(v, 'sheet');
    if (!list[i]) throw new ScriptApiError(`no sheet ${i}`);
    return i;
  };
  const target = async (s: unknown, address: unknown, max = MAX_RANGE_CELLS) => {
    const { sheet: named, range } = parseAddress(str(address, 'address'));
    if (cellCount(range) > max) throw new ScriptApiError(`range has more than ${max} cells`);
    return { sheet: await sheetIndex(s, named), range };
  };
  const exec = <T = unknown>(cmd: SheetCommand) => host.exec<T>(cmd);
  const toInput = (v: unknown): string => {
    if (v === null || v === undefined) return '';
    if (typeof v === 'number') {
      if (!Number.isFinite(v)) throw new ScriptApiError('values: numbers must be finite');
      return decimal === ',' ? String(v).replace('.', ',') : String(v);
    }
    if (typeof v === 'boolean') return v ? 'TRUE' : 'FALSE';
    if (typeof v === 'string') return v;
    throw new ScriptApiError('values: expected strings, numbers, booleans or null');
  };
  /** A cell input or display text as a value (`grouped`: display text with digit grouping). */
  const parseValue = (text: string, grouped = false): CellValue => {
    if (text === '') return null;
    const group = decimal === ',' ? '.' : ',';
    const t = (grouped ? text.split(group).join('') : text).replace(decimal, '.').trim();
    if (/^[-+]?(\d+\.?\d*|\.\d+)(e[-+]?\d+)?$/i.test(t)) return Number(t);
    if (text === 'TRUE') return true;
    if (text === 'FALSE') return false;
    return text;
  };
  const read = async (s: unknown, address: unknown, formulas: boolean) => {
    const { sheet, range } = await target(s, address);
    const rows: (CellValue | string)[][] = [];
    for (let r = range.r0; r <= range.r1; r++) {
      const row: (CellValue | string)[] = [];
      for (let c = range.c0; c <= range.c1; c++) {
        const d = await host.cell(sheet, r, c);
        if (formulas) row.push(d.input);
        else if (d.input.startsWith('=')) row.push(parseValue(d.display, true));
        else if (d.input.startsWith("'")) row.push(d.input.slice(1));
        else row.push(parseValue(d.input) ?? (d.display === '' ? null : d.display));
      }
      rows.push(row);
    }
    return rows;
  };
  const write = async (s: unknown, address: unknown, values: unknown) => {
    const { sheet, range } = await target(s, address);
    if (!Array.isArray(values))
      throw new ScriptApiError('values: expected rows (an array of arrays)');
    const cells: [number, number, string][] = [];
    values.forEach((row: unknown, i) => {
      const list: unknown[] = Array.isArray(row) ? row : [row];
      list.forEach((v, j) => cells.push([range.r0 + i, range.c0 + j, toInput(v)]));
    });
    if (cells.length > MAX_RANGE_CELLS) throw new ScriptApiError('too many cells');
    if (cells.length) await exec({ cmd: 'setInputs', sheet, cells });
    return cells.length;
  };

  const call = dispatcher({
    'sheet.list': () => sheets(),
    'sheet.active': async () => {
      const i = await sheetIndex(-1);
      return (await sheets())[i];
    },
    'sheet.add': async (name) => {
      if (!host.addSheet) throw new ScriptApiError('cannot add sheets here');
      const index = await host.addSheet(
        name === null || name === undefined ? undefined : str(name, 'name'),
      );
      return (await sheets())[index];
    },
    'sheet.rename': async (s, name) => {
      if (!host.renameSheet) throw new ScriptApiError('cannot rename sheets here');
      await host.renameSheet(await sheetIndex(s), str(name, 'name'));
    },
    'sheet.getValues': (s, a) => read(s, a, false),
    'sheet.getFormulas': (s, a) => read(s, a, true),
    'sheet.setValues': (s, a, v) => write(s, a, v),
    'sheet.setFormulas': (s, a, v) => write(s, a, v),
    'sheet.fill': async (s, a, v) => {
      const { sheet, range } = await target(s, a);
      const input = toInput(v);
      const cells: [number, number, string][] = [];
      for (let r = range.r0; r <= range.r1; r++) {
        for (let c = range.c0; c <= range.c1; c++) cells.push([r, c, input]);
      }
      await exec({ cmd: 'setInputs', sheet, cells });
    },
    'sheet.format': async (s, a, p) => {
      const { sheet, range } = await target(s, a, Number.MAX_SAFE_INTEGER);
      const o = obj(p, 'format');
      const patch: Record<string, unknown> = {};
      for (const k of PATCH_KEYS) {
        if (o[k] === undefined) continue;
        patch[k] = k === 'fontColor' || k === 'fillColor' ? hex(o[k], k) : o[k];
      }
      await exec({ cmd: 'format', sheet, range, patch: patch as StylePatch });
    },
    'sheet.clear': async (s, a, what) => {
      const { sheet, range } = await target(s, a);
      const w = str(what ?? 'all', 'what');
      if (w !== 'all' && w !== 'contents' && w !== 'formats') {
        throw new ScriptApiError('clear: all, contents or formats');
      }
      if (w !== 'formats') {
        const cells: [number, number, string][] = [];
        for (let r = range.r0; r <= range.r1; r++) {
          for (let c = range.c0; c <= range.c1; c++) cells.push([r, c, '']);
        }
        await exec({ cmd: 'setInputs', sheet, cells });
      }
      if (w !== 'contents') await exec({ cmd: 'clearFormats', sheet, range });
    },
    'sheet.merge': async (s, a, center) => {
      const { sheet, range } = await target(s, a, Number.MAX_SAFE_INTEGER);
      await exec({ cmd: 'merge', sheet, range, center: !!center });
    },
    'sheet.borders': async (s, a, preset, style, color) => {
      const { sheet, range } = await target(s, a, Number.MAX_SAFE_INTEGER);
      const p = str(preset, 'preset') as BorderPreset;
      if (!BORDERS.includes(p)) throw new ScriptApiError(`borders: one of ${BORDERS.join(', ')}`);
      await exec({
        cmd: 'borders',
        sheet,
        range,
        spec: {
          preset: p,
          ...(style === null || style === undefined ? {} : { style: str(style, 'style') }),
          ...(color === null || color === undefined ? {} : { color: hex(color, 'color') }),
        },
      });
    },
    'sheet.addChart': async (s, a, kind, _options) => {
      const { sheet, range } = await target(s, a, Number.MAX_SAFE_INTEGER);
      await exec({ cmd: 'insertChart', sheet, range, kind: str(kind, 'kind') });
      const charts = await exec<unknown[]>({ cmd: 'charts', sheet });
      return Array.isArray(charts) ? charts.length - 1 : null;
    },
    'sheet.charts': async (s) => exec({ cmd: 'charts', sheet: await sheetIndex(s) }),
    'sheet.command': (cmd) => exec(rawCommand<SheetCommand>(cmd)),
  });

  return {
    app: 'sheet',
    call,
    async begin() {
      const r = await host.exec<{ history?: number }>({ cmd: 'historyLen' }).catch(() => null);
      depth = typeof r?.history === 'number' ? r.history : null;
    },
    async end() {
      try {
        if (depth !== null) await host.exec({ cmd: 'mergeHistory', since: depth });
      } finally {
        depth = null;
        await onEnd?.();
      }
    },
  };
}
