/**
 * Saved BlueScripts (CORE-SCRIPT-001): kept per user and per application in the
 * platform's local storage (web: the origin's `localStorage`; desktop: the web
 * view's), never inside the document — a file opened elsewhere carries no code
 * that could run (ADR-0012). Export / import as `.js` files moves scripts around.
 */

export interface SavedScript {
  id: string;
  name: string;
  code: string;
  /** Unix ms of the last save. */
  updated: number;
  /** Keyboard shortcut that runs it (`Ctrl+Alt+1` form; CORE-SCRIPT-002). */
  shortcut?: string;
  /** Shown as a Quick Access Toolbar button (CORE-SCRIPT-002). */
  quickAccess?: boolean;
}

/** Key/value storage (a `Storage` subset). */
export interface KeyValueStorage {
  getItem(key: string): string | null;
  setItem(key: string, value: string): void;
}

export interface ScriptStore {
  list(): SavedScript[];
  save(script: Omit<SavedScript, 'id' | 'updated'> & { id?: string }): SavedScript;
  remove(id: string): void;
  /** Changes of the saved scripts of this application (any store instance of the same storage). */
  subscribe(fn: () => void): () => void;
}

/** Listeners by storage key: every store of an application hears the saves of the others. */
const listeners = new Map<string, Set<() => void>>();

/** Size cap of one script (characters). */
export const MAX_SCRIPT_CHARS = 200_000;

const memory = (): KeyValueStorage => {
  const m = new Map<string, string>();
  return { getItem: (k) => m.get(k) ?? null, setItem: (k, v) => void m.set(k, v) };
};

function defaultStorage(): KeyValueStorage {
  try {
    const s = globalThis.localStorage;
    if (s) return s;
  } catch {
    // storage blocked
  }
  return memory();
}

const isScript = (v: unknown): v is SavedScript => {
  const o = v as SavedScript | null;
  return (
    !!o &&
    typeof o.id === 'string' &&
    typeof o.name === 'string' &&
    typeof o.code === 'string' &&
    typeof o.updated === 'number'
  );
};

export function scriptStore(app: string, storage: KeyValueStorage = defaultStorage()): ScriptStore {
  const key = `blueoffice.scripts.${app}`;
  const read = (): SavedScript[] => {
    try {
      const v: unknown = JSON.parse(storage.getItem(key) ?? '[]');
      return Array.isArray(v) ? v.filter(isScript) : [];
    } catch {
      return [];
    }
  };
  const write = (list: SavedScript[]) => {
    try {
      storage.setItem(key, JSON.stringify(list));
    } catch {
      // quota / blocked: the pane keeps the text
    }
    listeners.get(key)?.forEach((l) => l());
  };
  return {
    list: () => read().sort((a, b) => a.name.localeCompare(b.name)),
    save(s) {
      const list = read();
      const id = s.id ?? `s${Date.now().toString(36)}${Math.random().toString(36).slice(2, 7)}`;
      const saved: SavedScript = {
        id,
        name: s.name.trim().slice(0, 200) || 'Script',
        code: s.code.slice(0, MAX_SCRIPT_CHARS),
        updated: Date.now(),
        ...(s.shortcut ? { shortcut: s.shortcut.slice(0, 40) } : {}),
        ...(s.quickAccess ? { quickAccess: true } : {}),
      };
      const i = list.findIndex((x) => x.id === id);
      if (i >= 0) list[i] = saved;
      else list.push(saved);
      write(list);
      return saved;
    },
    remove(id) {
      write(read().filter((x) => x.id !== id));
    },
    subscribe(fn) {
      let set = listeners.get(key);
      if (!set) listeners.set(key, (set = new Set()));
      set.add(fn);
      return () => void set.delete(fn);
    },
  };
}
