import * as Tabs from '@radix-ui/react-tabs';
import * as Tooltip from '@radix-ui/react-tooltip';
import { ChevronDown, ChevronUp, PanelTopClose, Rows2, Rows3 } from 'lucide-react';
import {
  useCallback,
  useEffect,
  useLayoutEffect,
  useRef,
  useState,
  type MouseEvent as ReactMouseEvent,
  type ReactNode,
} from 'react';
import { useDensity } from './density';
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
import {
  arrangeTabs,
  decideActiveTab,
  EMPTY_TRACKER,
  RIBBON_CONTEXTS,
  type ContextTracker,
  type RibbonContext,
} from './ribbonContext';
import {
  setRibbonPrefs,
  toggleRibbonAutoHide,
  toggleRibbonCollapsed,
  useRibbonPrefs,
} from './ribbonMode';
import { KeyTipsLayer } from './KeyTipsLayer';
import { ContextMenu, type MenuLine } from './ContextMenu';
import { applyRibbonCustomization, type RibbonCustomization } from './ribbonCustomize';

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
  /**
   * Row of a two-row group (1 = top, 2 = bottom): a group whose items set it is laid out as
   * those two rows (e.g. font name, size on top; bold, italic … below), as wide as its
   * longest row.
   */
  row?: 1 | 2;
  /**
   * Kept in the simplified (single-row) ribbon. When no item of a group sets it, the first
   * {@link SIMPLIFIED_COUNT} items are kept; the others move into the group's "…" menu.
   */
  simplified?: boolean;
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
  /**
   * Contextual tab (CORE-UI-001): shown only while such an object is selected, grouped after
   * the regular tabs under the context's header label, and activated when it appears.
   */
  context?: RibbonContext;
}

/** Ribbon strings (from i18n, see `ribbonLabels`). */
export interface RibbonLabels {
  /** Accessible name of the overflow ("more commands") button; the visible glyph is "…". */
  more?: string;
  /** Header label of each context ("Table tools"…). */
  contexts?: Partial<Record<RibbonContext, string>>;
  /** Toggle button of the simplified (single-row) ribbon; shown when set. */
  simplified?: string;
  /** Collapse / expand button (Ctrl+F1); shown when both are set. */
  collapse?: string;
  expand?: string;
  /** Auto-hide toggle (UI-RIBBON-003); shown when set. */
  autoHide?: string;
  /** The thin bar of the auto-hidden ribbon: click it to show the ribbon. */
  showRibbon?: string;
}

export interface RibbonProps {
  tabs: RibbonTab[];
  value?: string;
  onValueChange?: (tab: string) => void;
  /** Content shown at the right of the tab strip (e.g. host Share/Close buttons). */
  trailing?: ReactNode;
  /** Accessible name of the overflow ("more commands") button; the visible glyph is "…". */
  moreLabel?: string;
  labels?: RibbonLabels;
  /** Forces the simplified / full layout (default: the remembered preference). */
  simplified?: boolean;
  /**
   * Contexts whose first tab is activated when they appear (default: all). Editors leave out
   * the ones the caret moves through while typing (table cells), which then only show.
   */
  autoActivate?: readonly RibbonContext[];
  /**
   * The File button at the start of the tab strip (CORE-UI-002): opens the full-window
   * File screen (`Backstage`) instead of a ribbon tab.
   */
  fileMenu?: RibbonFileMenu;
  /** Alt / F10 KeyTips (CORE-UI-004); on by default. */
  keyTips?: boolean;
  /** KeyTip codes by id (`tab:<id>`, `file`, command ids) replacing the derived ones. */
  keyTipOverrides?: Readonly<Record<string, string>>;
  /**
   * The user's customisation (CORE-UI-005), applied over `tabs` (contextual tabs unchanged);
   * `resolveItem` supplies commands of custom groups that no ribbon tab has.
   */
  customization?: RibbonCustomization | null;
  resolveItem?: (id: string) => RibbonItem | undefined;
  /** Right-click menu of the ribbon (customise ribbon / Quick Access Toolbar). */
  customizeMenu?: RibbonCustomizeMenu;
}

export interface RibbonCustomizeMenu {
  labels: {
    /** Accessible name of the menu. */
    menu: string;
    customizeRibbon: string;
    customizeQat: string;
    addToQat: string;
    removeFromQat: string;
  };
  onCustomizeRibbon: () => void;
  onCustomizeQat: () => void;
  /** The command is on the Quick Access Toolbar. */
  inQat: (id: string) => boolean;
  onToggleQat: (id: string) => void;
}

