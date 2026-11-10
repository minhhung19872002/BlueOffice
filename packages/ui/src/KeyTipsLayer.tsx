import { useEffect, useLayoutEffect, useRef, useState, type RefObject } from 'react';
import { createPortal } from 'react-dom';
import { assignKeyTips, keyTipHit, keyTipMatches, KEYTIP_OVERRIDES } from './keyTips';
import styles from './KeyTipsLayer.module.css';

/** Where KeyTips are: the tab strip, or the commands of one tab. */
type Level = { level: 'tabs' } | { level: 'items'; tab: string };

interface Target {
  id: string;
  code: string;
  el: HTMLElement;
  left: number;
  top: number;
}

export interface KeyTipsLayerProps {
  /** The ribbon root (`Tabs.Root`). */
  root: RefObject<HTMLElement | null>;
  /** Tabs in strip order (regular, then contextual). */
  tabs: readonly { id: string; label: string }[];
  /** The File button is shown (code `file`). */
  file?: { label: string; onOpen: () => void };
  /** Activate a tab (the ribbon shows its commands, also when collapsed). */
  onTab: (id: string) => void;
  overrides?: Readonly<Record<string, string>>;
  /** Fires with true when KeyTips show, false when they go away. */
  onActive?: (active: boolean) => void;
  /**
   * Called just before KeyTips show (Alt / F10), in the same update: an auto-hidden ribbon
   * shows its tab strip so the letters can be placed on it.
   */
  onShow?: () => void;
}

/** Ribbon key presses handled once even when several ribbons are mounted. */
const seen = new WeakSet<Event>();

function codeOf(e: KeyboardEvent): string | null {
  const m = /^(?:Key|Digit|Numpad)([A-Z0-9])$/.exec(e.code ?? '');
  if (m) return m[1]!;
  return e.key.length === 1 && /[a-z0-9]/i.test(e.key) ? e.key.toUpperCase() : null;
}

function labelOf(el: Element, fallback: string): string {
  const own = el.getAttribute('aria-label') ?? el.getAttribute('title');
  if (own) return own;
  const inner = el.querySelector('[aria-label],[title]');
  return inner?.getAttribute('aria-label') ?? inner?.getAttribute('title') ?? fallback;
}

/** Commands of the active tab panel reachable from the keyboard, in ribbon order. */
function itemTargets(panel: HTMLElement): { id: string; label: string; el: HTMLElement }[] {
  const out: { id: string; label: string; el: HTMLElement }[] = [];
  const groups = panel.querySelectorAll<HTMLElement>(':scope > [data-group]');
  groups.forEach((g) => {
    const gid = g.dataset.group ?? '';
    if (g.dataset.level === 'collapsed') {
      const trigger = g.querySelector<HTMLElement>('button');
      if (trigger) out.push({ id: `group:${gid}`, label: labelOf(trigger, gid), el: trigger });
      return;
    }
    g.querySelectorAll<HTMLElement>('button[data-command], [data-control]').forEach((el) => {
      if (el.closest('[data-control]') !== el && el.closest('[data-control]')) return;
      const id = el.dataset.command ?? el.dataset.control ?? '';
      if (!id || out.some((o) => o.id === id)) return;
      const label = el.dataset.control
        ? labelOf(el, id)
        : el.getAttribute('aria-label') || el.textContent?.trim() || id;
      out.push({ id, label, el });
    });
    const more = g.querySelector<HTMLElement>(`[data-testid="ribbon-group-more-${gid}"]`);
    if (more) out.push({ id: `more:${gid}`, label: labelOf(more, gid), el: more });
  });
  const overflow = panel.querySelector<HTMLElement>('[data-testid="ribbon-more"]');
  if (overflow) out.push({ id: 'more', label: labelOf(overflow, 'more'), el: overflow });
  return out;
}

/** Run a command element: click a button, focus an input / combobox. */
function activate(el: HTMLElement) {
  if (el.matches('button')) {
    el.click();
    return;
  }
  const field = el.querySelector<HTMLElement>('input, select, textarea');
  if (field) {
    field.focus();
    if (field instanceof HTMLInputElement) field.select();
    return;
  }
  el.querySelector<HTMLElement>('button, [tabindex]')?.click();
}

/**
 * Alt KeyTips over the ribbon (CORE-UI-004): press and release Alt (or F10) for letters on
 * the tabs, type one to open the tab and get letters on its commands, type a command's
 * letters to run it. Escape steps back (typed letters → tab strip → off); Alt, a click or
 * leaving the window turns them off.
 */
