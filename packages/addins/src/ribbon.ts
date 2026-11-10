/**
 * Ribbon commands contributed by add-ins (CORE-ADDIN-001): each installed
 * add-in with the `ui:ribbon` permission gets one group, placed in the tab its
 * manifest names (`ribbon.tab`) when that tab exists, else in the "Add-in" tab,
 * which also holds "Manage add-ins".
 */
import type { RibbonGroup, RibbonTab } from '@blueoffice/ui';

export const ADDINS_TAB = 'addins';

export interface AddinGroup {
  /** Manifest `ribbon.tab` (may name no tab). */
  tab?: string;
  group: RibbonGroup;
}

/** `tabs` with the add-in groups merged in and the "Add-in" tab appended. */
export function mergeAddinTabs(
  tabs: RibbonTab[],
  groups: AddinGroup[],
  addinTab: RibbonTab,
): RibbonTab[] {
  const extra = new Map<string, RibbonGroup[]>();
  const own: RibbonGroup[] = [];
  for (const g of groups) {
    const target =
      g.tab && g.tab !== ADDINS_TAB ? tabs.find((t) => t.id === g.tab && !t.context) : undefined;
    if (target) extra.set(target.id, [...(extra.get(target.id) ?? []), g.group]);
    else own.push(g.group);
  }
  const merged = tabs.map((t) => {
    const add = extra.get(t.id);
    return add ? { ...t, groups: [...t.groups, ...add] } : t;
  });
  // Contextual tabs stay last (the ribbon groups them after the regular tabs).
  const at = merged.findIndex((t) => t.context);
  const tab: RibbonTab = { ...addinTab, groups: [...addinTab.groups, ...own] };
  return at < 0 ? [...merged, tab] : [...merged.slice(0, at), tab, ...merged.slice(at)];
}
