import type { ReactNode } from 'react';

export interface StatusBarProps {
  left?: ReactNode;
  right?: ReactNode;
}

/** Bottom status bar. */
export function StatusBar({ left, right }: StatusBarProps) {
  return (
    <footer
      role="status"
      style={{
        display: 'flex',
        alignItems: 'center',
        justifyContent: 'space-between',
        gap: 16,
        height: 26,
        padding: '0 12px',
        fontSize: 12,
        color: 'var(--bo-brand-contrast)',
        background: 'var(--bo-brand)',
      }}
    >
      <div style={{ display: 'flex', gap: 16, alignItems: 'center' }}>{left}</div>
      <div style={{ display: 'flex', gap: 12, alignItems: 'center' }}>{right}</div>
    </footer>
  );
}
