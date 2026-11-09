import type { ReactNode } from 'react';

export interface StatusBarProps {
  left?: ReactNode;
  right?: ReactNode;
}

/** Bottom status bar: 24px (docs/ui/DESIGN.md "Khung cửa sổ"), document info left, view controls right. */
export function StatusBar({ left, right }: StatusBarProps) {
  return (
    <footer
      role="status"
      data-testid="status-bar"
      style={{
        display: 'flex',
        alignItems: 'center',
        justifyContent: 'space-between',
        gap: 16,
        height: 24,
        minHeight: 24,
        boxSizing: 'border-box',
        padding: '0 12px',
        fontSize: 12,
        color: 'var(--bo-text)',
        background: 'var(--bo-surface-2)',
        borderTop: '1px solid var(--bo-border)',
      }}
    >
      <div style={{ display: 'flex', gap: 16, alignItems: 'center' }}>{left}</div>
      <div style={{ display: 'flex', gap: 12, alignItems: 'center' }}>{right}</div>
    </footer>
  );
}
