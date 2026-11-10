/**
 * BlueAI commands (CORE-AI-001), in the three apps' registries: `ai.pane` opens the BlueAI task
 * pane; each feature of an app is a command too (`ai.rewrite`, `ai.summarize`, `ai.draft`,
 * `ai.slidesFromOutline`, `ai.slidesFromTopic`, `ai.formula`, `ai.analyze`) that opens the pane
 * on that feature. A command never sends anything by itself: the user runs the feature and
 * confirms its preview in the pane. Features that need the provider are disabled once BlueAI
 * is known to be off; rule-based ones (slides from an outline) stay enabled.
 */
import type { CommandRegistry } from './commands';
import type { AiOpen, BlueAi } from './AiPane';

/**
 * Holds the editor's BlueAI hook for the commands, which run at event time; the editor keeps
 * one per mount and refreshes it after each render (like `TranslateLink`).
 */
export class AiLink {
  private hook: BlueAi | null = null;

  attach(hook: BlueAi): void {
    this.hook = hook;
  }

  /** BlueAI is known to be off. */
  get off(): boolean {
    return this.hook?.off ?? false;
  }

  open(o: AiOpen): void {
    this.hook?.open(o);
  }

  close(): void {
    this.hook?.close();
  }
}

export function registerAiCommands<S>(
  r: CommandRegistry<S>,
  deps: {
    enabled: (s: S) => boolean;
    /** Opens the pane, on `feature` when given. */
    begin: (s: S, feature?: string) => void | Promise<void>;
    /** Feature ids → their command (`ai`: needs the provider). */
    features?: Record<string, { command: string; ai: boolean }>;
    /** BlueAI is known to be off. */
    isOff?: () => boolean;
  },
): () => void {
  const offs = [
    r.register({ id: 'ai.pane', isEnabled: deps.enabled, run: (s) => deps.begin(s) }),
    ...Object.entries(deps.features ?? {}).map(([feature, c]) =>
      r.register({
        id: c.command,
        isEnabled: (s) => deps.enabled(s) && !(c.ai && deps.isOff?.()),
        run: (s) => deps.begin(s, feature),
      }),
    ),
  ];
  return () => offs.forEach((off) => off());
}
