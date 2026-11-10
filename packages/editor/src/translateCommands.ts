/**
 * Translation commands (CORE-LANG-001), in the three apps' registries: `review.translate` opens
 * the translation pane on the selection; each app's bulk actions are commands too
 * (`review.translateDocument`, `review.translateCells`, `review.translateCellsInPlace`,
 * `review.translateDeck`) that open the pane and run the action at once.
 */
import type { CommandRegistry } from './commands';
import type { Translate, TranslateOpen } from './TranslatePane';

/**
 * Holds the editor's translation hook for the commands, which run at event time; the editor
 * keeps one per mount and refreshes it after each render (like `ReadAloudLink`).
 */
export class TranslateLink {
  private hook: Translate | null = null;

  attach(hook: Translate): void {
    this.hook = hook;
  }

  open(o: TranslateOpen): void {
    this.hook?.open(o);
  }

  close(): void {
    this.hook?.close();
  }
}

export function registerTranslateCommands<S>(
  r: CommandRegistry<S>,
  deps: {
    enabled: (s: S) => boolean;
    /** Opens the pane on the selection; `action` runs that bulk action at once. */
    begin: (s: S, action?: string) => void | Promise<void>;
    /** Bulk action ids → command ids (`document` → `review.translateDocument`). */
    actions?: Record<string, string>;
  },
): () => void {
  const offs = [
    r.register({
      id: 'review.translate',
      isEnabled: deps.enabled,
      run: (s) => deps.begin(s),
    }),
    ...Object.entries(deps.actions ?? {}).map(([action, id]) =>
      r.register({ id, isEnabled: deps.enabled, run: (s) => deps.begin(s, action) }),
    ),
  ];
  return () => offs.forEach((off) => off());
}
