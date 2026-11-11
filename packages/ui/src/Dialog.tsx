import * as RD from '@radix-ui/react-dialog';
import { useLayoutEffect, useRef, type ReactNode } from 'react';
import styles from './Dialog.module.css';

export interface DialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  title: string;
  /** Buttons at the bottom (OK / Cancel …). */
  footer?: ReactNode;
  children?: ReactNode;
  /** Width in px (default 420). */
  width?: number;
  /** `data-testid` of the dialog content. */
  testId?: string;
}

/** Modal dialog (Radix Dialog): focus trap, Escape to close, labelled title. */
export function Dialog({
  open,
  onOpenChange,
  title,
  footer,
  children,
  width,
  testId,
}: DialogProps) {
  // WCAG 2.4.3: when the dialog goes away, the keyboard goes back to what had focus when it
  // opened. Radix does this when the element is still focusable at that moment; editors
  // that unmount the dialog while re-rendering left the focus on <body>.
  const opener = useRef<HTMLElement | null>(null);
  useLayoutEffect(() => {
    if (!open) return undefined;
    const a = document.activeElement;
    opener.current = a instanceof HTMLElement && a !== document.body ? a : null;
    return () => {
      const el = opener.current;
      requestAnimationFrame(() => {
        const now = document.activeElement;
        if (el?.isConnected && (!now || now === document.body)) el.focus();
      });
    };
  }, [open]);
  return (
    <RD.Root open={open} onOpenChange={onOpenChange}>
      <RD.Portal>
        <RD.Overlay className={styles.overlay} />
        <RD.Content
          className={styles.content}
          style={{ width: width ?? 420 }}
          data-testid={testId}
          aria-describedby={undefined}
        >
          <RD.Title className={styles.title}>{title}</RD.Title>
          <div className={styles.body}>{children}</div>
          {footer && <div className={styles.footer}>{footer}</div>}
        </RD.Content>
      </RD.Portal>
    </RD.Root>
  );
}
