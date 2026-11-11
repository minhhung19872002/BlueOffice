import { useTranslation } from '@blueoffice/i18n';
import { ClipboardPaste } from 'lucide-react';
import {
  useCallback,
  useEffect,
  useRef,
  useState,
  type KeyboardEvent as ReactKeyboardEvent,
} from 'react';
import styles from './Clipboard.module.css';

export interface PasteOptionItem {
  id: string;
  label: string;
}

export interface PasteOptionsProps {
  /** Screen point (CSS px) the button sits at: just after the pasted content. */
  anchor: () => { x: number; y: number } | null;
  options: readonly PasteOptionItem[];
  /** Id of the mode the paste is in now. */
  current: string;
  onChoose: (id: string) => void;
  /** Extra menu entry, e.g. the default paste settings. */
  settings?: { label: string; onSelect: () => void };
  /** Another use of the button (the sheet's Auto Fill Options): its name and test id. */
  label?: string;
  testId?: string;
}

/** Keyboard shortcut opening the menu while the button shows (shown in its tooltip). */
export const PASTE_OPTIONS_SHORTCUT = 'Alt+Shift+V';

const isShortcut = (e: KeyboardEvent) =>
  e.altKey && e.shiftKey && !e.ctrlKey && !e.metaKey && e.code === 'KeyV';

/**
 * Small button shown after a paste (CORE-PASTE-001): its menu redoes the last paste in
 * another mode. Alt+Shift+V opens the menu; in the menu the digit before each mode
 * chooses it, arrows move, Escape closes.
 */
export function PasteOptionsButton({
  anchor,
  options,
  current,
  onChoose,
  settings,
  label,
  testId = 'paste-options',
}: PasteOptionsProps) {
  const { t } = useTranslation();
  const name = label ?? t('pasteOptions.button');
  const [open, setOpen] = useState(false);
  // Placed when shown (hosts remount the button for each paste), then kept in place
  // while the page scrolls or resizes.
  const [pos, setPos] = useState<{ x: number; y: number } | null>(() => anchor());
  const menu = useRef<HTMLDivElement>(null);
  const button = useRef<HTMLButtonElement>(null);
  const frame = useRef(0);

  const locate = useCallback(() => {
    cancelAnimationFrame(frame.current);
    frame.current = requestAnimationFrame(() => setPos(anchor()));
  }, [anchor]);

  useEffect(() => {
    window.addEventListener('scroll', locate, true);
    window.addEventListener('resize', locate);
    return () => {
      cancelAnimationFrame(frame.current);
      window.removeEventListener('scroll', locate, true);
      window.removeEventListener('resize', locate);
    };
  }, [locate]);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (!isShortcut(e)) return;
      e.preventDefault();
      e.stopPropagation();
      setOpen(true);
    };
    window.addEventListener('keydown', onKey, true);
    return () => window.removeEventListener('keydown', onKey, true);
  }, []);

  useEffect(() => {
    if (!open) return;
    const items = menu.current?.querySelectorAll<HTMLElement>('[role^="menuitem"]');
    const checked = menu.current?.querySelector<HTMLElement>('[aria-checked="true"]');
    (checked ?? items?.[0])?.focus();
    const away = (e: MouseEvent) => {
      const target = e.target as Node | null;
      if (target && (menu.current?.contains(target) || button.current?.contains(target))) return;
      setOpen(false);
    };
    window.addEventListener('mousedown', away, true);
    return () => window.removeEventListener('mousedown', away, true);
  }, [open]);

  if (!pos || !options.length) return null;

  const choose = (id: string) => {
    setOpen(false);
    onChoose(id);
  };

  const onMenuKey = (e: ReactKeyboardEvent) => {
    e.stopPropagation();
    const items = Array.from(
      menu.current?.querySelectorAll<HTMLElement>('[role^="menuitem"]') ?? [],
    );
    const at = items.indexOf(document.activeElement as HTMLElement);
    if (e.key === 'Escape') {
      e.preventDefault();
      setOpen(false);
      button.current?.focus();
    } else if (e.key === 'ArrowDown' || e.key === 'ArrowUp') {
      e.preventDefault();
      const n = items.length;
      items[(at + (e.key === 'ArrowDown' ? 1 : n - 1)) % n]?.focus();
    } else if (/^[1-9]$/.test(e.key)) {
      const o = options[Number(e.key) - 1];
      if (o) {
        e.preventDefault();
        choose(o.id);
      }
    }
  };

  return (
    <div className={styles.pasteOptions} style={{ left: pos.x, top: pos.y }}>
      <button
        ref={button}
        type="button"
        className={styles.pasteButton}
        data-testid={testId}
        aria-label={name}
        aria-haspopup="menu"
        aria-expanded={open}
        aria-keyshortcuts={PASTE_OPTIONS_SHORTCUT}
        title={`${name} (${PASTE_OPTIONS_SHORTCUT})`}
        onMouseDown={(e) => e.preventDefault()}
        onClick={() => setOpen((o) => !o)}
      >
        <ClipboardPaste size={14} aria-hidden="true" />
        <span aria-hidden="true">▾</span>
      </button>
      {open && (
        <div
          ref={menu}
          className={styles.pasteMenu}
          role="menu"
          aria-label={label ?? t('pasteOptions.menu')}
          data-testid={`${testId}-menu`}
          onKeyDown={onMenuKey}
        >
          {options.map((o, i) => (
            <button
              key={o.id}
              type="button"
              role="menuitemradio"
              aria-checked={o.id === current}
              className={styles.pasteItem}
              data-paste-option={o.id}
              aria-keyshortcuts={String(i + 1)}
              onClick={() => choose(o.id)}
            >
              <span className={styles.key} aria-hidden="true">
                {i + 1}
              </span>
              {o.label}
            </button>
          ))}
          {settings && (
            <button
              type="button"
              role="menuitem"
              className={`${styles.pasteItem} ${styles.separated}`}
              data-paste-option="settings"
              onClick={() => {
                setOpen(false);
                settings.onSelect();
              }}
            >
              <span className={styles.key} aria-hidden="true" />
              {settings.label}
            </button>
          )}
        </div>
      )}
    </div>
  );
}
