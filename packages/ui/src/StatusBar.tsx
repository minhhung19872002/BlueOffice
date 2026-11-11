import {
  useEffect,
  useLayoutEffect,
  useRef,
  useState,
  type KeyboardEvent as ReactKeyboardEvent,
  type ReactNode,
} from 'react';
import styles from './StatusBar.module.css';

/** One line of the "customise status bar" checklist. */
export interface StatusBarMenuItem {
  id: string;
  label: string;
  checked: boolean;
  /** Current value shown at the right (page number, sum, …). */
  value?: string;
}

export interface StatusBarCustomize {
  /** Accessible name of the checklist. */
  label: string;
  items: StatusBarMenuItem[];
  onToggle: (id: string) => void;
  /** "Reset" line at the end (omitted: none). */
  resetLabel?: string;
  onReset?: () => void;
}

export interface StatusBarProps {
  left?: ReactNode;
  right?: ReactNode;
  /** Right click, Shift+F10 or the Menu key on the bar opens this checklist (CORE-UI-007). */
  customize?: StatusBarCustomize;
}

/**
 * Bottom status bar: 24px (docs/ui/DESIGN.md "Khung cửa sổ"), document info left, view
 * controls right. With `customize`, the user picks the items shown.
 */
export function StatusBar({ left, right, customize }: StatusBarProps) {
  const [menu, setMenu] = useState<{ x: number; y: number } | null>(null);
  const bar = useRef<HTMLElement>(null);
  const openAt = (x: number, y: number) => customize && setMenu({ x, y });
  const onKeyDown = (e: ReactKeyboardEvent<HTMLElement>) => {
    if (!customize || menu) return;
    if (e.key === 'ContextMenu' || (e.shiftKey && e.key === 'F10')) {
      e.preventDefault();
      e.stopPropagation();
      const r = (e.target as HTMLElement).getBoundingClientRect();
      openAt(r.left, r.top);
    }
  };
  return (
    <footer
      ref={bar}
      role="status"
      data-testid="status-bar"
      data-f6-region="status"
      className={styles.bar}
      // Fixed by the design system (also asserted by tests): 24px.
      style={{ height: 24, minHeight: 24 }}
      tabIndex={customize ? -1 : undefined}
      aria-haspopup={customize ? 'menu' : undefined}
      onContextMenu={(e) => {
        if (!customize) return;
        e.preventDefault();
        openAt(e.clientX, e.clientY);
      }}
      onKeyDown={onKeyDown}
    >
      <div className={styles.left}>{left}</div>
      <div className={styles.right}>{right}</div>
      {menu && customize && (
        <StatusBarMenu
          at={menu}
          customize={customize}
          onClose={(refocus) => {
            setMenu(null);
            if (refocus) bar.current?.focus();
          }}
        />
      )}
    </footer>
  );
}

function StatusBarMenu({
  at,
  customize,
  onClose,
}: {
  at: { x: number; y: number };
  customize: StatusBarCustomize;
  onClose: (refocus: boolean) => void;
}) {
  const ref = useRef<HTMLDivElement>(null);
  const [pos, setPos] = useState<{ left: number; top: number } | null>(null);
  const [active, setActive] = useState(0);
  const count = customize.items.length + (customize.onReset ? 1 : 0);
  const closeRef = useRef(onClose);
  useEffect(() => {
    closeRef.current = onClose;
  }, [onClose]);

  // The bar is at the bottom of the window: the checklist opens upwards, inside the window.
  useLayoutEffect(() => {
    const el = ref.current;
    if (!el) return;
    const vw = window.innerWidth || 1024;
    const vh = window.innerHeight || 768;
    setPos({
      left: Math.max(4, Math.min(at.x, vw - el.offsetWidth - 4)),
      top: Math.max(4, Math.min(at.y - el.offsetHeight, vh - el.offsetHeight - 4)),
    });
    el.focus();
  }, [at]);

  useEffect(() => {
    const down = (e: MouseEvent) => {
      if (!ref.current?.contains(e.target as Node)) closeRef.current(false);
    };
    document.addEventListener('mousedown', down, true);
    return () => document.removeEventListener('mousedown', down, true);
  }, []);

  const select = (i: number) => {
    const item = customize.items[i];
    if (item) customize.onToggle(item.id);
    else customize.onReset?.();
  };

  return (
    <div
      ref={ref}
      role="menu"
      tabIndex={-1}
      aria-label={customize.label}
      data-testid="status-bar-menu"
      className={styles.menu}
      style={pos ? pos : { left: 0, top: 0, visibility: 'hidden' }}
      onContextMenu={(e) => {
        e.preventDefault();
        e.stopPropagation();
      }}
      onKeyDown={(e) => {
        e.stopPropagation();
        if (e.key === 'Escape' || e.key === 'Tab') {
          e.preventDefault();
          onClose(true);
        } else if (e.key === 'ArrowDown') {
          e.preventDefault();
          setActive((a) => (a + 1) % count);
        } else if (e.key === 'ArrowUp') {
          e.preventDefault();
          setActive((a) => (a - 1 + count) % count);
        } else if (e.key === 'Home') {
          e.preventDefault();
          setActive(0);
        } else if (e.key === 'End') {
          e.preventDefault();
          setActive(count - 1);
        } else if (e.key === 'Enter' || e.key === ' ') {
          e.preventDefault();
          select(active);
        }
      }}
    >
      <div className={styles.heading}>{customize.label}</div>
      {customize.items.map((it, i) => (
        <div
          key={it.id}
          role="menuitemcheckbox"
          aria-checked={it.checked}
          data-testid={`status-item-${it.id}`}
          data-active={i === active}
          className={styles.item}
          onMouseEnter={() => setActive(i)}
          onClick={() => select(i)}
        >
          <span className={styles.check} aria-hidden="true">
            {it.checked ? '✓' : ''}
          </span>
          <span className={styles.label}>{it.label}</span>
          {it.value !== undefined && <span className={styles.value}>{it.value}</span>}
        </div>
      ))}
      {customize.onReset && (
        <>
          <div className={styles.sep} role="separator" />
          <div
            role="menuitem"
            data-testid="status-item-reset"
            data-active={active === customize.items.length}
            className={styles.item}
            onMouseEnter={() => setActive(customize.items.length)}
            onClick={() => select(customize.items.length)}
          >
            <span className={styles.check} aria-hidden="true" />
            <span className={styles.label}>{customize.resetLabel}</span>
          </div>
        </>
      )}
    </div>
  );
}