export interface RibbonFileMenu {
  label: string;
  onOpen: () => void;
  /** The File screen is shown. */
  open?: boolean;
}

/** Commands per group kept in the simplified ribbon when no item is flagged `simplified`. */
export const SIMPLIFIED_COUNT = 3;

/** The items of a group shown in the simplified ribbon row, and the ones moved into "…". */
export function simplifiedItems(group: RibbonGroup): { shown: RibbonItem[]; rest: RibbonItem[] } {
  const flagged = group.items.some((i) => i.simplified);
  const keep = (i: RibbonItem, n: number) => (flagged ? !!i.simplified : n < SIMPLIFIED_COUNT);
  const shown: RibbonItem[] = [];
  const rest: RibbonItem[] = [];
  group.items.forEach((i, n) => (keep(i, n) ? shown : rest).push(i));
  return { shown, rest };
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
  if (g.items.some((i) => i.row)) return styles.twoRows ?? '';
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
        {group.items.some((i) => i.row)
          ? ([1, 2] as const).map((row) => (
              <div key={row} className={styles.itemRow}>
                {group.items.filter((i) => (i.row ?? 1) === row).map((i) => groupItem(i, level))}
              </div>
            ))
          : group.items.map((i) => groupItem(i, level))}
      </div>
      <div className={styles.groupLabel}>
        {group.label}
        {group.launcher && <LauncherButton launcher={group.launcher} />}
      </div>
    </div>
  );
}

function groupItem(i: RibbonItem, level: CollapseLevel) {
  return i.render ? (
    <div key={i.id} className={styles.control} data-control={i.id}>
      {i.render}
    </div>
  ) : (
    <RibbonButton key={i.id} item={i} mode={level === 'compact' ? 'iconOnly' : 'normal'} />
  );
}

/**
 * A group in the simplified ribbon: one row of its most-used commands (icons, label in the
 * tooltip) and a "…" menu with the others. `compact` renders like `full` (already icon-only).
 */
function SimplifiedGroupView({
  group,
  level,
  moreLabel,
}: {
  group: RibbonGroup;
  level: CollapseLevel;
  moreLabel?: string;
}) {
  if (level === 'collapsed') return <RibbonGroupView group={group} level={level} />;
  const { shown, rest } = simplifiedItems(group);
  const restGroup: RibbonGroup = { ...group, items: rest };
  return (
    <div
      className={`${styles.group} ${styles.simpleGroup}`}
      role="group"
      aria-label={group.label}
      data-group={group.id}
      data-level={level}
    >
      {shown.map((i) =>
        i.render ? (
          <div key={i.id} className={styles.control} data-control={i.id}>
            {i.render}
          </div>
        ) : (
          <RibbonButton key={i.id} item={{ ...i, size: 'small' }} mode="iconOnly" />
        ),
      )}
      {(rest.length > 0 || group.launcher) && (
        <Popover
          label={moreLabel ? `${group.label} · ${moreLabel}` : group.label}
          testId={`ribbon-group-more-${group.id}`}
          className={styles.simpleMore}
          trigger={<span aria-hidden="true">…</span>}
        >
          {(close) => <GroupMenu group={restGroup} close={close} />}
        </Popover>
      )}
    </div>
  );
}

/** Cache key of a group's measured widths: changes when its content or the layout changes. */
function groupKey(g: RibbonGroup, simplified: boolean): string {
  const items = g.items.map((i) => `${i.id}:${i.label}:${i.size ?? ''}`).join(',');
  return `${simplified ? 's' : 'f'}|${g.id}|${g.label}|${items}`;
}

type WidthCache = Map<string, Partial<GroupWidths>>;

