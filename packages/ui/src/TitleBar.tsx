import {
  useEffect,
  useRef,
  useState,
  type MouseEvent as ReactMouseEvent,
  type ReactNode,
  type RefObject,
} from 'react';
import { Icon } from './icons';
import styles from './TitleBar.module.css';

/** One quick-access command (Save, Undo, Redo, …) drawn as an icon button. */
export interface QuickAccessItem {
  /** Command id; also the icon name unless `icon` is set. */
  id: string;
  /** Icon name when it differs from the command id (a bound script: `script.run`). */
  icon?: string;
  label: string;
  shortcut?: string;
  enabled?: boolean;
  onSelect: () => void;
}

export type SaveState = 'saved' | 'saving' | 'unsaved';

/**
 * The window operations of a frameless desktop window (structurally the platform's
 * `WindowChrome`, ADR-0016). Coordinates are CSS pixels of the page.
 */
export interface WindowControls {
  minimize(): void;
  toggleMaximize(): void;
  close(): void;
  startDragging(): void;
  showSystemMenu(x: number, y: number): void;
  getState(): Promise<{ maximized: boolean; focused: boolean }>;
  onStateChange(handler: (state: { maximized: boolean; focused: boolean }) => void): () => void;
  setMaximizeButton(rect: { x: number; y: number; width: number; height: number } | null): void;
  onMaximizeButton(handler: (state: { hover: boolean; pressed: boolean }) => void): () => void;
  setAppearance(appearance: { dark: boolean; solid: boolean }): Promise<'mica' | 'none'>;
}

/** The title bar as the window caption: window buttons and their localized names. */
export interface TitleBarWindow {
  controls: WindowControls;
  labels: { minimize: string; maximize: string; restore: string; close: string };
}

/**
 * {@link TitleBarProps.window} for a platform window chrome (absent: the web, native frames),
 * with the `titleBar.window.*` labels.
 */
export function titleBarWindow(
  controls: WindowControls | undefined,
  t: (key: string) => string,
): TitleBarWindow | undefined {
  if (!controls) return undefined;
  return {
    controls,
    labels: {
      minimize: t('titleBar.window.minimize'),
      maximize: t('titleBar.window.maximize'),
      restore: t('titleBar.window.restore'),
      close: t('titleBar.window.close'),
    },
  };
}

export interface TitleBarProps {
  /** App id: picks the app mark and (through `data-app` on the root) the accent. */
  app: 'doc' | 'sheet' | 'slide';
  /** App name shown before the document, e.g. "BlueSheet" (or "BlueDrive · BlueSheet"). */
  appName: string;
  /** Document name; empty when nothing is open. */
  docName?: string;
  saveState?: SaveState | null;
  /** Localized text of the save state ("Đã lưu", "Đang lưu…", "Chưa lưu"). */
  saveLabel?: string;
  quickAccess?: QuickAccessItem[];
  /** Accessible name of the quick-access toolbar. */
  quickAccessLabel: string;
  /** Optional search box opener (command search, UI-FRAME-003). */
  search?: { label: string; shortcut?: string; onOpen: () => void };
  /** Right side: collaborators, Comments, Share, account, host buttons. */
  actions?: ReactNode;
  /**
   * Desktop window without a system frame: the bar becomes the caption (drag, double-click,
   * right-click / Alt+Space system menu) and ends with the window buttons. Absent on the web.
   */
  window?: TitleBarWindow;
}

/**
 * The Quick Access Toolbar's buttons: in the title bar, or in its own row below the ribbon
 * (`below`, CORE-UI-005).
 */
export function QuickAccessBar({
  items,
  label,
  below,
}: {
  items: readonly QuickAccessItem[];
  label: string;
  below?: boolean;
}) {
  return (
    <div
      className={below ? `${styles.qat} ${styles.qatBelow}` : styles.qat}
      role="toolbar"
      aria-label={label}
      data-testid={below ? 'qat-below' : 'qat'}
    >
      {items.map((q) => (
        <button
          key={q.id}
          type="button"
          className={styles.qatButton}
          data-command={`qat:${q.id}`}
          aria-label={q.label}
          title={q.shortcut ? `${q.label} (${q.shortcut})` : q.label}
          disabled={q.enabled === false}
          onClick={q.onSelect}
        >
          <Icon name={q.icon ?? q.id} size={16} />
        </button>
      ))}
    </div>
  );
}

/** Elements of the bar that keep their own mouse handling (not part of the caption). */
const INTERACTIVE =
  'button, a, input, select, textarea, [role="button"], [role="menuitem"], [contenteditable="true"]';

