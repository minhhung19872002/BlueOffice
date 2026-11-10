import { useEffect, useLayoutEffect, useRef, useState, type ReactNode } from 'react';
import styles from './ContextMenu.module.css';

/** A command of a context menu (resolved from the command registry / ribbon). */
export interface MenuEntry {
  /** Command id (`edit.copy`) or a local action id (`spell.pick`). */
  id: string;
  label: string;
  shortcut?: string;
  enabled?: boolean;
  /** Checkable command: shown with a check mark when true. */
  checked?: boolean;
  /** Spelling suggestion and similar: shown in bold at the top. */
  emphasis?: boolean;
  icon?: ReactNode;
  onSelect: () => void;
}

/** A line of a context menu: a command, a separator or a non-interactive heading. */
export type MenuLine = MenuEntry | { separator: true } | { heading: string; testId?: string };

export const isEntry = (l: MenuLine): l is MenuEntry => 'onSelect' in l;

export interface ContextMenuProps {
  /** Viewport point the menu opens at (kept inside the window). */
  x: number;
  y: number;
  items: MenuLine[];
  /** Accessible name of the menu. */
  label: string;
  onClose: () => void;
  testId?: string;
}

/**
 * Context menu (CORE-UI-003): `role="menu"`, arrows / Home / End move between enabled
 * commands, Enter or Space runs one, Escape / Tab / a click elsewhere closes. Disabled
 * commands are shown greyed (`aria-disabled`) and skipped by the arrows.
 */
export function ContextMenu({ x, y, items, label, onClose, testId }: ContextMenuProps) {
  const ref = useRef<HTMLDivElement>(null);
  const [pos, setPos] = useState({ left: x, top: y });
  const entries = items.filter(isEntry);
  const enabled = entries.map((e, i) => (e.enabled === false ? -1 : i)).filter((i) => i >= 0);
  // The item chosen with the arrows / pointer; until then (or once it is disabled) the first
  // enabled one, as the commands' state may still change after the menu opens.
  const [chosen, setActive] = useState<number | null>(null);
  const active = chosen !== null && enabled.includes(chosen) ? chosen : (enabled[0] ?? -1);
  const closeRef = useRef(onClose);
  useEffect(() => {
    closeRef.current = onClose;
  }, [onClose]);

  // Keep the menu inside the window (flip left / up near the edges).
  useLayoutEffect(() => {
    const el = ref.current;
    if (!el) return;
    const w = el.offsetWidth;
    const h = el.offsetHeight;
    const vw = window.innerWidth || 1024;
    const vh = window.innerHeight || 768;
    setPos({
      left: Math.max(4, x + w > vw - 4 ? Math.max(4, x - w) : x),
      top: Math.max(4, y + h > vh - 4 ? Math.max(4, vh - h - 4) : y),
    });
  }, [x, y, items.length]);

  useEffect(() => {
    ref.current?.querySelector<HTMLElement>(`[data-index="${active}"]`)?.focus();
  }, [active]);

  useEffect(() => {
    const onDown = (e: MouseEvent) => {
      if (!ref.current?.contains(e.target as Node)) closeRef.current();
    };
    const onBlur = () => closeRef.current();
    window.addEventListener('mousedown', onDown, true);
    window.addEventListener('blur', onBlur);
    window.addEventListener('resize', onBlur);
    return () => {
      window.removeEventListener('mousedown', onDown, true);
      window.removeEventListener('blur', onBlur);
      window.removeEventListener('resize', onBlur);
    };
  }, []);

  const run = (i: number) => {
    const e = entries[i];
    if (!e || e.enabled === false) return;
    closeRef.current();
    e.onSelect();
  };
  const step = (dir: 1 | -1) => {
    if (!enabled.length) return;
    const at = enabled.indexOf(active);
    const next = at < 0 ? 0 : (at + dir + enabled.length) % enabled.length;
    setActive(enabled[next] ?? -1);
  };
  // Keys work wherever the focus is (an editor may keep it in its text input).
  const onKey = (e: KeyboardEvent) => {
    let handled = true;
    if (e.key === 'ArrowDown') step(1);
    else if (e.key === 'ArrowUp') step(-1);
    else if (e.key === 'Home') setActive(enabled[0] ?? -1);
    else if (e.key === 'End') setActive(enabled[enabled.length - 1] ?? -1);
    else if (e.key === 'Enter' || e.key === ' ') run(active);
    else if (e.key === 'Escape' || e.key === 'Tab' || (e.key === 'F10' && e.shiftKey))
      closeRef.current();
    else handled = false;
    if (handled) {
      e.preventDefault();
      e.stopPropagation();
    }
  };
  const keyRef = useRef(onKey);
  useEffect(() => {
    keyRef.current = onKey;
  });
  useEffect(() => {
    const f = (e: KeyboardEvent) => keyRef.current(e);
    window.addEventListener('keydown', f, true);
    return () => window.removeEventListener('keydown', f, true);
  }, []);

  return (
    <div
      ref={ref}
      role="menu"
      aria-label={label}
      aria-orientation="vertical"
      className={styles.menu}
      data-testid={testId ?? 'context-menu'}
      data-bo-popover=""
      style={{ left: pos.left, top: pos.top }}
      onContextMenu={(e) => e.preventDefault()}
    >
      {items.map((line, key) => {
        if ('separator' in line) return <div key={key} role="separator" className={styles.sep} />;
        if ('heading' in line)
          return (
            <div key={key} className={styles.heading} role="presentation" data-testid={line.testId}>
              {line.heading}
            </div>
          );
        const i = entries.indexOf(line);
        const checkable = line.checked !== undefined;
        return (
          <button
            key={key}
            type="button"
            role={checkable ? 'menuitemcheckbox' : 'menuitem'}
            aria-checked={checkable ? line.checked : undefined}
            aria-disabled={line.enabled === false || undefined}
            aria-keyshortcuts={line.shortcut}
            tabIndex={i === active ? 0 : -1}
            data-index={i}
            data-active={i === active || undefined}
            data-menu-command={line.id}
            className={`${styles.item} ${line.emphasis ? styles.emphasis : ''}`}
            onMouseEnter={() => line.enabled !== false && setActive(i)}
            onMouseDown={(e) => e.preventDefault()}
            onClick={() => run(i)}
          >
            <span className={styles.check} aria-hidden="true">
              {line.checked ? '✓' : (line.icon ?? '')}
            </span>
            <span className={styles.label}>{line.label}</span>
            {line.shortcut && <kbd className={styles.kbd}>{line.shortcut}</kbd>}
          </button>
        );
      })}
    </div>
  );
}

/** Shift+F10 or the context-menu key: open the menu at the caret / selection. */
export function isContextMenuKey(
  e: Pick<KeyboardEvent, 'key' | 'shiftKey' | 'ctrlKey' | 'altKey' | 'metaKey'>,
): boolean {
  if (e.key === 'ContextMenu') return true;
  return e.key === 'F10' && e.shiftKey && !e.ctrlKey && !e.altKey && !e.metaKey;
}

/** Client point under an element (bottom-left of its box), for keyboard-opened menus. */
export function menuPointOf(el: Element | null | undefined): { x: number; y: number } | null {
  if (!el) return null;
  const r = el.getBoundingClientRect();
  if (r.width === 0 && r.height === 0 && r.left === 0 && r.top === 0) return null;
  return { x: Math.round(r.left), y: Math.round(r.bottom + 2) };
}
