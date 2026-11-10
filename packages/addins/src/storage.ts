/**
 * Per-add-in key/value storage (CORE-ADDIN-001, `storage` permission). Each
 * add-in gets one entry of the platform's local storage, keyed by its id, so an
 * add-in can never name — let alone read — another add-in's data; uninstalling
 * deletes the entry. The values stay in the user's browser / desktop web view
 * (like saved BlueScripts, ADR-0012).
 */

export interface KeyValueBacking {
  getItem(key: string): string | null;
  setItem(key: string, value: string): void;
  removeItem(key: string): void;
}

export const STORAGE_PREFIX = 'blueoffice.addin.storage.';

export const STORAGE_LIMITS = {
  /** JSON characters of all of one add-in's values. */
  totalChars: 1_000_000,
  keys: 500,
  keyChars: 200,
} as const;

export class AddinStorageError extends Error {
  override name = 'AddinStorageError';
}

export interface AddinStorage {
  get(key: string): unknown;
  set(key: string, value: unknown): void;
  remove(key: string): void;
  keys(): string[];
  /** Deletes everything (uninstall). */
  clear(): void;
}

/** In-memory backing (tests, or where local storage throws). */
export function memoryBacking(): KeyValueBacking & { dump(): Record<string, string> } {
  const m = new Map<string, string>();
  return {
    getItem: (k) => m.get(k) ?? null,
    setItem: (k, v) => void m.set(k, v),
    removeItem: (k) => void m.delete(k),
    dump: () => Object.fromEntries(m),
  };
}

/** The browser's local storage, or an in-memory one where it is unavailable. */
export function defaultBacking(): KeyValueBacking {
  try {
    if (typeof localStorage !== 'undefined') return localStorage;
  } catch {
    // Storage disabled (privacy mode): fall through.
  }
  return memoryBacking();
}

const checkKey = (key: unknown): string => {
  if (typeof key !== 'string' || !key || key.length > STORAGE_LIMITS.keyChars) {
    throw new AddinStorageError(`key: expected 1 to ${STORAGE_LIMITS.keyChars} characters`);
  }
  if (key === '__proto__') throw new AddinStorageError('key: reserved');
  return key;
};

/** Storage of add-in `addinId`. */
export function addinStorage(addinId: string, backing: KeyValueBacking): AddinStorage {
  const entry = STORAGE_PREFIX + addinId;
  const read = (): Record<string, unknown> => {
    try {
      const v = JSON.parse(backing.getItem(entry) ?? '{}') as unknown;
      return v && typeof v === 'object' && !Array.isArray(v) ? (v as Record<string, unknown>) : {};
    } catch {
      return {};
    }
  };
  const write = (data: Record<string, unknown>) => {
    const text = JSON.stringify(data);
    if (text.length > STORAGE_LIMITS.totalChars) {
      throw new AddinStorageError(`storage is limited to ${STORAGE_LIMITS.totalChars} characters`);
    }
    if (Object.keys(data).length === 0) backing.removeItem(entry);
    else backing.setItem(entry, text);
  };
  return {
    get: (key) => {
      const data = read();
      const k = checkKey(key);
      return Object.prototype.hasOwnProperty.call(data, k) ? data[k] : null;
    },
    set: (key, value) => {
      const k = checkKey(key);
      if (value === undefined) throw new AddinStorageError('value: expected JSON');
      const data = read();
      if (
        !Object.prototype.hasOwnProperty.call(data, k) &&
        Object.keys(data).length >= STORAGE_LIMITS.keys
      ) {
        throw new AddinStorageError(`at most ${STORAGE_LIMITS.keys} keys`);
      }
      data[k] = value;
      write(data);
    },
    remove: (key) => {
      const data = read();
      delete data[checkKey(key)];
      write(data);
    },
    keys: () => Object.keys(read()),
    clear: () => backing.removeItem(entry),
  };
}