/** Whether a press on `target` belongs to the caption (drag / maximise / system menu). */
export function isCaptionTarget(target: EventTarget | null): boolean {
  return target instanceof Element && !target.closest(INTERACTIVE);
}

/**
 * Keeps the window backdrop following the app theme (`data-theme` of the nearest themed
 * ancestor), high contrast and reduced transparency, and tags the bar's parent with the height
 * of the window chrome (`--bo-chrome-h`: title bar + ribbon, the see-through band over Mica).
 */
function useBackdrop(bar: RefObject<HTMLElement | null>, controls: WindowControls | undefined) {
  useEffect(() => {
    const el = bar.current;
    if (!controls || !el) return;
    const themed = el.closest('[data-theme]');
    const media = (q: string) => (typeof matchMedia === 'function' ? matchMedia(q) : null);
    const forced = media('(forced-colors: active)');
    const reduced = media('(prefers-reduced-transparency: reduce)');
    let last = '';
    const apply = () => {
      const theme = themed?.getAttribute('data-theme') ?? 'light';
      const solid = theme === 'hc' || !!forced?.matches || !!reduced?.matches;
      const key = `${theme}:${solid}`;
      if (key === last) return;
      last = key;
      void controls.setAppearance({ dark: theme === 'dark', solid }).catch(() => {});
    };
    apply();
    const themeObserver = themed ? new MutationObserver(apply) : null;
    if (themed)
      themeObserver?.observe(themed, { attributes: true, attributeFilter: ['data-theme'] });
    forced?.addEventListener?.('change', apply);
    reduced?.addEventListener?.('change', apply);
    const parent = el.parentElement;
    let frame = 0;
    const measure = () => {
      cancelAnimationFrame(frame);
      frame = requestAnimationFrame(() => {
        if (!parent) return;
        const top = parent.getBoundingClientRect().top;
        let bottom = top;
        for (const c of parent.querySelectorAll('[data-bo-chrome]')) {
          bottom = Math.max(bottom, c.getBoundingClientRect().bottom);
        }
        parent.style.setProperty('--bo-chrome-h', `${Math.round(bottom - top)}px`);
      });
    };
    const sizes = typeof ResizeObserver === 'function' ? new ResizeObserver(measure) : null;
    const observeChrome = () => {
      sizes?.disconnect();
      if (!parent) return;
      for (const c of parent.querySelectorAll('[data-bo-chrome]')) sizes?.observe(c);
      measure();
    };
    observeChrome();
    // The ribbon comes and goes (focus mode, auto-hide): watch the parent's children.
    const tree = parent ? new MutationObserver(observeChrome) : null;
    if (parent) tree?.observe(parent, { childList: true });
    return () => {
      themeObserver?.disconnect();
      tree?.disconnect();
      sizes?.disconnect();
      cancelAnimationFrame(frame);
      forced?.removeEventListener?.('change', apply);
      reduced?.removeEventListener?.('change', apply);
      parent?.style.removeProperty('--bo-chrome-h');
    };
  }, [bar, controls]);
}

/** Minimise / maximise-restore / close at the right end of the caption. */
function WindowButtons({
  window: w,
  bar,
}: {
  window: TitleBarWindow;
  bar: RefObject<HTMLElement | null>;
}) {
  const { controls, labels } = w;
  const [maximized, setMaximized] = useState(false);
  const [native, setNative] = useState({ hover: false, pressed: false });
  const max = useRef<HTMLButtonElement>(null);
  useEffect(() => {
    let live = true;
    void controls.getState().then((s) => {
      if (live) setMaximized(s.maximized);
    });
    const off = controls.onStateChange((s) => setMaximized(s.maximized));
    const offMax = controls.onMaximizeButton(setNative);
    return () => {
      live = false;
      off();
      offMax();
    };
  }, [controls]);
  // Snap layouts: the shell puts its hit area where the maximise button is drawn.
  useEffect(() => {
    const el = max.current;
    if (!el) return;
    let last = '';
    let frame = 0;
    const report = () => {
      cancelAnimationFrame(frame);
      frame = requestAnimationFrame(() => {
        const r = el.getBoundingClientRect();
        const key = `${r.x},${r.y},${r.width},${r.height},${globalThis.devicePixelRatio ?? 1}`;
        if (key === last) return;
        last = key;
        controls.setMaximizeButton({ x: r.x, y: r.y, width: r.width, height: r.height });
      });
    };
    report();
    const sizes = typeof ResizeObserver === 'function' ? new ResizeObserver(report) : null;
    sizes?.observe(el);
    if (bar.current) sizes?.observe(bar.current);
    globalThis.addEventListener?.('resize', report);
    return () => {
      cancelAnimationFrame(frame);
      sizes?.disconnect();
      globalThis.removeEventListener?.('resize', report);
      controls.setMaximizeButton(null);
    };
  }, [controls, bar, maximized]);
  return (
    <span className={styles.caption} data-testid="window-buttons">
      <button
        type="button"
        className={styles.captionButton}
        data-testid="window-minimize"
        aria-label={labels.minimize}
        title={labels.minimize}
        onClick={() => controls.minimize()}
      >
        <Icon name="window.minimize" size={16} />
      </button>
      <button
        ref={max}
        type="button"
        className={styles.captionButton}
        data-testid="window-maximize"
        data-hover={native.hover ? '' : undefined}
        data-pressed={native.pressed ? '' : undefined}
        aria-label={maximized ? labels.restore : labels.maximize}
        title={maximized ? labels.restore : labels.maximize}
        onClick={() => controls.toggleMaximize()}
      >
        <Icon name={maximized ? 'window.restore' : 'window.maximize'} size={16} />
      </button>
      <button
        type="button"
        className={`${styles.captionButton} ${styles.closeButton}`}
        data-testid="window-close"
        aria-label={labels.close}
        title={labels.close}
        onClick={() => controls.close()}
      >
        <Icon name="window.close" size={16} />
      </button>
    </span>
  );
}

