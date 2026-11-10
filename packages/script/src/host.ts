/**
 * What a BlueScript run talks to on the host side (CORE-SCRIPT-001): an
 * allow-listed dispatcher of API methods plus the begin/end hooks that make the
 * run one undo step and refresh the editor.
 */
import type { ScriptApp, ScriptCall } from './sandbox';

export interface ScriptHost {
  readonly app: ScriptApp;
  /** One API call; unknown methods reject. */
  call: ScriptCall;
  /** Before the script starts (remembers the undo history length). */
  begin?(): Promise<void>;
  /** After it ends, however it ends (joins its undo steps, redraws). */
  end?(): Promise<void>;
}

/** Error thrown back into the script. */
export class ScriptApiError extends Error {
  override name = 'ScriptApiError';
}

export type Handler = (...args: unknown[]) => Promise<unknown>;

/** Message of a value thrown by a host call, as the script sees it. */
export const errorText = (e: unknown): string =>
  e instanceof Error ? e.message : typeof e === 'string' ? e : (JSON.stringify(e) ?? String(e));

/** A host from a table of handlers: unknown methods are refused. */
export function dispatcher(table: Record<string, Handler>): ScriptCall {
  return async (method, args) => {
    if (!Object.prototype.hasOwnProperty.call(table, method)) {
      throw new ScriptApiError(`unknown API method ${method}`);
    }
    return table[method]!(...args);
  };
}

// ------------------------------------------------------------------ argument checks

export const int = (v: unknown, what: string, min = 0): number => {
  if (typeof v !== 'number' || !Number.isInteger(v) || v < min) {
    throw new ScriptApiError(`${what}: expected an integer ≥ ${min}`);
  }
  return v;
};

export const num = (v: unknown, what: string): number => {
  if (typeof v !== 'number' || !Number.isFinite(v)) {
    throw new ScriptApiError(`${what}: expected a number`);
  }
  return v;
};

export const str = (v: unknown, what: string): string => {
  if (typeof v !== 'string') throw new ScriptApiError(`${what}: expected a string`);
  return v;
};

export const obj = (v: unknown, what: string): Record<string, unknown> => {
  if (!v || typeof v !== 'object' || Array.isArray(v)) {
    throw new ScriptApiError(`${what}: expected an object`);
  }
  return v as Record<string, unknown>;
};

/** A raw engine command (`{ cmd: '…', … }`) for the `command()` escape hatch. */
export const rawCommand = <T>(v: unknown): T => {
  const o = obj(v, 'command');
  if (typeof o.cmd !== 'string' || !o.cmd) throw new ScriptApiError('command: missing cmd');
  return o as T;
};

/** `#RRGGBB` / `RRGGBB` → `RRGGBB`; null stays null. */
export const hex = (v: unknown, what: string): string | null => {
  if (v === null || v === undefined) return null;
  const s = str(v, what).replace(/^#/, '').toUpperCase();
  if (!/^[0-9A-F]{6}$/.test(s)) throw new ScriptApiError(`${what}: expected #RRGGBB`);
  return s;
};

/** UTF-8 byte offset of UTF-16 index `i` in `text` (engine positions are bytes). */
export function byteOffset(text: string, i: number): number {
  const end = Math.max(0, Math.min(i, text.length));
  let bytes = 0;
  for (let k = 0; k < end; k++) {
    const c = text.charCodeAt(k);
    if (c < 0x80) bytes += 1;
    else if (c < 0x800) bytes += 2;
    else if (c >= 0xd800 && c < 0xdc00 && k + 1 < end) {
      bytes += 4;
      k++;
    } else bytes += 3;
  }
  return bytes;
}

/** UTF-16 index of UTF-8 byte offset `b` in `text`. */
export function utf16Offset(text: string, b: number): number {
  let bytes = 0;
  for (let k = 0; k < text.length; k++) {
    if (bytes >= b) return k;
    const c = text.charCodeAt(k);
    if (c < 0x80) bytes += 1;
    else if (c < 0x800) bytes += 2;
    else if (c >= 0xd800 && c < 0xdc00) {
      bytes += 4;
      k++;
    } else bytes += 3;
  }
  return text.length;
}
