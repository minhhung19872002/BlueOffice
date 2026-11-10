import type { RibbonTab } from './Ribbon';

/** A command the search box can run (flattened from the ribbon). */
export interface SearchEntry {
  id: string;
  label: string;
  /** Where it lives: "Tab › Group". */
  path: string;
  shortcut?: string;
  enabled: boolean;
  onSelect: () => void;
}

/** Lower-case, strip Vietnamese diacritics (đ → d) so "dinh dang" finds "Định dạng". */
export function normalize(s: string): string {
  return s
    .normalize('NFD')
    .replace(/[̀-ͯ]/g, '')
    .replace(/đ/g, 'd')
    .replace(/Đ/g, 'D')
    .toLowerCase()
    .trim();
}

/** Every ribbon command once (first occurrence wins); custom controls are left out. */
export function flattenRibbon(tabs: RibbonTab[]): SearchEntry[] {
  const seen = new Set<string>();
  const out: SearchEntry[] = [];
  for (const tab of tabs)
    for (const group of tab.groups)
      for (const item of group.items) {
        if (item.render || seen.has(item.id) || !item.label || item.label === item.id) continue;
        seen.add(item.id);
        out.push({
          id: item.id,
          label: item.label,
          path: `${tab.label} › ${group.label}`,
          shortcut: item.shortcut,
          enabled: item.enabled !== false,
          onSelect: item.onSelect,
        });
      }
  return out;
}

/**
 * Rank entries for a query: label starts with it (0) < a word of the label starts with it (1) <
 * label contains it (2) < every query word found in label + path + id (3); ties keep ribbon
 * order. Enabled commands come before disabled ones within a rank.
 */
export function searchCommands(entries: SearchEntry[], query: string, limit = 12): SearchEntry[] {
  const q = normalize(query);
  if (!q) return [];
  const words = q.split(/\s+/);
  const scored: { e: SearchEntry; rank: number; i: number }[] = [];
  entries.forEach((e, i) => {
    const label = normalize(e.label);
    const hay = `${label} ${normalize(e.path)} ${e.id.toLowerCase()}`;
    let rank: number;
    if (label.startsWith(q)) rank = 0;
    else if (label.split(/[\s/–-]+/).some((w) => w.startsWith(q))) rank = 1;
    else if (label.includes(q)) rank = 2;
    else if (words.every((w) => hay.includes(w))) rank = 3;
    else return;
    scored.push({ e, rank: rank * 2 + (e.enabled ? 0 : 1), i });
  });
  scored.sort((a, b) => a.rank - b.rank || a.i - b.i);
  return scored.slice(0, limit).map((s) => s.e);
}

/**
 * Commands of the best match's ribbon group not already found ("related" suggestions), in
 * ribbon order, enabled first.
 */
export function relatedCommands(
  entries: SearchEntry[],
  found: SearchEntry[],
  limit = 3,
): SearchEntry[] {
  const top = found[0];
  if (!top) return [];
  const ids = new Set(found.map((f) => f.id));
  return entries
    .filter((e) => e.path === top.path && !ids.has(e.id))
    .sort((a, b) => Number(b.enabled) - Number(a.enabled))
    .slice(0, limit);
}

const RECENT_KEY = 'bo.commandSearch.recent';
const RECENT_MAX = 10;

/** Commands last run from the search box, newest first (per browser: a convenience). */
export function loadRecentCommands(): string[] {
  try {
    const raw = globalThis.localStorage?.getItem(RECENT_KEY);
    const v: unknown = raw ? JSON.parse(raw) : [];
    return Array.isArray(v) ? v.filter((x): x is string => typeof x === 'string') : [];
  } catch {
    return [];
  }
}

/** Remember a command run from the search box; returns the new list. */
export function pushRecentCommand(id: string): string[] {
  const next = [id, ...loadRecentCommands().filter((x) => x !== id)].slice(0, RECENT_MAX);
  try {
    globalThis.localStorage?.setItem(RECENT_KEY, JSON.stringify(next));
  } catch {
    // Storage unavailable: the list lives for this session only.
  }
  return next;
}