/**
 * The 40px title bar shared by the three apps (docs/ui/DESIGN.md "Khung cửa sổ"): app mark and
 * quick-access commands on the left, document name with its save state in the middle, search
 * and actions on the right. It sits on the app accent band (`--bo-chrome-band-bg`); on the
 * desktop (`window`) it is also the window caption, over Mica on Windows 11 (ADR-0016).
 */
export function TitleBar(p: TitleBarProps) {
  const bar = useRef<HTMLElement>(null);
  const controls = p.window?.controls;
  const [focused, setFocused] = useState(true);
  useBackdrop(bar, controls);
  useEffect(() => {
    if (!controls) return;
    const off = controls.onStateChange((s) => setFocused(s.focused));
    // Alt+Space: the system menu under the app mark, as on a framed window.
    const onKey = (e: KeyboardEvent) => {
      if (!e.altKey || e.ctrlKey || e.metaKey || e.code !== 'Space') return;
      e.preventDefault();
      e.stopPropagation();
      const r = bar.current?.getBoundingClientRect();
      controls.showSystemMenu(r?.left ?? 0, r?.bottom ?? 0);
    };
    globalThis.addEventListener?.('keydown', onKey, true);
    return () => {
      off();
      globalThis.removeEventListener?.('keydown', onKey, true);
    };
  }, [controls]);
  const caption = controls
    ? {
        onMouseDown: (e: ReactMouseEvent) => {
          if (e.button !== 0 || !isCaptionTarget(e.target)) return;
          e.preventDefault();
          if (e.detail >= 2) controls.toggleMaximize();
          else controls.startDragging();
        },
        onContextMenu: (e: ReactMouseEvent) => {
          if (!isCaptionTarget(e.target)) return;
          e.preventDefault();
          controls.showSystemMenu(e.clientX, e.clientY);
        },
      }
    : {};
  return (
    <header
      ref={bar}
      className={styles.bar}
      data-testid="title-bar"
      data-bo-chrome=""
      data-caption={controls ? '' : undefined}
      data-focused={controls && !focused ? 'false' : undefined}
      {...caption}
    >
      <span className={styles.app}>
        <Icon name={`app.${p.app}`} size={20} />
        <span className={styles.appName}>{p.appName}</span>
      </span>
      {p.quickAccess && p.quickAccess.length > 0 && (
        <QuickAccessBar items={p.quickAccess} label={p.quickAccessLabel} />
      )}
      <span className={styles.doc}>
        <span className={styles.docName} data-testid="doc-name">
          {p.docName ?? ''}
        </span>
        {p.docName && p.saveState && p.saveLabel && (
          <span className={styles.save} data-state={p.saveState} data-testid="save-state">
            {p.saveLabel}
          </span>
        )}
      </span>
      {p.search && (
        <button
          type="button"
          className={styles.search}
          data-testid="title-search"
          onClick={p.search.onOpen}
          aria-keyshortcuts={p.search.shortcut}
        >
          <Icon name="edit.find" size={16} />
          <span>{p.search.label}</span>
        </button>
      )}
      <span className={styles.actions}>{p.actions}</span>
      {p.window && <WindowButtons window={p.window} bar={bar} />}
    </header>
  );
}
