/**
 * Dictation commands (CORE-LANG-002), the same ids in the three apps' registries:
 * `edit.dictate` starts or stops dictation at the caret / text box / active cell; the others
 * drive the running session.
 */
import type { CommandRegistry } from './commands';
import type { Dictation } from './DictationBar';

/** Start or stop dictation (our choice of keys: no default binding uses it). */
export const DICTATE_SHORTCUT = 'Ctrl+Alt+Shift+D';

export const DICTATION_COMMANDS_IDS = [
  'edit.dictate',
  'edit.dictatePause',
  'edit.dictateStop',
] as const;

/** Holds the editor's dictation hook for the commands, which run at event time. */
export class DictationLink {
  private hook: Dictation | null = null;

  attach(hook: Dictation): void {
    this.hook = hook;
  }

  current(): Dictation | null {
    return this.hook;
  }
}

export function registerDictationCommands<S>(
  r: CommandRegistry<S>,
  deps: {
    /** There is somewhere to write (a document is open and editable). */
    enabled: (s: S) => boolean;
    /** The editor's dictation hook (a ref: commands run at event time). */
    current: () => Dictation | null;
  },
): () => void {
  const active = () => !!deps.current()?.active;
  const offs = [
    r.register({
      id: 'edit.dictate',
      shortcut: DICTATE_SHORTCUT,
      isEnabled: deps.enabled,
      isChecked: active,
      run: async () => {
        await deps.current()?.toggle();
      },
    }),
    r.register({
      id: 'edit.dictatePause',
      isEnabled: () => active() && deps.current()?.state !== 'idle',
      isChecked: () => deps.current()?.state === 'paused',
      run: async () => {
        await deps.current()?.togglePause();
      },
    }),
    r.register({
      id: 'edit.dictateStop',
      isEnabled: active,
      run: () => deps.current()?.stop(),
    }),
  ];
  return () => offs.forEach((off) => off());
}