export function KeyTipsLayer({
  root,
  tabs,
  file,
  onTab,
  overrides,
  onActive,
  onShow,
}: KeyTipsLayerProps) {
  const [level, setLevel] = useState<Level | null>(null);
  const [typed, setTyped] = useState('');
  const [targets, setTargets] = useState<Target[]>([]);
  const state = useRef({ level, typed, targets, tabs, file, onTab, overrides, onShow });
  useLayoutEffect(() => {
    state.current = { level, typed, targets, tabs, file, onTab, overrides, onShow };
  });
  const alt = useRef({ down: false, clean: false });
  const tabsKey = `${file?.label ?? ''}|${tabs.map((t) => `${t.id}:${t.label}`).join(',')}`;

  useEffect(() => onActive?.(level !== null), [level, onActive]);

  // Measure the targets of the level once it is rendered (a tab switch renders first).
  useLayoutEffect(() => {
    const r = root.current;
    if (!level || !r) return undefined;
    const { tabs, file, overrides } = state.current;
    const measure = () => {
      const o = overrides ?? KEYTIP_OVERRIDES;
      let list: { id: string; label: string; el: HTMLElement }[] = [];
      if (level.level === 'tabs') {
        const fileEl = r.querySelector<HTMLElement>('[data-testid="file-tab"]');
        if (file && fileEl) list.push({ id: 'file', label: file.label, el: fileEl });
        for (const t of tabs) {
          const el = r.querySelector<HTMLElement>(`[role="tab"][data-tab="${t.id}"]`);
          if (el) list.push({ id: `tab:${t.id}`, label: t.label, el });
        }
      } else {
        const panel = r.querySelector<HTMLElement>('[role="tabpanel"][data-state="active"]');
        list = panel ? itemTargets(panel) : [];
      }
      const codes = assignKeyTips(list, o);
      setTargets(
        list.flatMap((t) => {
          const code = codes.get(t.id);
          const b = t.el.getBoundingClientRect();
          if (!code || !t.el.isConnected || t.el.closest('[hidden]')) return [];
          return [
            {
              id: t.id,
              code,
              el: t.el,
              left: b.left + b.width / 2,
              top: level.level === 'tabs' ? b.bottom - 4 : b.bottom - 6,
            },
          ];
        }),
      );
    };
    measure();
    // The panel of a newly chosen tab may lay out in the next frame (collapsed groups).
    const raf = requestAnimationFrame(measure);
    return () => cancelAnimationFrame(raf);
  }, [level, root, tabsKey]);

  useEffect(() => {
    const exit = () => {
      setLevel(null);
      setTyped('');
    };
    const onDown = (e: KeyboardEvent) => {
      const s = state.current;
      if (e.key === 'Alt') {
        alt.current = { down: true, clean: !e.ctrlKey && !e.shiftKey && !e.metaKey };
        return;
      }
      if (alt.current.down) alt.current.clean = false;
      if (!s.level) {
        if (e.key === 'F10' && !e.shiftKey && !e.ctrlKey && !e.altKey && !e.metaKey) {
          if (seen.has(e) || modalOpen()) return;
          seen.add(e);
          e.preventDefault();
          s.onShow?.();
          setLevel({ level: 'tabs' });
        }
        return;
      }
      if (seen.has(e)) return;
      seen.add(e);
      if (e.key === 'Escape') {
        e.preventDefault();
        e.stopPropagation();
        if (s.typed) setTyped('');
        else if (s.level.level === 'items') setLevel({ level: 'tabs' });
        else exit();
        return;
      }
      if (e.key === 'F10' || e.key === 'Tab') {
        e.preventDefault();
        exit();
        return;
      }
      if (e.ctrlKey || e.metaKey) return;
      const c = codeOf(e);
      if (!c) return;
      e.preventDefault();
      e.stopPropagation();
      const next = s.typed + c;
      const codes = new Map(s.targets.map((t) => [t.id, t.code]));
      const hit = keyTipHit(codes, next);
      if (hit) {
        const t = s.targets.find((x) => x.id === hit);
        setTyped('');
        if (hit === 'file') {
          exit();
          s.file?.onOpen();
        } else if (hit.startsWith('tab:')) {
          const id = hit.slice(4);
          s.onTab(id);
          setLevel({ level: 'items', tab: id });
        } else if (t) {
          exit();
          activate(t.el);
        }
        return;
      }
      if (keyTipMatches(codes, next).length) setTyped(next);
    };
    const onUp = (e: KeyboardEvent) => {
      if (e.key !== 'Alt') return;
      const a = alt.current;
      alt.current = { down: false, clean: false };
      if (!a.down || !a.clean || seen.has(e)) return;
      seen.add(e);
      e.preventDefault();
      if (state.current.level) exit();
      else if (!modalOpen()) {
        state.current.onShow?.();
        setLevel({ level: 'tabs' });
      }
    };
    const onMouse = () => {
      alt.current.clean = false;
      if (state.current.level) exit();
    };
    window.addEventListener('keydown', onDown, true);
    window.addEventListener('keyup', onUp, true);
    window.addEventListener('mousedown', onMouse, true);
    window.addEventListener('blur', exit);
    window.addEventListener('resize', exit);
    return () => {
      window.removeEventListener('keydown', onDown, true);
      window.removeEventListener('keyup', onUp, true);
      window.removeEventListener('mousedown', onMouse, true);
      window.removeEventListener('blur', exit);
      window.removeEventListener('resize', exit);
    };
  }, []);

  if (!level || !targets.length) return null;
  // Targets measured for an earlier level are not shown once it ended.
  const host = (root.current?.closest('.bo-root') as HTMLElement | null) ?? document.body;
  return createPortal(
    <div className={styles.layer} data-testid="keytips" data-level={level.level} aria-hidden="true">
      {targets.map((t) => {
        const dim = typed && !t.code.startsWith(typed);
        return (
          <span
            key={t.id}
            className={`${styles.tip} ${dim ? styles.dim : ''}`}
            data-keytip={t.code}
            data-keytip-for={t.id}
            style={{ left: t.left, top: t.top }}
          >
            {t.code}
          </span>
        );
      })}
    </div>,
    host,
  );
}

function modalOpen(): boolean {
  return !!document.querySelector('[role="dialog"][aria-modal="true"], [role="alertdialog"]');
}
