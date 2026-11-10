/**
 * Saved BlueScripts bound to a keyboard shortcut or a Quick Access Toolbar
 * button (CORE-SCRIPT-002). Each bound script is a `script.run.<id>` command in
 * the editor's {@link CommandRegistry}, so it runs through the same shortcut
 * matching as built-in commands; a shortcut already taken is refused.
 * Bindings live with the saved scripts (per user, per application, ADR-0012).
 */
import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import {
  defaultRunner,
  scriptStore,
  type RunResult,
  type SavedScript,
  type ScriptApp,
  type ScriptHost,
  type ScriptRecorder,
  type ScriptRunner,
  type ScriptStore,
} from '@blueoffice/script';
import { normalizeShortcut, type CommandRegistry } from './commands';

export const SCRIPT_COMMAND_PREFIX = 'script.run.';

/** Whether a shortcut may be bound: Ctrl or Alt, or a function key. */
export function validScriptShortcut(shortcut: string): boolean {
  const parts = normalizeShortcut(shortcut).split('+');
  const key = parts[parts.length - 1] ?? '';
  if (!key) return false;
  if (/^f([1-9]|1[0-2])$/.test(key)) return true;
  return parts.includes('ctrl') || parts.includes('alt');
}

/**
 * Why `shortcut` cannot run script `scriptId`: `invalid`, the label of the
 * command already bound to it, or null when it is free.
 */
export function scriptShortcutConflict<S>(
  registry: CommandRegistry<S>,
  shortcut: string,
  scriptId: string,
  label: (commandId: string) => string = (id) => id,
): string | null {
  if (!validScriptShortcut(shortcut)) return 'invalid';
  const taken = registry.findByShortcut(shortcut);
  if (!taken || taken.id === SCRIPT_COMMAND_PREFIX + scriptId) return null;
  return label(taken.id);
}

export interface BindScriptsOptions<S> {
  registry: CommandRegistry<S>;
  scripts: SavedScript[];
  /** Host of a run; null while no file is open. */
  host: () => ScriptHost | null;
  runner?: ScriptRunner;
  recorder?: ScriptRecorder;
  onResult?: (script: SavedScript, result: RunResult) => void;
}

/** Registers a `script.run.<id>` command per bound script; returns the unregister function. */
export function bindScripts<S>({
  registry,
  scripts,
  host,
  runner,
  recorder,
  onResult,
}: BindScriptsOptions<S>): () => void {
  const offs: (() => void)[] = [];
  let run: ScriptRunner | null = runner ?? null;
  for (const s of scripts) {
    if (!s.shortcut && !s.quickAccess) continue;
    const id = SCRIPT_COMMAND_PREFIX + s.id;
    if (registry.has(id)) continue;
    // A shortcut taken by a built-in command since it was bound stays with the built-in.
    const shortcut =
      s.shortcut && !registry.findByShortcut(s.shortcut) && validScriptShortcut(s.shortcut)
        ? s.shortcut
        : undefined;
    offs.push(
      registry.register({
        id,
        ...(shortcut ? { shortcut } : {}),
        isEnabled: () => host() !== null,
        run: async () => {
          const h = host();
          if (!h) return;
          run ??= defaultRunner();
          const r = run.run(s.code, h);
          const res = await (recorder ? recorder.pause(() => r.done) : r.done);
          onResult?.(s, res);
        },
      }),
    );
  }
  return () => offs.forEach((off) => off());
}

export interface ScriptBindingsOptions<S> {
  app: ScriptApp;
  registry: CommandRegistry<S>;
  host: () => ScriptHost | null;
  recorder?: ScriptRecorder;
  store?: ScriptStore;
  runner?: ScriptRunner;
  onResult?: (script: SavedScript, result: RunResult) => void;
  /** Label of a command id in conflict messages. */
  label?: (commandId: string) => string;
}

/**
 * Keeps the registry's script commands in step with the saved scripts; returns
 * the scripts shown on the Quick Access Toolbar and the shortcut check for the
 * BlueScript pane.
 */
export function useScriptBindings<S>({
  app,
  registry,
  host,
  recorder,
  store,
  runner,
  onResult,
  label,
}: ScriptBindingsOptions<S>) {
  const scripts = useMemo(() => store ?? scriptStore(app), [store, app]);
  const [list, setList] = useState<SavedScript[]>(() => scripts.list());
  useEffect(() => scripts.subscribe(() => setList(scripts.list())), [scripts]);
  const latest = useRef({ host, onResult, label });
  useEffect(() => {
    latest.current = { host, onResult, label };
  });
  useEffect(
    () =>
      bindScripts({
        registry,
        scripts: list,
        host: () => latest.current.host(),
        ...(runner ? { runner } : {}),
        ...(recorder ? { recorder } : {}),
        onResult: (s, r) => latest.current.onResult?.(s, r),
      }),
    [registry, list, runner, recorder],
  );
  const checkShortcut = useCallback(
    (shortcut: string, scriptId: string) => {
      const named = (id: string) => {
        if (id.startsWith(SCRIPT_COMMAND_PREFIX)) {
          const s = list.find((x) => SCRIPT_COMMAND_PREFIX + x.id === id);
          if (s) return s.name;
        }
        return latest.current.label?.(id) ?? id;
      };
      return scriptShortcutConflict(registry, shortcut, scriptId, named);
    },
    [registry, list],
  );
  const quickAccess = useMemo(() => list.filter((s) => s.quickAccess), [list]);
  return { quickAccess, checkShortcut };
}
