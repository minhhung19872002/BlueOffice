import * as Tabs from '@radix-ui/react-tabs';
import * as Tooltip from '@radix-ui/react-tooltip';
import { useCallback, useLayoutEffect, useRef, useState, type ReactNode } from 'react';
import { hasIcon, Icon } from './icons';
import { Popover } from './Popover';
import styles from './Ribbon.module.css';
import {
  estimateWidths,
  planRibbonLayout,
  samePlan,
  type CollapseLevel,
  type GroupWidths,
  type RibbonLayoutPlan,
} from './ribbonLayout';

export interface RibbonItem {
  /** Command id from the command registry (e.g. `file.open`). */
  id: string;
  label: string;
  icon?: ReactNode;
  enabled?: boolean;
  checked?: boolean;
  size?: 'large' | 'small';
  tooltip?: string;
  shortcut?: string;
  onSelect: () => void;
  /** Custom control (combobox, colour picker…) shown instead of a button. */
  render?: ReactNode;
}

export interface RibbonGroup {
  id: string;
  label: string;
  items: RibbonItem[];
  /** Small button next to the group name opening the group's pane or dialog. */
  launcher?: RibbonLauncher;
}

export interface RibbonLauncher {
  /** Command id (e.g. `edit.clipboardPanel`). */
  id: string;
  /** Accessible name and tooltip. */
  label: string;
  onSelect: () => void;
}

function LauncherButton({ launcher, onDone }: { launcher: RibbonLauncher; onDone?: () => void }) {
  return (
    <button
      type="button"
      className={styles.launcher}
      data-command={launcher.id}
      data-launcher="true"
      aria-label={launcher.label}
      title={launcher.label}
      onMouseDown={(e) => e.preventDefault()}
      onClick={() => {
        launcher.onSelect();
        onDone?.();
      }}
    >
      <span aria-hidden="true">⤡</span>
    </button>
  );
}

export interface RibbonTab {
  id: string;
  label: string;
  groups: RibbonGroup[];
}

export interface RibbonProps {
  tabs: RibbonTab[];
  value?: string;
  onValueChange?: (tab: string) => void;
  /** Content shown at the right of the tab strip (e.g. host Share/Close buttons). */
  trailing?: ReactNode;
  /** Accessible name of the overflow ("more commands") button; the visible glyph is "…". */
  moreLabel?: string;
}

type ButtonMode = 'normal' | 'iconOnly' | 'menu';

/**
 * The item's icon: the BlueOffice icon of its command id (UI-ICONS-002: one look for every
 * ribbon), else the one it brings — 24px on large buttons (fits the 66px row with a two-line
 * label), 20px elsewhere.
 */
function itemIcon(item: RibbonItem, iconOnly = false): ReactNode {
  if (hasIcon(item.id)) {
    return <Icon name={item.id} size={item.size === 'small' || iconOnly ? 20 : 24} />;
  }
  return item.icon;
}

function RibbonButton({
  item,
  mode = 'normal',
  onDone,
}: {
  item: RibbonItem;
  mode?: ButtonMode;
  onDone?: () => void;
}) {
  const disabled = item.enabled === false;
  const icon = itemIcon(item, mode === 'iconOnly');
  // Compact groups, and small buttons that bring their own icon, show only the icon; the label
  // stays for screen readers and tests, and moves into the tooltip. A small button whose icon
  // comes from the icon set keeps its visible label.
  const hideLabel =
    mode !== 'menu' &&
    Boolean(icon) &&
    (mode === 'iconOnly' || (item.size === 'small' && Boolean(item.icon)));
  const sizeClass = mode === 'menu' ? styles.menuItem : item.size === 'small' ? styles.small : '';
  const button = (
    <button
      type="button"
      className={`${styles.button} ${sizeClass}`}
      data-command={item.id}
      aria-disabled={disabled}
      aria-pressed={item.checked ?? undefined}
      aria-keyshortcuts={item.shortcut}
      // Keep keyboard focus in the document (caret, IME) when clicking the ribbon.
      onMouseDown={(e) => e.preventDefault()}
      onClick={() => {
        if (disabled) return;
        item.onSelect();
        onDone?.();
      }}
    >
      {icon && <span className={styles.icon}>{icon}</span>}
      <span className={hideLabel ? styles.srOnly : undefined}>{item.label}</span>
    </button>
  );
  const tip = [hideLabel ? item.label : '', item.tooltip, item.shortcut]
    .filter(Boolean)
    .join(' · ');
  if (!tip || mode === 'menu') return button;
  return (
    <Tooltip.Root>
      <Tooltip.Trigger asChild>{button}</Tooltip.Trigger>
      <Tooltip.Portal>
        <Tooltip.Content className={styles.tooltip} sideOffset={4}>
          {tip}
        </Tooltip.Content>
      </Tooltip.Portal>
    </Tooltip.Root>
  );
}

