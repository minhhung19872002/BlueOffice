/**
 * Contextual ribbon tabs (CORE-UI-001, UI-RIBBON-004): tabs that belong to the object at the
 * selection (table, picture, chart…). Pure helpers (no DOM) so the show / hide / auto-activate
 * rules can be unit-tested.
 */

/** Kind of object a contextual tab belongs to. */
export type RibbonContext =
  | 'table'
  | 'picture'
  | 'chart'
  | 'shape'
  | 'headerFooter'
  | 'smartArt'
  | 'equation'
  | 'ink'
  | 'object'
  | 'media'
  | 'sparkline';

export const RIBBON_CONTEXTS: readonly RibbonContext[] = [
  'table',
  'picture',
  'chart',
  'shape',
  'headerFooter',
  'smartArt',
  'equation',
  'ink',
  'object',
  'media',
  'sparkline',
];

interface TabLike {
  id: string;
  context?: RibbonContext;
}

/** The tab strip: regular tabs in order, then one run of tabs per context (first-seen order). */
export interface ArrangedTabs<T extends TabLike> {
  plain: T[];
  contexts: { context: RibbonContext; tabs: T[] }[];
}

export function arrangeTabs<T extends TabLike>(tabs: readonly T[]): ArrangedTabs<T> {
  const plain: T[] = [];
  const contexts: { context: RibbonContext; tabs: T[] }[] = [];
  for (const t of tabs) {
    if (!t.context) {
      plain.push(t);
      continue;
    }
    const run = contexts.find((c) => c.context === t.context);
    if (run) run.tabs.push(t);
    else contexts.push({ context: t.context, tabs: [t] });
  }
  return { plain, contexts };
}

/** What the ribbon remembers between renders to decide the active tab. */
export interface ContextTracker {
  /** Contexts shown at the previous decision. */
  present: RibbonContext[];
  /** Last regular (non-contextual) tab that was active: where the ribbon returns. */
  lastPlain: string | null;
}

export const EMPTY_TRACKER: ContextTracker = { present: [], lastPlain: null };

export interface ActiveTabDecision {
  value: string | undefined;
  tracker: ContextTracker;
  /** Set when a context just appeared and its first tab was activated. */
  activated: boolean;
}

/**
 * Chooses the active tab:
 *  - a context that just appeared (not shown at the previous decision) activates its first tab,
 *    once — switching to another tab while the object stays selected is kept. Only the
 *    `autoActivate` contexts do: caret contexts (a table cell being typed in) usually only show;
 *  - otherwise the requested tab when it exists;
 *  - otherwise (its context went away) the last regular tab, else the first tab.
 */
export function decideActiveTab<T extends TabLike>(
  tabs: readonly T[],
  requested: string | undefined,
  tracker: ContextTracker,
  /** Contexts that activate their tab when they appear (default: all). */
  autoActivate: readonly RibbonContext[] = RIBBON_CONTEXTS,
): ActiveTabDecision {
  const present: RibbonContext[] = [];
  for (const t of tabs) if (t.context && !present.includes(t.context)) present.push(t.context);
  const appeared = present.find((c) => !tracker.present.includes(c) && autoActivate.includes(c));
  let value: string | undefined;
  let activated = false;
  const asked = requested === undefined ? undefined : tabs.find((t) => t.id === requested);
  // Already on a tab of the appearing context (e.g. opened on it): keep it.
  if (appeared && !(asked?.context && asked.context === appeared)) {
    value = tabs.find((t) => t.context === appeared)?.id;
    activated = value !== undefined && value !== requested;
  } else if (requested !== undefined && tabs.some((t) => t.id === requested)) {
    value = requested;
  } else {
    const back = tracker.lastPlain;
    value = back && tabs.some((t) => t.id === back) ? back : tabs[0]?.id;
  }
  // Remember the regular tab the user was on (the requested one before an auto-activation).
  const plainOf = (id: string | undefined) =>
    id !== undefined && tabs.some((t) => t.id === id && !t.context) ? id : null;
  const lastPlain = plainOf(value) ?? plainOf(requested) ?? tracker.lastPlain;
  return { value, tracker: { present, lastPlain }, activated };
}
