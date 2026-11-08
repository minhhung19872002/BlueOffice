/**
 * Asks whether a local file found in a BlueDrive sync folder should switch to
 * online collaboration (prompt §6A.3).
 */
import type { Language } from '@blueoffice/i18n';
import { useEffect, useRef } from 'react';
import { startText } from './text';

export function SyncPrompt(props: {
  language: Language;
  name: string;
  onOnline: () => void;
  onLocal: () => void;
}) {
  const t = (k: Parameters<typeof startText>[1], v?: Record<string, string>) =>
    startText(props.language, k, v);
  const first = useRef<HTMLButtonElement>(null);
  useEffect(() => first.current?.focus(), []);
  const btn = {
    border: '1px solid #d9e1ec',
    borderRadius: 6,
    padding: '6px 14px',
    font: 'inherit',
    cursor: 'pointer',
  } as const;
  return (
    <div
      style={{
        position: 'fixed',
        inset: 0,
        zIndex: 1100,
        display: 'grid',
        placeItems: 'center',
        background: 'rgba(15,23,42,.35)',
      }}
    >
      <div
        role="dialog"
        aria-modal="true"
        aria-labelledby="bo-sync-title"
        data-testid="sync-prompt"
        onKeyDown={(e) => {
          if (e.key === 'Escape') props.onLocal();
        }}
        style={{
          maxWidth: 440,
          padding: 20,
          borderRadius: 10,
          background: '#fff',
          color: '#16233a',
          font: '14px/1.5 system-ui, "Segoe UI", sans-serif',
          boxShadow: '0 8px 30px rgba(0,0,0,.25)',
        }}
      >
        <h2 id="bo-sync-title" style={{ margin: '0 0 8px', fontSize: 16 }}>
          {t('syncPromptTitle')}
        </h2>
        <p style={{ margin: '0 0 16px' }}>{t('syncPromptBody', { name: props.name })}</p>
        <div style={{ display: 'flex', gap: 8, justifyContent: 'flex-end' }}>
          <button type="button" style={btn} data-testid="sync-prompt-local" onClick={props.onLocal}>
            {t('syncPromptLocal')}
          </button>
          <button
            ref={first}
            type="button"
            data-testid="sync-prompt-online"
            style={{ ...btn, background: '#0b5cad', borderColor: '#0b5cad', color: '#fff' }}
            onClick={props.onOnline}
          >
            {t('syncPromptOnline')}
          </button>
        </div>
      </div>
    </div>
  );
}