function itemsClass(g: RibbonGroup): string {
  if (g.items.some((i) => i.render)) return styles.mixed ?? '';
  if (g.items.length > 3 && g.items.every((i) => i.size === 'small')) return styles.compact ?? '';
  return '';
}

/** The commands of a group as a vertical menu (collapsed group and overflow menu). */
function GroupMenu({ group, close }: { group: RibbonGroup; close: () => void }) {
  return (
    <div className={styles.menu} role="group" aria-label={group.label}>
      {group.items.map((i) =>
        i.render ? (
          <div key={i.id} className={styles.control} data-control={i.id}>
            {i.render}
          </div>
        ) : (
          <RibbonButton key={i.id} item={i} mode="menu" onDone={close} />
        ),
      )}
      {group.launcher && (
        <RibbonButton
          item={{
            id: group.launcher.id,
            label: group.launcher.label,
            onSelect: group.launcher.onSelect,
          }}
          mode="menu"
          onDone={close}
        />
      )}
    </div>
  );
}

function RibbonGroupView({ group, level }: { group: RibbonGroup; level: CollapseLevel }) {
  if (level === 'collapsed') {
    const icon = group.items.map((i) => itemIcon(i)).find(Boolean);
    return (
      <div
        className={`${styles.group} ${styles.collapsedGroup}`}
        role="group"
        aria-label={group.label}
        data-group={group.id}
        data-level={level}
      >
        <Popover
          label={group.label}
          testId={`ribbon-group-${group.id}`}
          className={styles.collapsedTrigger}
          trigger={
            <>
              {icon && <span className={styles.icon}>{icon}</span>}
              <span className={styles.collapsedLabel}>
                {group.label}
                <span aria-hidden="true"> ▾</span>
              </span>
            </>
          }
        >
          {(close) => <GroupMenu group={group} close={close} />}
        </Popover>
      </div>
    );
  }
  return (
    <div
      className={`${styles.group} ${level === 'compact' ? styles.compactGroup : ''}`}
      role="group"
      aria-label={group.label}
      data-group={group.id}
      data-level={level}
    >
      <div
        className={[
          styles.groupItems,
          itemsClass(group),
          // Compact level: three or more plain buttons stack in two rows of icons.
          level === 'compact' && !itemsClass(group) && group.items.length > 2 ? styles.stacked : '',
        ]
          .filter(Boolean)
          .join(' ')}
      >
        {group.items.map((i) =>
          i.render ? (
            <div key={i.id} className={styles.control} data-control={i.id}>
              {i.render}
            </div>
          ) : (
            <RibbonButton key={i.id} item={i} mode={level === 'compact' ? 'iconOnly' : 'normal'} />
          ),
        )}
      </div>
      <div className={styles.groupLabel}>
        {group.label}
        {group.launcher && <LauncherButton launcher={group.launcher} />}
      </div>
    </div>
  );
}

/** Cache key of a group's measured widths: changes when its content changes. */
function groupKey(g: RibbonGroup): string {
  return `${g.id}|${g.label}|${g.items.map((i) => `${i.id}:${i.label}:${i.size ?? ''}`).join(',')}`;
}

type WidthCache = Map<string, Partial<GroupWidths>>;

