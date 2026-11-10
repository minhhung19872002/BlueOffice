/**
 * Read aloud commands (CORE-LANG-003), the same ids in the three apps' registries.
 * `review.readAloud` starts reading (from the caret / selection / selected cells / current
 * slide) or stops it; the others drive the running session.
 */
import type { CommandRegistry } from './commands';
import type { ReadAloud } from './ReadAloudBar';

/** Start or stop reading (our choice of keys: no default binding uses it). */
export const READ_ALOUD_SHORTCUT = 'Ctrl+Alt+Shift+R';

export const READ_ALOUD_COMMANDS = [
  'review.readAloud',
  'review.readAloudPause',
  'review.readAloudPrevious',
  'review.readAloudNext',
  'review.readAloudStop',
] as const;

/**
 * Holds the editor's read-aloud hook (and how to begin reading) for the commands, which run at
 * event time; the editor keeps one per mount (`useState(() => new ReadAloudLink())`) and
 * refreshes it after each render.
 */
export class ReadAloudLink<S = unknown> {
  private hook: ReadAloud | null = null;
  private start: ((s: S) => void | Promise<void>) | null = null;

  /** The latest hook value and start function (call from an effect). */
  attach(hook: ReadAloud, start?: (s: S) => void | Promise<void>): void {
    this.hook = hook;
    if (start) this.start = start;
  }

  aloud(): ReadAloud | null {
    return this.hook;
  }

  async begin(s: S): Promise<void> {
    await this.start?.(s);
  }
}

export function registerReadAloudCommands<S>(
  r: CommandRegistry<S>,
  deps: {
    /** There is something to read (a document is open). */
    enabled: (s: S) => boolean;
    /** Start reading from the current position. */
    begin: (s: S) => void | Promise<void>;
    /** The editor's read-aloud hook (a ref: commands run at event time). */
    current: () => ReadAloud | null;
  },
): () => void {
  const active = () => !!deps.current()?.active;
  const offs = [
    r.register({
      id: 'review.readAloud',
      shortcut: READ_ALOUD_SHORTCUT,
      isEnabled: deps.enabled,
      isChecked: active,
      run: async (s) => {
        const ra = deps.current();
        if (ra?.active) ra.stop();
        else await deps.begin(s);
      },
    }),
    r.register({
      id: 'review.readAloudPause',
      isEnabled: () => deps.current()?.state !== 'idle' && active(),
      isChecked: () => deps.current()?.state === 'paused',
      run: () => deps.current()?.togglePause(),
    }),
    r.register({
      id: 'review.readAloudPrevious',
      isEnabled: () => deps.current()?.state !== 'idle' && active(),
      run: () => deps.current()?.previous(),
    }),
    r.register({
      id: 'review.readAloudNext',
      isEnabled: () => deps.current()?.state !== 'idle' && active(),
      run: () => deps.current()?.next(),
    }),
    r.register({
      id: 'review.readAloudStop',
      isEnabled: active,
      run: () => deps.current()?.stop(),
    }),
  ];
  return () => offs.forEach((off) => off());
}
