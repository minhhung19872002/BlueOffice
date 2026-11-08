/**
 * Responsive ribbon layout: decides how far each group collapses so the row of
 * groups fits the available width. Pure (no DOM) so it can be unit-tested.
 *
 * Levels, applied progressively from the rightmost group leftwards:
 *  - `full`: icons + labels;
 *  - `compact`: labels hidden (icon-only buttons, label in the tooltip);
 *  - `collapsed`: the whole group becomes one dropdown button listing its commands.
 * When even every group collapsed does not fit, the trailing groups move into a
 * single "more" dropdown, so the ribbon never overflows horizontally.
 */

export type CollapseLevel = 'full' | 'compact' | 'collapsed';

/** Measured (or estimated) width in CSS px of one group at each level. */
export interface GroupWidths {
  full: number;
  compact: number;
  collapsed: number;
}

export interface RibbonLayoutPlan {
  levels: CollapseLevel[];
  /** Index of the first group shown in the overflow menu; `levels.length` when none. */
  overflowFrom: number;
}

const ORDER: CollapseLevel[] = ['full', 'compact', 'collapsed'];

function widthAt(w: GroupWidths, level: CollapseLevel): number {
  return w[level];
}

/**
 * Plans the collapse levels. `available` is the inner width of the panel and
 * `overflowButton` the width reserved for the "more" button when it is needed.
 * Each step only applies when it makes the group narrower.
 */
export function planRibbonLayout(
  groups: readonly GroupWidths[],
  available: number,
  overflowButton = 48,
): RibbonLayoutPlan {
  const levels: CollapseLevel[] = groups.map(() => 'full');
  let total = groups.reduce((sum, g) => sum + g.full, 0);
  const fits = () => total <= available;

  // Steps 1 and 2: hide labels, then collapse into a dropdown, right to left.
  for (const target of ORDER.slice(1)) {
    for (let i = groups.length - 1; i >= 0 && !fits(); i -= 1) {
      const g = groups[i];
      const from = levels[i];
      if (!g || !from || ORDER.indexOf(from) >= ORDER.indexOf(target)) continue;
      const before = widthAt(g, from);
      const after = widthAt(g, target);
      if (after < before) {
        levels[i] = target;
        total += after - before;
      }
    }
  }

  // Step 3: move trailing groups into the overflow menu until the rest fits.
  let overflowFrom = groups.length;
  if (!fits()) {
    let shown = total;
    while (overflowFrom > 0 && shown + overflowButton > available) {
      overflowFrom -= 1;
      const g = groups[overflowFrom];
      const level = levels[overflowFrom];
      if (g && level) shown -= widthAt(g, level);
    }
  }
  return { levels, overflowFrom };
}

/** Estimates widths not measured yet from the ones that are (first layout pass). */
export function estimateWidths(known: Partial<GroupWidths>): GroupWidths {
  const full = known.full ?? known.compact ?? 120;
  const compact = known.compact ?? Math.min(full, Math.max(40, full * 0.6));
  const collapsed = known.collapsed ?? Math.min(compact, 72);
  return { full, compact, collapsed };
}

/** True when two plans render identically (avoids layout ping-pong). */
export function samePlan(a: RibbonLayoutPlan, b: RibbonLayoutPlan): boolean {
  return (
    a.overflowFrom === b.overflowFrom &&
    a.levels.length === b.levels.length &&
    a.levels.every((l, i) => l === b.levels[i])
  );
}