function RibbonPanel({
  tab,
  cache,
  moreLabel,
  simplified,
}: {
  tab: RibbonTab;
  cache: WidthCache;
  moreLabel?: string;
  simplified: boolean;
}) {
  // Callback ref: the content element only exists while its tab is active.
  const [el, setEl] = useState<HTMLDivElement | null>(null);
  const [plan, setPlan] = useState<RibbonLayoutPlan>(() => ({
    levels: tab.groups.map(() => 'full'),
    overflowFrom: tab.groups.length,
  }));
  const [width, setWidth] = useState<number | null>(null);
  const passes = useRef(0);
  const keys = tab.groups.map((g) => groupKey(g, simplified));
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

  // A group that changes size by itself (a gallery filled in once its data arrives) asks for a
  // new plan too; the observer reports sizes asynchronously, it never forces a layout.
  const [resized, setResized] = useState(0);
  useLayoutEffect(() => {
    if (!el || typeof ResizeObserver === 'undefined') return undefined;
    let initial = true;
    const ro = new ResizeObserver(() => {
      if (initial) {
        initial = false;
        return;
      }
      passes.current = 0;
      setResized((n) => n + 1);
    });
    for (const node of el.querySelectorAll<HTMLElement>(':scope > [data-group]')) ro.observe(node);
    return () => ro.disconnect();
  }, [el, signature, plan]);

  // After a render that may change the groups' widths (other groups, labels or sizes, another
  // level, another width): record the widths of the groups at their current level, then
  // re-plan. Converges because each level is measured at most once. Not after every render:
  // reading the widths forces a layout, which would delay every keystroke in the editors.
  const replan = useCallback(() => {
    if (!el || width === null || width <= 0) return;
    const groups = Array.from(el.querySelectorAll<HTMLElement>(':scope > [data-group]'));
    for (const node of groups) {
      // A group's key is s|f | id | label | items: it names the group's id.
      const index = keys.findIndex((k) => k.split('|')[1] === node.dataset.group);
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
  }, [el, width, plan, signature, cache, resized]);

  useLayoutEffect(() => {
    passes.current = 0;
  }, [signature]);
  useLayoutEffect(replan, [replan]);

  const levels =
    plan.levels.length === tab.groups.length ? plan.levels : tab.groups.map(() => 'full' as const);
  const overflowFrom = Math.min(plan.overflowFrom, tab.groups.length);
  const hidden = tab.groups.slice(overflowFrom);
  return (
    <Tabs.Content
      ref={setEl}
      value={tab.id}
      className={`${styles.panel} ${simplified ? styles.simplePanel : ''}`}
      data-simplified={simplified ? 'true' : undefined}
      data-context={tab.context}
    >
      {tab.groups
        .slice(0, overflowFrom)
        .map((g, i) =>
          simplified ? (
            <SimplifiedGroupView
              key={g.id}
              group={g}
              level={levels[i] ?? 'full'}
              moreLabel={moreLabel}
            />
          ) : (
            <RibbonGroupView key={g.id} group={g} level={levels[i] ?? 'full'} />
          ),
        )}
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

interface TabState {
  tracker: ContextTracker;
  /** Active tab of an uncontrolled ribbon. */
  own: string | undefined;
  /** Tab decided by the ribbon over `base` (the parent's value when decided). */
  override: { tab: string; base: string | undefined } | null;
}

function sameTracker(a: ContextTracker, b: ContextTracker): boolean {
  return (
    a.lastPlain === b.lastPlain &&
    a.present.length === b.present.length &&
    a.present.every((c, i) => c === b.present[i])
  );
}

/** The auto-hidden ribbon shown over the document (UI-RIBBON-003); else its content as is. */
function RevealedPanel({ revealed, children }: { revealed: boolean; children: ReactNode }) {
  if (!revealed) return <>{children}</>;
  return (
    <div className={styles.revealed} data-testid="ribbon-revealed">
      {children}
    </div>
  );
}

/** Ctrl+F1 handled once per key press even when several ribbons are mounted. */
const handledKeys = new WeakSet<Event>();

/**
 * Tabbed toolbar. Every button dispatches a registry command. When the window is
 * narrower than the groups, they collapse progressively (see `planRibbonLayout`):
 * labels first, then whole groups into dropdowns, then trailing groups into a
 * "more" menu — the ribbon never overlaps or scrolls horizontally.
 *
 * Contextual tabs (`context`) follow the regular ones under their header label and are
 * activated when their object gets selected (see `decideActiveTab`). The simplified layout
 * shows one row per tab; the collapsed ribbon keeps only the tab strip (Ctrl+F1) and shows a
 * tab's commands over the document when the tab is clicked.
 */
export function Ribbon({
  tabs: builtTabs,
  value,
  onValueChange,
  trailing,
  moreLabel,
  labels,
  simplified: forceSimplified,
  autoActivate,
  fileMenu,
  keyTips = true,
  keyTipOverrides,
  customization,
  resolveItem,
  customizeMenu,
}: RibbonProps) {
  const [menu, setMenu] = useState<{ x: number; y: number; command?: string } | null>(null);
  const tabs = customization
    ? applyRibbonCustomization(builtTabs, customization, resolveItem)
    : builtTabs;
  // Measured widths survive re-renders; the map is mutated in layout effects only.
  const [cache] = useState<WidthCache>(() => new Map());
  const prefs = useRibbonPrefs();
  const simplified = forceSimplified ?? prefs.simplified;
  const autoHide = prefs.autoHide;
  // The auto-hidden ribbon wins over the collapsed one.
  const collapsed = prefs.collapsed && !autoHide;
  const more = labels?.more ?? moreLabel;
  const controlled = value !== undefined;
  const [st, setSt] = useState<TabState>(() => ({
    tracker: EMPTY_TRACKER,
    own: tabs[0]?.id,
    override: null,
  }));
  // The parent's (or own) tab, unless the ribbon decided another one the parent has not
  // adopted yet (a context appeared, or the active one went away).
  const base = controlled ? value : st.own;
  const requested = st.override && st.override.base === base ? st.override.tab : base;
  const decision = decideActiveTab(tabs, requested, st.tracker, autoActivate);
  const active = decision.value;
  const override = active !== base && active !== undefined ? { tab: active, base } : null;
  const own = controlled ? st.own : active;
  // Adjust the state during render (no effect, no extra commit) when the decision moved on.
  if (
    !sameTracker(decision.tracker, st.tracker) ||
    own !== st.own ||
    override?.tab !== st.override?.tab ||
    override?.base !== st.override?.base
  ) {
    setSt({ tracker: decision.tracker, own, override });
  }
  // Tell a controlled parent about the decided tab.
  useEffect(() => {
    if (controlled && active !== undefined && active !== value) onValueChange?.(active);
  }, [controlled, active, value, onValueChange]);
  const [peekOpen, setPeek] = useState(false);
  const peek = peekOpen && collapsed;
  // Auto-hide mode: the ribbon is shown over the document after a click on the bar or Alt.
  const [revealedOpen, setRevealed] = useState(false);
  const revealed = revealedOpen && autoHide;
  const hidden = autoHide && !revealed;
  const downOnActive = useRef<boolean | null>(null);
  const root = useRef<HTMLDivElement>(null);
  // The ribbon keeps the height of its tallest tab (per layout and density): switching tabs,
  // or a contextual tab appearing under the pointer, never moves the document.
  const density = useDensity();
  const heightKey = `${simplified ? 's' : 'f'}|${density}`;
  const [tallest, setTallest] = useState<Record<string, number>>({});
  const [bodyEl, setBodyEl] = useState<HTMLDivElement | null>(null);
  useEffect(() => {
    if (!bodyEl || typeof ResizeObserver === 'undefined') return undefined;
    const ro = new ResizeObserver(() => {
      const panel = bodyEl.querySelector<HTMLElement>(
        ':scope > [role="tabpanel"][data-state="active"]',
      );
      const h = panel?.offsetHeight ?? 0;
      if (h > 0) setTallest((m) => ((m[heightKey] ?? 0) >= h ? m : { ...m, [heightKey]: h }));
    });
    ro.observe(bodyEl);
    return () => ro.disconnect();
  }, [bodyEl, heightKey]);

  const select = (v: string) => {
    setSt((s) => ({ ...s, own: controlled ? s.own : v, override: null }));
    onValueChange?.(v);
  };

  // Ctrl+F1: collapse / expand the ribbon.
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (!(e.ctrlKey && e.key === 'F1') || e.altKey || e.shiftKey || e.metaKey) return;
      e.preventDefault();
      setPeek(false);
      if (handledKeys.has(e)) return;
      handledKeys.add(e);
      toggleRibbonCollapsed();
    };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, []);

  // The collapsed ribbon (or the auto-hidden one) shown over the document closes on outside
  // click and Escape.
  const floating = peek || revealed;
  useEffect(() => {
    if (!floating) return undefined;
    const onDown = (e: MouseEvent) => {
      const t = e.target;
      if (!(t instanceof Node) || root.current?.contains(t)) return;
      if (t instanceof Element && t.closest('[data-bo-popover]')) return;
      setPeek(false);
      setRevealed(false);
    };
    const onKey = (e: KeyboardEvent) => {
      if (e.key !== 'Escape') return;
      setPeek(false);
      setRevealed(false);
    };
    window.addEventListener('mousedown', onDown, true);
    window.addEventListener('keydown', onKey);
    return () => {
      window.removeEventListener('mousedown', onDown, true);
      window.removeEventListener('keydown', onKey);
    };
  }, [floating]);

  const arranged = arrangeTabs(tabs);
  const trigger = (t: RibbonTab) => (
    <Tabs.Trigger
      key={t.id}
      value={t.id}
      className={styles.tab}
      data-tab={t.id}
      data-context={t.context}
      // Runs before the tab activates on mouse down: was it the shown tab already?
      onMouseDown={() => {
        downOnActive.current = t.id === active;
      }}
      onClick={() => {
        const again = downOnActive.current ?? t.id === active;
        downOnActive.current = null;
        if (collapsed) setPeek((p) => (again ? !p : true));
      }}
      // Double-click a tab: collapse / expand the ribbon.
      onDoubleClick={() => {
        setPeek(false);
        toggleRibbonCollapsed();
      }}
    >
      {t.label}
    </Tabs.Trigger>
  );
  const showBody = (!collapsed || peek) && !hidden;
  const stripTabs = [...arranged.plain, ...arranged.contexts.flatMap((c) => c.tabs)];
  // Running a command from the ribbon shown over the document closes it.
  const onBodyClick = (e: ReactMouseEvent) => {
    if (!floating) return;
    const t = e.target;
    if (t instanceof Element && t.closest('button[data-command]')) {
      setPeek(false);
      setRevealed(false);
    }
  };
  return (
    <Tooltip.Provider delayDuration={400}>
      <Tabs.Root
        ref={root}
        className={styles.ribbon}
        data-testid="ribbon"
        data-f6-region="ribbon"
        data-simplified={simplified ? 'true' : undefined}
        data-collapsed={collapsed ? 'true' : undefined}
        data-autohide={autoHide ? 'true' : undefined}
        data-revealed={revealed ? 'true' : undefined}
        value={active}
        onValueChange={select}
        onContextMenu={
          customizeMenu
            ? (e) => {
                const t = e.target;
                if (t instanceof Element && t.closest('input, textarea, select')) return;
                e.preventDefault();
                const el =
                  t instanceof Element ? t.closest<HTMLElement>('button[data-command]') : null;
                const command = el?.dataset.launcher ? undefined : el?.dataset.command;
                setMenu({ x: e.clientX, y: e.clientY, command });
              }
            : undefined
        }
      >
        {hidden && (
          <button
            type="button"
            className={styles.autoHideBar}
            data-testid="ribbon-autohide-bar"
            aria-expanded={false}
            aria-label={labels?.showRibbon}
            title={labels?.showRibbon ? `${labels.showRibbon} (Alt)` : undefined}
            onMouseDown={(e) => e.preventDefault()}
            onClick={() => setRevealed(true)}
          />
        )}
        {!hidden && (
          <RevealedPanel revealed={revealed}>
            <div className={styles.tabStrip}>
              {fileMenu && (
                <button
                  type="button"
                  className={`${styles.tab} ${styles.fileTab}`}
                  data-testid="file-tab"
                  aria-haspopup="dialog"
                  aria-expanded={!!fileMenu.open}
                  onClick={() => {
                    setPeek(false);
                    setRevealed(false);
                    fileMenu.onOpen();
                  }}
                >
                  {fileMenu.label}
                </button>
              )}
              <Tabs.List className={styles.tabList}>
                {arranged.plain.map(trigger)}
                {arranged.contexts.map((c) => (
                  <div
                    key={c.context}
                    className={styles.contextGroup}
                    data-context-group={c.context}
                    role="presentation"
                  >
                    {labels?.contexts?.[c.context] && (
                      <span className={styles.contextLabel} aria-hidden="true">
                        {labels.contexts[c.context]}
                      </span>
                    )}
                    {c.tabs.map(trigger)}
                  </div>
                ))}
              </Tabs.List>
              {labels?.simplified && (
                <button
                  type="button"
                  className={styles.modeButton}
                  data-testid="ribbon-simplified"
                  aria-pressed={simplified}
                  aria-label={labels.simplified}
                  title={labels.simplified}
                  onMouseDown={(e) => e.preventDefault()}
                  onClick={() => setRibbonPrefs({ simplified: !simplified })}
                >
                  {simplified ? <Rows3 size={16} /> : <Rows2 size={16} />}
                </button>
              )}
              {labels?.collapse && labels.expand && (
                <button
                  type="button"
                  className={styles.modeButton}
                  data-testid="ribbon-collapse"
                  aria-expanded={!collapsed}
                  aria-keyshortcuts="Control+F1"
                  aria-label={collapsed ? labels.expand : labels.collapse}
                  title={`${collapsed ? labels.expand : labels.collapse} (Ctrl+F1)`}
                  onMouseDown={(e) => e.preventDefault()}
                  onClick={() => {
                    setPeek(false);
                    toggleRibbonCollapsed();
                  }}
                >
                  {collapsed ? <ChevronDown size={16} /> : <ChevronUp size={16} />}
                </button>
              )}
              {labels?.autoHide && (
                <button
                  type="button"
                  className={styles.modeButton}
                  data-testid="ribbon-autohide"
                  aria-pressed={autoHide}
                  aria-label={labels.autoHide}
                  title={labels.autoHide}
                  onMouseDown={(e) => e.preventDefault()}
                  onClick={() => {
                    setPeek(false);
                    setRevealed(false);
                    toggleRibbonAutoHide();
                  }}
                >
                  <PanelTopClose size={16} />
                </button>
              )}
              {trailing}
            </div>
            {showBody && (
              <div
                ref={setBodyEl}
                style={{ minHeight: tallest[heightKey] }}
                className={peek ? styles.peek : styles.body}
                data-testid={peek ? 'ribbon-peek' : undefined}
                onClick={onBodyClick}
              >
                {tabs.map((t) => (
                  <RibbonPanel
                    key={t.id}
                    tab={t}
                    cache={cache}
                    moreLabel={more}
                    simplified={simplified}
                  />
                ))}
              </div>
            )}
          </RevealedPanel>
        )}
        {keyTips && (
          <KeyTipsLayer
            root={root}
            tabs={stripTabs}
            file={fileMenu ? { label: fileMenu.label, onOpen: fileMenu.onOpen } : undefined}
            overrides={keyTipOverrides}
            onTab={(id) => {
              select(id);
              if (collapsed) setPeek(true);
            }}
            // Auto-hidden ribbon: Alt / F10 shows it first (same render as the KeyTips).
            onShow={autoHide ? () => setRevealed(true) : undefined}
          />
        )}
      </Tabs.Root>
      {menu && customizeMenu && (
        <ContextMenu
          x={menu.x}
          y={menu.y}
          label={customizeMenu.labels.menu}
          testId="context-menu-ribbon"
          onClose={() => setMenu(null)}
          items={customizeLines(customizeMenu, menu.command)}
        />
      )}
    </Tooltip.Provider>
  );
}

function customizeLines(m: RibbonCustomizeMenu, command: string | undefined): MenuLine[] {
  const lines: MenuLine[] = [];
  if (command) {
    const has = m.inQat(command);
    lines.push(
      {
        id: has ? 'ribbon.removeFromQat' : 'ribbon.addToQat',
        label: has ? m.labels.removeFromQat : m.labels.addToQat,
        onSelect: () => m.onToggleQat(command),
      },
      { separator: true },
    );
  }
  lines.push(
    {
      id: 'options.customizeRibbon',
      label: m.labels.customizeRibbon,
      onSelect: m.onCustomizeRibbon,
    },
    { id: 'options.customizeQat', label: m.labels.customizeQat, onSelect: m.onCustomizeQat },
  );
  return lines;
}

/** Ribbon strings from the shared i18n keys (`ribbon.mode.*`, `ribbon.context.*`). */
export function ribbonLabels(t: (key: string) => string): RibbonLabels {
  const contexts: Partial<Record<RibbonContext, string>> = {};
  for (const c of RIBBON_CONTEXTS) contexts[c] = t(`ribbon.context.${c}`);
  return {
    more: t('ribbon.more'),
    contexts,
    simplified: t('ribbon.mode.simplified'),
    collapse: t('ribbon.mode.collapse'),
    expand: t('ribbon.mode.expand'),
    autoHide: t('ribbon.mode.autoHide'),
    showRibbon: t('ribbon.mode.showRibbon'),
  };
}
