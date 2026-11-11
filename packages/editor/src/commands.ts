/**
 * Command registry (prompt §5.8): every feature is a command with an id;
 * ribbon, shortcuts, context menu and command palette all go through here.
 * Enabled/checked state is computed from the current editor state.
 */
export interface Command<S> {
  id: string;
  run: (state: S) => void | Promise<void>;
  isEnabled?: (state: S) => boolean;
  isChecked?: (state: S) => boolean;
  /** Shortcut in `Ctrl+Shift+K` form ("Ctrl" means Cmd on macOS). */
  shortcut?: string;
}

export class CommandRegistry<S> {
  private commands = new Map<string, Command<S>>();
  /** User shortcuts (CORE-UI-004): a combo replaces the default, `null` removes it. */
  private overrides = new Map<string, string | null>();
  private repeatable: ((id: string) => boolean) | null = null;
  /** Last command run through `execute` that `repeatable` accepted (F4 "repeat"). */
  lastRepeatable: string | null = null;

  register(cmd: Command<S>): () => void {
    if (this.commands.has(cmd.id)) throw new Error(`command already registered: ${cmd.id}`);
    this.commands.set(cmd.id, cmd);
    return () => this.commands.delete(cmd.id);
  }

  has(id: string): boolean {
    return this.commands.has(id);
  }

  /** The command, with its effective shortcut (the user's one when customised). */
  get(id: string): Command<S> | undefined {
    const c = this.commands.get(id);
    if (!c || !this.overrides.has(id)) return c;
    return { ...c, shortcut: this.overrides.get(id) ?? undefined };
  }

  /** Shortcut the command was registered with (before user changes). */
  defaultShortcut(id: string): string | undefined {
    return this.commands.get(id)?.shortcut;
  }

  /** Effective shortcut of a command. */
  shortcut(id: string): string | undefined {
    if (this.overrides.has(id)) return this.overrides.get(id) ?? undefined;
    return this.commands.get(id)?.shortcut;
  }

  /** Replace the user shortcuts (`null`: shortcut removed); unknown ids are kept for later. */
  setShortcutOverrides(map: Readonly<Record<string, string | null>>): void {
    this.overrides = new Map(Object.entries(map));
  }

  shortcutOverrides(): Record<string, string | null> {
    return Object.fromEntries(this.overrides);
  }

  /** Commands remembered for "repeat last command" (F4); see `lastRepeatable`. */
  trackRepeat(pred: (id: string) => boolean): void {
    this.repeatable = pred;
  }

  /** Commands whose effective shortcut is `shortcut` (any form), optionally but one. */
  conflicts(shortcut: string, except?: string): string[] {
    const combo = normalizeShortcut(shortcut);
    return [...this.commands.keys()].filter((id) => {
      if (id === except) return false;
      const s = this.shortcut(id);
      return !!s && normalizeShortcut(s) === combo;
    });
  }

  ids(): string[] {
    return [...this.commands.keys()];
  }

  isEnabled(id: string, state: S): boolean {
    const c = this.commands.get(id);
    return !!c && (c.isEnabled?.(state) ?? true);
  }

  isChecked(id: string, state: S): boolean {
    return this.commands.get(id)?.isChecked?.(state) ?? false;
  }

  /** Run a command if it exists and is enabled. Returns whether it ran. */
  async execute(id: string, state: S): Promise<boolean> {
    const c = this.commands.get(id);
    if (!c || !(c.isEnabled?.(state) ?? true)) return false;
    if (this.repeatable?.(id)) this.lastRepeatable = id;
    await c.run(state);
    return true;
  }

  /** The command bound to `shortcut` (`Ctrl+Shift+K` form, any case or modifier order). */
  findByShortcut(shortcut: string): Command<S> | undefined {
    const combo = normalizeShortcut(shortcut);
    for (const id of this.commands.keys()) {
      const s = this.shortcut(id);
      if (s && normalizeShortcut(s) === combo) return this.get(id);
    }
    return undefined;
  }

  /** Find the command bound to a keyboard event (by key, else by physical key: Ctrl+Shift+8 = Ctrl+*). */
  match(
    e: Pick<KeyboardEvent, 'key' | 'ctrlKey' | 'metaKey' | 'shiftKey' | 'altKey'> &
      Partial<Pick<KeyboardEvent, 'code'>>,
  ): Command<S> | undefined {
    const combos = [shortcutOf(e)];
    const physical =
      /^(?:Digit|Key)([0-9A-Z])$/.exec(e.code ?? '')?.[1] ?? PUNCTUATION[e.code ?? ''];
    // (Not { ...e }: the modifier flags of a real KeyboardEvent are getters, not own fields.)
    if (physical)
      combos.push(
        shortcutOf({
          key: physical,
          ctrlKey: e.ctrlKey,
          metaKey: e.metaKey,
          shiftKey: e.shiftKey,
          altKey: e.altKey,
        }),
      );
    // User shortcuts first, so a combo moved to another command wins over its default owner.
    for (const combo of combos) {
      for (const [id, s] of this.overrides) {
        if (s && this.commands.has(id) && normalizeShortcut(s) === combo) return this.get(id);
      }
      for (const [id, c] of this.commands) {
        if (this.overrides.has(id)) continue;
        if (c.shortcut && normalizeShortcut(c.shortcut) === combo) return c;
      }
    }
    return undefined;
  }
}

/** Canonical form of a shortcut: `ctrl+shift+alt+key`, lower case. */
export function normalizeShortcut(s: string): string {
  const parts = s.split('+').map((p) => p.trim().toLowerCase());
  const key = parts.pop() ?? '';
  const mods = ['ctrl', 'shift', 'alt'].filter((m) => parts.includes(m));
  return [...mods, key].join('+');
}

/** Keys by physical position (Ctrl+Shift+= is Ctrl+Shift+Plus, Ctrl+Shift+. is Ctrl+Shift+>). */
const PUNCTUATION: Record<string, string> = {
  Equal: '=',
  Minus: '-',
  Period: '.',
  Comma: ',',
  BracketLeft: '[',
  BracketRight: ']',
  Semicolon: ';',
  Slash: '/',
  Space: 'Space',
};

function shortcutOf(
  e: Pick<KeyboardEvent, 'key' | 'ctrlKey' | 'metaKey' | 'shiftKey' | 'altKey'>,
): string {
  const mods: string[] = [];
  if (e.ctrlKey || e.metaKey) mods.push('ctrl');
  if (e.shiftKey) mods.push('shift');
  if (e.altKey) mods.push('alt');
  return [...mods, e.key === ' ' ? 'space' : e.key.toLowerCase()].join('+');
}
