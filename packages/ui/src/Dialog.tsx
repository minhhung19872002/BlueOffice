import * as RD from '@radix-ui/react-dialog';
import type { ReactNode } from 'react';
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