function RibbonPanel({
  tab,
  cache,
  moreLabel,
}: {
  tab: RibbonTab;
  cache: WidthCache;
  moreLabel?: string;
}) {
  // Callback ref: the content element only exists while its tab is active.
  const [el, setEl] = useState<HTMLDivElement | null>(null);
  const [plan, setPlan] = useState<RibbonLayoutPlan>(() => ({
    levels: tab.groups.map(() => 'full'),
    overflowFrom: tab.groups.length,
  }));
  const [width, setWidth] = useState<number | null>(null);
  const passes = useRef(0);
  const keys = tab.groups.map(groupKey);
  const signature = keys.join('\n');

  // Track the available width.
  useLayoutEffect(() => {
    if (!el) return undefined;
    const read = () => {
      const cs = getComputedStyle(el);
      const inner =
        el.clientWidth - (parseFloat(cs.paddingLeft) || 0) - (parseFloat(cs.paddingRight) || 0);
      passes.current = 0;
      setWidth(inner);
    };
    read();
    if (typeof ResizeObserver === 'undefined') {
      window.addEventListener('resize', read);
      return () => window.removeEventListener('resize', read);
    }
    const ro = new ResizeObserver(read);
    ro.observe(el);
    return () => ro.disconnect();
  }, [el]);

  // After every render: record the widths of the groups at their current level,
  // then re-plan. Converges because each level is measured at most once.
  const replan = useCallback(() => {
    if (!el || width === null || width <= 0) return;
    const groups = Array.from(el.querySelectorAll<HTMLElement>(':scope > [data-group]'));
    for (const node of groups) {
      const index = tab.groups.findIndex((g) => g.id === node.dataset.group);
      const key = keys[index];
      const level = node.dataset.level as CollapseLevel | undefined;
      if (key === undefined || !level) continue;
      const measured = node.getBoundingClientRect().width;
      if (measured <= 0) continue;
      cache.set(key, { ...cache.get(key), [level]: measured });
    }
    const widths = keys.map((k) => estimateWidths(cache.get(k) ?? {}));
    const next = planRibbonLayout(widths, width);
    if (passes.current < 12 && !samePlan(next, plan)) {
      passes.current += 1;
      setPlan(next);
    }
    // `keys` is derived from `signature`.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [el, width, plan, signature, cache, tab.groups]);

  useLayoutEffect(() => {
    passes.current = 0;
  }, [signature]);
  useLayoutEffect(replan);

  const levels =
    plan.levels.length === tab.groups.length ? plan.levels : tab.groups.map(() => 'full' as const);
  const overflowFrom = Math.min(plan.overflowFrom, tab.groups.length);
  const hidden = tab.groups.slice(overflowFrom);
  return (
    <Tabs.Content ref={setEl} value={tab.id} className={styles.panel}>
      {tab.groups.slice(0, overflowFrom).map((g, i) => (
        <RibbonGroupView key={g.id} group={g} level={levels[i] ?? 'full'} />
      ))}
      {hidden.length > 0 && (
        <div className={styles.more} data-overflow={hidden.length}>
          <Popover
            label={moreLabel ?? '…'}
            testId="ribbon-more"
            trigger={<span aria-hidden="true">…</span>}
          >
            {(close) => (
              <div className={styles.overflowMenu}>
                {hidden.map((g) => (
                  <section key={g.id} className={styles.overflowSection}>
                    <div className={styles.groupLabel}>{g.label}</div>
                    <GroupMenu group={g} close={close} />
                  </section>
                ))}
              </div>
            )}
          </Popover>
        </div>
      )}
    </Tabs.Content>
  );
}

/**
 * Tabbed toolbar. Every button dispatches a registry command. When the window is
 * narrower than the groups, they collapse progressively (see `planRibbonLayout`):
 * labels first, then whole groups into dropdowns, then trailing groups into a
 * "more" menu — the ribbon never overlaps or scrolls horizontally.
 */
export function Ribbon({ tabs, value, onValueChange, trailing, moreLabel }: RibbonProps) {
  // Measured widths survive re-renders; the map is mutated in layout effects only.
  const [cache] = useState<WidthCache>(() => new Map());
  return (
    <Tooltip.Provider delayDuration={400}>
      <Tabs.Root
        className={styles.ribbon}
        data-testid="ribbon"
        value={value}
        defaultValue={tabs[0]?.id}
        onValueChange={onValueChange}
      >
        <div className={styles.tabStrip}>
          <Tabs.List className={styles.tabList}>
            {tabs.map((t) => (
              <Tabs.Trigger key={t.id} value={t.id} className={styles.tab}>
                {t.label}
              </Tabs.Trigger>
            ))}
          </Tabs.List>
          {trailing}
        </div>
        {tabs.map((t) => (
          <RibbonPanel key={t.id} tab={t} cache={cache} moreLabel={moreLabel} />
        ))}
      </Tabs.Root>
    </Tooltip.Provider>
  );
}
