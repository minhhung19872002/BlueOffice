import type { OutlineItem } from '@blueoffice/platform';

/** Level used for ordering: headings 0–8, body text below every heading. */
export const BODY = 9;

export const levelOf = (i: OutlineItem): number => i.level ?? BODY;

/** Whether the item at `index` has subordinate items (its family is more than itself). */
export function hasChildren(items: OutlineItem[], index: number): boolean {
  const it = items[index];
  const next = items[index + 1];
  return !!it && it.level !== null && !!next && levelOf(next) > levelOf(it);
}

/**
 * Items left visible by collapsed headings (`collapsed` holds paragraph ids) and the
 * "show level" filter (`showLevel` n keeps levels 1…n; null keeps everything).
 */
export function visibleItems(
  items: OutlineItem[],
  collapsed: ReadonlySet<number>,
  showLevel: number | null,
): OutlineItem[] {
  const out: OutlineItem[] = [];
  let hideBelow: number | null = null;
  for (const it of items) {
    const l = levelOf(it);
    if (hideBelow !== null && l > hideBelow) continue;
    hideBelow = collapsed.has(it.para) && it.level !== null ? l : null;
    if (showLevel !== null && l >= showLevel) continue;
    out.push(it);
  }
  return out;
}

/** Headings only (navigation pane). */
export const headingsOf = (items: OutlineItem[]): OutlineItem[] =>
  items.filter((i) => i.level !== null);

/** The outline item holding the caret paragraph: the last item at or before it. */
export function itemAt(items: OutlineItem[], para: number | undefined): OutlineItem | null {
  if (para === undefined) return null;
  let best: OutlineItem | null = null;
  for (const it of items) {
    if (it.para > para) break;
    best = it;
  }
  return best;
}

/** The heading whose section holds the caret paragraph. */
export function headingAt(items: OutlineItem[], para: number | undefined): OutlineItem | null {
  return itemAt(headingsOf(items), para);
}

/**
 * Level after "promote" (one level up; body text becomes a heading at the level of
 * the heading above it); `undefined` = nothing to change.
 */
export function promoted(items: OutlineItem[], para: number): number | null | undefined {
  const i = items.findIndex((x) => x.para === para);
  const it = items[i];
  if (!it) return undefined;
  if (it.level === null) {
    const above = items
      .slice(0, i)
      .reverse()
      .find((x) => x.level !== null);
    return above?.level ?? 0;
  }
  return it.level > 0 ? it.level - 1 : undefined;
}

/** Level after "demote" (one level down; level 9 becomes body text). */
export function demoted(items: OutlineItem[], para: number): number | null | undefined {
  const it = items.find((x) => x.para === para);
  if (!it || it.level === null) return undefined;
  return it.level < 8 ? it.level + 1 : null;
}