export interface StatusZoomProps {
  /** Zoom as a factor (1 = 100%). */
  zoom: number;
  min: number;
  max: number;
  onZoom: (zoom: number) => void;
  /** Accessible names. */
  labels: { slider: string; zoomIn: string; zoomOut: string; percent: string };
  /** Parts to show (the status bar checklist turns them on and off separately). */
  slider?: boolean;
  percent?: boolean;
  /** Click on the percentage (back to 100%, a zoom dialog, …). */
  onPercent?: () => void;
  /** Step of the − / + buttons (default 0.1). */
  step?: number;
  /** The − / + buttons run this instead of stepping (the app's own zoom commands). */
  onStep?: (direction: 1 | -1) => void;
}

/** Zoom slider with − / + and the percentage (status bar, right side). */
export function StatusZoom({
  zoom,
  min,
  max,
  onZoom,
  labels,
  slider = true,
  percent = true,
  onPercent,
  step = 0.1,
  onStep,
}: StatusZoomProps) {
  const clamp = (z: number) => Math.min(max, Math.max(min, Math.round(z * 100) / 100));
  const pct = Number.isFinite(zoom) ? Math.round(zoom * 100) : 100;
  return (
    <span className={styles.zoom}>
      {slider && (
        <>
          <button
            type="button"
            className={styles.button}
            data-testid="status-zoom-out"
            aria-label={labels.zoomOut}
            title={labels.zoomOut}
            disabled={zoom <= min}
            onClick={() => (onStep ? onStep(-1) : onZoom(clamp(zoom - step)))}
          >
            −
          </button>
          <input
            type="range"
            className={styles.slider}
            data-testid="status-zoom-slider"
            aria-label={labels.slider}
            min={Math.round(min * 100)}
            max={Math.round(max * 100)}
            step={5}
            value={pct}
            onChange={(e) => onZoom(clamp(Number(e.target.value) / 100))}
          />
          <button
            type="button"
            className={styles.button}
            data-testid="status-zoom-in"
            aria-label={labels.zoomIn}
            title={labels.zoomIn}
            disabled={zoom >= max}
            onClick={() => (onStep ? onStep(1) : onZoom(clamp(zoom + step)))}
          >
            +
          </button>
        </>
      )}
      {percent && (
        <button
          type="button"
          className={styles.button}
          data-testid="status-zoom"
          aria-label={labels.percent}
          title={labels.percent}
          onClick={onPercent}
        >
          {pct}%
        </button>
      )}
    </span>
  );
}

export interface StatusToggle {
  id: string;
  label: string;
  icon?: ReactNode;
  pressed?: boolean;
  disabled?: boolean;
  onClick: () => void;
}

/**
 * A row of small toggle buttons (notes, comments). `exclusive`: exactly one of them is on
 * (the view modes) — a radio group, so the choices are announced as one of several and are
 * told apart from the ribbon buttons of the same name.
 */
export function StatusButtons({
  label,
  buttons,
  testId,
  exclusive = false,
}: {
  label: string;
  buttons: StatusToggle[];
  testId?: string;
  exclusive?: boolean;
}) {
  return (
    <span
      role={exclusive ? 'radiogroup' : 'group'}
      aria-label={label}
      className={styles.group}
      data-testid={testId}
    >
      {buttons.map((b) => (
        <button
          key={b.id}
          type="button"
          role={exclusive ? 'radio' : undefined}
          className={styles.button}
          data-testid={`status-button-${b.id}`}
          aria-label={b.label}
          title={b.label}
          aria-checked={exclusive ? !!b.pressed : undefined}
          aria-pressed={exclusive ? undefined : (b.pressed ?? undefined)}
          disabled={b.disabled}
          onClick={b.onClick}
        >
          {b.icon ?? b.label}
        </button>
      ))}
    </span>
  );
}
