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

  register(cmd: Command<S>): () => void {
    if (this.commands.has(cmd.id)) throw new Error(`command already registered: ${cmd.id}`);
    this.commands.set(cmd.id, cmd);
    return () => this.commands.delete(cmd.id);
  }

  has(id: string): boolean {
    return this.commands.has(id);
  }

  get(id: string): Command<S> | undefined {
    return this.commands.get(id);
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
    await c.run(state);
    return true;
  }

  /** Find the command bound to a keyboard event. */
  match(e: Pick<KeyboardEvent, 'key' | 'ctrlKey' | 'metaKey' | 'shiftKey' | 'altKey'>): Command<S> | undefined {
    const combo = shortcutOf(e);
    for (const c of this.commands.values()) {
      if (c.shortcut && normalize(c.shortcut) === combo) return c;
    }
    return undefined;
  }
}

function normalize(s: string): string {
  const parts = s.split('+').map((p) => p.trim().toLowerCase());
  const key = parts.pop() ?? '';
  const mods = ['ctrl', 'shift', 'alt'].filter((m) => parts.includes(m));
  return [...mods, key].join('+');
}

function shortcutOf(e: Pick<KeyboardEvent, 'key' | 'ctrlKey' | 'metaKey' | 'shiftKey' | 'altKey'>): string {
  const mods: string[] = [];
  if (e.ctrlKey || e.metaKey) mods.push('ctrl');
  if (e.shiftKey) mods.push('shift');
  if (e.altKey) mods.push('alt');
  return [...mods, e.key.toLowerCase()].join('+');
}
