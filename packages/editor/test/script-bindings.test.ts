// Saved scripts bound to shortcuts / the Quick Access Toolbar (CORE-SCRIPT-002).
import { describe, expect, it } from 'vitest';
import {
  scriptStore,
  type RunResult,
  type ScriptHost,
  type ScriptRunner,
} from '@blueoffice/script';
import { CommandRegistry } from '../src/commands';
import {
  bindScripts,
  scriptShortcutConflict,
  SCRIPT_COMMAND_PREFIX,
  validScriptShortcut,
} from '../src/scriptBindings';

const memory = () => {
  const m = new Map<string, string>();
  return {
    getItem: (k: string) => m.get(k) ?? null,
    setItem: (k: string, v: string) => void m.set(k, v),
  };
};

const host: ScriptHost = { app: 'doc', call: async () => null };

/** Records what runs instead of starting QuickJS. */
const fakeRunner = (ran: string[]): ScriptRunner => ({
  run: (code) => {
    ran.push(code);
    const result: RunResult = { ok: true, ms: 0, calls: 0 };
    return { done: Promise.resolve(result), stop: () => {} };
  },
});

describe('script bindings', () => {
  it('accepts only shortcuts with Ctrl / Alt or a function key', () => {
    expect(validScriptShortcut('Ctrl+Alt+1')).toBe(true);
    expect(validScriptShortcut('Alt+Shift+M')).toBe(true);
    expect(validScriptShortcut('F9')).toBe(true);
    expect(validScriptShortcut('Shift+F12')).toBe(true);
    expect(validScriptShortcut('M')).toBe(false);
    expect(validScriptShortcut('Shift+M')).toBe(false);
    expect(validScriptShortcut('F13')).toBe(false);
  });

  it('reports a shortcut that a command or another script already uses', () => {
    const r = new CommandRegistry<null>();
    r.register({ id: 'format.bold', shortcut: 'Ctrl+B', run: () => {} });
    r.register({ id: `${SCRIPT_COMMAND_PREFIX}s1`, shortcut: 'Ctrl+Alt+1', run: () => {} });
    expect(scriptShortcutConflict(r, 'ctrl+b', 's2')).toBe('format.bold');
    expect(scriptShortcutConflict(r, 'Ctrl+B', 's2', (id) => `«${id}»`)).toBe('«format.bold»');
    expect(scriptShortcutConflict(r, 'Alt+Ctrl+1', 's2')).toBe(`${SCRIPT_COMMAND_PREFIX}s1`);
    // A script keeps its own shortcut.
    expect(scriptShortcutConflict(r, 'Ctrl+Alt+1', 's1')).toBeNull();
    expect(scriptShortcutConflict(r, 'Ctrl+Alt+2', 's2')).toBeNull();
    expect(scriptShortcutConflict(r, 'Q', 's2')).toBe('invalid');
  });

  it('registers bound scripts as commands that a shortcut runs', async () => {
    const store = scriptStore('doc', memory());
    const a = store.save({ name: 'A', code: 'a()', shortcut: 'Ctrl+Alt+A' });
    const b = store.save({ name: 'B', code: 'b()', quickAccess: true });
    store.save({ name: 'C', code: 'c()' });
    // Taken by a built-in: the script stays reachable from the toolbar only.
    const d = store.save({ name: 'D', code: 'd()', shortcut: 'Ctrl+B', quickAccess: true });
    expect(store.list().find((s) => s.id === a.id)?.shortcut).toBe('Ctrl+Alt+A');

    const r = new CommandRegistry<null>();
    r.register({ id: 'format.bold', shortcut: 'Ctrl+B', run: () => {} });
    const ran: string[] = [];
    const results: string[] = [];
    let open = true;
    const off = bindScripts({
      registry: r,
      scripts: store.list(),
      host: () => (open ? host : null),
      runner: fakeRunner(ran),
      onResult: (s, res) => results.push(`${s.name}:${res.ok}`),
    });
    const ids = r.ids().filter((id) => id.startsWith(SCRIPT_COMMAND_PREFIX));
    expect(ids.sort()).toEqual([a.id, b.id, d.id].map((id) => SCRIPT_COMMAND_PREFIX + id).sort());
    const key = { key: 'a', ctrlKey: true, altKey: true, shiftKey: false, metaKey: false };
    expect(r.match(key)?.id).toBe(SCRIPT_COMMAND_PREFIX + a.id);
    expect(r.match({ ...key, key: 'b', altKey: false })?.id).toBe('format.bold');
    await r.execute(SCRIPT_COMMAND_PREFIX + a.id, null);
    await r.execute(SCRIPT_COMMAND_PREFIX + b.id, null);
    expect(ran).toEqual(['a()', 'b()']);
    expect(results).toEqual(['A:true', 'B:true']);
    // No document: disabled.
    open = false;
    expect(r.isEnabled(SCRIPT_COMMAND_PREFIX + a.id, null)).toBe(false);
    off();
    expect(r.ids().some((id) => id.startsWith(SCRIPT_COMMAND_PREFIX))).toBe(false);
  });

  it('notifies other stores of the same application', () => {
    const storage = memory();
    const one = scriptStore('sheet', storage);
    const two = scriptStore('sheet', storage);
    let seen = 0;
    const off = two.subscribe(() => seen++);
    one.save({ name: 'X', code: '1' });
    expect(seen).toBe(1);
    expect(two.list().map((s) => s.name)).toEqual(['X']);
    off();
    one.save({ name: 'Y', code: '2' });
    expect(seen).toBe(1);
  });
});
