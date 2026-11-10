import { useEffect, useLayoutEffect, useRef, useState } from 'react';
import { hasIcon, Icon } from './icons';
import type { RibbonItem } from './Ribbon';
import styles from './MiniToolbar.module.css';

export interface MiniToolbarProps {
  /** Viewport point of the selection end (mouse-up); the bar shows above it. */
  x: number;
  y: number;
  /** Ribbon items to show (buttons and custom controls such as the font box). */
  items: RibbonItem[];
  label: string;
  onClose: () => void;
}

/** Pointer distance (px) from the bar where it starts fading, and where it closes. */
export const MINI_FADE_FROM = 24;
export const MINI_CLOSE_AT = 160;

/** Opacity of the bar for a pointer `d` px away from it (1 near, fading out, 0 = closed). */
export function miniOpacity(d: number): number {
  if (d <= MINI_FADE_FROM) return 1;
  if (d >= MINI_CLOSE_AT) return 0;
  return 1 - ((d - MINI_FADE_FROM) / (MINI_CLOSE_AT - MINI_FADE_FROM)) * 0.85;
}

/** Distance from a point to a rectangle (0 inside). */
export function rectDistance(
  r: { left: number; top: number; right: number; bottom: number },
  x: number,
  y: number,
): number {
  const dx = Math.max(r.left - x, 0, x - r.right);
  const dy = Math.max(r.top - y, 0, y - r.bottom);
  return Math.hypot(dx, dy);
}

/**
 * Floating formatting bar (CORE-UI-003) shown near a text selection on mouse-up: it fades as
 * the pointer moves away and closes when it is far, on typing, on a click elsewhere or Escape.
 * Popovers opened from it (font list, colours) keep it open.
 */
export function MiniToolbar({ x, y, items, label, onClose }: MiniToolbarProps) {
  const ref = useRef<HTMLDivElement>(null);
  const [pos, setPos] = useState({ left: x, top: y });
  const [opacity, setOpacity] = useState(1);
  const closeRef = useRef(onClose);
  useEffect(() => {
    closeRef.current = onClose;
  }, [onClose]);

  useLayoutEffect(() => {
    const el = ref.current;
    if (!el) return;
    const w = el.offsetWidth;
    const h = el.offsetHeight;
    const vw = window.innerWidth || 1024;
    const top = y - h - 14;
    setPos({
      left: Math.max(4, Math.min(x - w / 2, vw - w - 4)),
      top: top < 4 ? y + 20 : top,
    });
  }, [x, y]);

  useEffect(() => {
    const inPopover = (t: EventTarget | null) =>
      t instanceof Element && (!!ref.current?.contains(t) || !!t.closest('[data-bo-popover]'));
    const onMove = (e: MouseEvent) => {
      const el = ref.current;
      if (!el || inPopover(e.target)) {
        setOpacity(1);
        return;
      }
      const d = rectDistance(el.getBoundingClientRect(), e.clientX, e.clientY);
      const o = miniOpacity(d);
      if (o === 0) closeRef.current();
      else setOpacity(o);
    };
    const onDown = (e: MouseEvent) => {
      if (!inPopover(e.target)) closeRef.current();
    };
    const onKey = (e: KeyboardEvent) => {
      if (inPopover(e.target)) {
        if (e.key === 'Escape') closeRef.current();
        return;
      }
      if (['Shift', 'Control', 'Alt', 'Meta'].includes(e.key)) return;
      closeRef.current();
    };
    window.addEventListener('mousemove', onMove);
    window.addEventListener('mousedown', onDown, true);
    window.addEventListener('keydown', onKey, true);
    const onWheel = () => closeRef.current();
    window.addEventListener('wheel', onWheel, { passive: true });
    return () => {
      window.removeEventListener('mousemove', onMove);
      window.removeEventListener('mousedown', onDown, true);
      window.removeEventListener('keydown', onKey, true);
      window.removeEventListener('wheel', onWheel);
    };
  }, []);

  return (
    <div
      ref={ref}
      role="toolbar"
      aria-label={label}
      className={styles.bar}
      data-testid="mini-toolbar"
      style={{ left: pos.left, top: pos.top, opacity }}
      onMouseDown={(e) => {
        // Keep the caret / IME focus in the document (inputs inside still focus).
        if (!(e.target instanceof HTMLInputElement)) e.preventDefault();
      }}
    >
      {items.map((i) =>
        i.render ? (
          <div key={i.id} className={styles.control} data-mini-control={i.id}>
            {i.render}
          </div>
        ) : (
          <button
            key={i.id}
            type="button"
            className={styles.button}
            data-mini-command={i.id}
            aria-label={i.label}
            title={i.shortcut ? `${i.label} (${i.shortcut})` : i.label}
            aria-pressed={i.checked ?? undefined}
            aria-disabled={i.enabled === false || undefined}
            onClick={() => {
              if (i.enabled !== false) i.onSelect();
            }}
          >
            {hasIcon(i.id) ? <Icon name={i.id} size={16} /> : (i.icon ?? i.label)}
          </button>
        ),
      )}
    </div>
  );
}
