import { useEffect, useLayoutEffect, useRef, useState, type ReactNode } from 'react';
import { createPortal } from 'react-dom';
import styles from './Popover.module.css';

export interface PopoverProps {
  /** Content of the trigger button. */
  trigger: ReactNode;
  /** Accessible name of the trigger. */
  label: string;
  children: ReactNode | ((close: () => void) => ReactNode);
  disabled?: boolean;
  /** `data-testid` of the trigger (the panel gets `<testId>-panel`). */
  testId?: string;
  /** Pressed look of the trigger. */
  active?: boolean;
  className?: string;
}

/**
 * Dropdown panel anchored to a button (menus, colour palettes). Rendered in the
 * nearest `.bo-root` with fixed positioning so scrolling ribbons do not clip it;
 * closes on outside click and Escape. Keyboard focus stays in the document
 * unless the panel contains inputs.
 */
export function Popover({
  trigger,
  label,
  children,
  disabled,
  testId,
  active,
  className,
}: PopoverProps) {
  const [open, setOpen] = useState(false);
  const [host, setHost] = useState<HTMLElement | null>(null);
  const [pos, setPos] = useState<{ left: number; top: number }>({ left: 0, top: 0 });
  const button = useRef<HTMLButtonElement>(null);
  const panel = useRef<HTMLDivElement>(null);
  const close = () => setOpen(false);

  useLayoutEffect(() => {
    if (!open || !button.current) return;
    const r = button.current.getBoundingClientRect();
    const width = panel.current?.offsetWidth ?? 0;
    setPos({
      left: Math.max(4, Math.min(r.left, window.innerWidth - width - 4)),
      top: r.bottom + 2,
    });
  }, [open]);

  useEffect(() => {
    if (!open) return;
    const onDown = (e: MouseEvent) => {
      const t = e.target as Node;
      if (!panel.current?.contains(t) && !button.current?.contains(t)) setOpen(false);
    };
    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape') {
        e.stopPropagation();
        setOpen(false);
      }
    };
    window.addEventListener('mousedown', onDown, true);
    window.addEventListener('keydown', onKey, true);
    return () => {
      window.removeEventListener('mousedown', onDown, true);
      window.removeEventListener('keydown', onKey, true);
    };
  }, [open]);

  return (
    <>
      <button
        ref={button}
        type="button"
        className={`${styles.trigger} ${className ?? ''}`}
        aria-label={label}
        title={label}
        aria-haspopup="true"
        aria-expanded={open}
        aria-disabled={disabled}
        aria-pressed={active ?? undefined}
        data-testid={testId}
        onMouseDown={(e) => e.preventDefault()}
        onClick={(e) => {
          if (disabled) return;
          setHost((e.currentTarget.closest('.bo-root') as HTMLElement | null) ?? document.body);
          setOpen(!open);
        }}
      >
        {trigger}
      </button>
      {open &&
        host &&
        createPortal(
          <div
            ref={panel}
            className={styles.panel}
            role="dialog"
            aria-label={label}
            data-testid={testId ? `${testId}-panel` : undefined}
            style={{ left: pos.left, top: pos.top }}
          >
            {typeof children === 'function' ? children(close) : children}
          </div>,
          host,
        )}
    </>
  );
}
