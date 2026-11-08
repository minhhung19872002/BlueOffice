/**
 * Host chrome of the embedded editor (prompt §3.5, §6A.2): the host's brand,
 * the file name with its unsaved marker, and the Back / Share / Close buttons
 * the host asked for (`BlueOffice.open({ ui })`). The buttons do not act
 * themselves: they report to the host page through the SDK (`share-requested`,
 * `closed`), which answers with its own UI.
 */
import type { Language } from '@blueoffice/i18n';
import { useResolvedTheme, type ThemeSetting } from '@blueoffice/ui';
import { ArrowLeft, Share2, X } from 'lucide-react';
import { useEffect, useState, type CSSProperties } from 'react';
import type { DirtyState } from './dirty';
import { hostedText } from './HostedBanner';

export interface HostedChromeUi {
  brandName?: string;
  showShare?: boolean;
  showClose?: boolean;
  showBack?: boolean;
}

export interface HostedChromeProps {
  ui: HostedChromeUi;
  /** App display name ("BlueDoc", …). */
  appName: string;
  fileName: string;
  dirty?: DirtyState;
  language: Language;
  theme: ThemeSetting;
  onBack?: () => void;
  onShare?: () => void;
  onClose?: () => void;
}

const bar: CSSProperties = {
  display: 'flex',
  alignItems: 'center',
  gap: 8,
  height: 36,
  padding: '0 8px',
  flex: '0 0 auto',
  background: 'var(--bo-brand)',
  color: 'var(--bo-brand-contrast)',
  font: '500 13px/20px var(--bo-font)',
};

const button: CSSProperties = {
  display: 'inline-flex',
  alignItems: 'center',
  gap: 4,
  height: 28,
  padding: '0 10px',
  border: '1px solid rgba(255,255,255,.35)',
  borderRadius: 'var(--bo-radius)',
  background: 'transparent',
  color: 'inherit',
  font: 'inherit',
  cursor: 'pointer',
};

/** True when the host asked for any part of the chrome. */
export const wantsChrome = (ui: HostedChromeUi): boolean =>
  !!(ui.brandName || ui.showShare || ui.showClose || ui.showBack);

export function HostedChrome(p: HostedChromeProps) {
  const theme = useResolvedTheme(p.theme);
  const [dirty, setDirty] = useState(p.dirty?.dirty ?? false);
  useEffect(() => p.dirty?.subscribe(setDirty), [p.dirty]);
  const t = (k: 'back' | 'share' | 'close' | 'unsaved') => hostedText(p.language, k);
  return (
    <header className="bo-root" data-testid="hosted-chrome" data-theme={theme} style={bar}>
      {p.ui.showBack && (
        <button
          type="button"
          style={button}
          data-testid="hosted-back"
          aria-label={t('back')}
          title={t('back')}
          onClick={p.onBack}
        >
          <ArrowLeft size={16} aria-hidden />
        </button>
      )}
      <span data-testid="hosted-brand" style={{ fontWeight: 600, whiteSpace: 'nowrap' }}>
        {p.ui.brandName ? `${p.ui.brandName} · ${p.appName}` : p.appName}
      </span>
      <span
        style={{
          flex: 1,
          minWidth: 0,
          overflow: 'hidden',
          textOverflow: 'ellipsis',
          whiteSpace: 'nowrap',
          opacity: 0.9,
        }}
      >
        {p.fileName}
        {dirty && (
          <span data-testid="hosted-unsaved" title={t('unsaved')}>
            {' •'}
          </span>
        )}
      </span>
      {p.ui.showShare && (
        <button type="button" style={button} data-testid="hosted-share" onClick={p.onShare}>
          <Share2 size={14} aria-hidden /> {t('share')}
        </button>
      )}
      {p.ui.showClose && (
        <button
          type="button"
          style={button}
          data-testid="hosted-close"
          aria-label={t('close')}
          title={t('close')}
          onClick={p.onClose}
        >
          <X size={16} aria-hidden />
        </button>
      )}
    </header>
  );
}

/** Shown in the iframe once the editor has left the session (`closed`). */
export function HostedClosed({ language, theme }: { language: Language; theme: ThemeSetting }) {
  const resolved = useResolvedTheme(theme);
  return (
    <div
      className="bo-root"
      data-theme={resolved}
      data-testid="hosted-closed"
      style={{
        display: 'grid',
        placeItems: 'center',
        height: '100%',
        background: 'var(--bo-bg)',
        color: 'var(--bo-text-muted)',
      }}
    >
      {hostedText(language, 'closed')}
    </div>
  );
}
