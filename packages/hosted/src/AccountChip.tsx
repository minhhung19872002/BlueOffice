/**
 * Sign-in chip of the web and desktop shells (shown only when the server has
 * an OIDC client configured): "Đăng nhập" starts the provider sign-in (web:
 * redirect; desktop: system browser); once signed in it shows the user's name
 * and signs out on click.
 */
import { resources, type Language } from '@blueoffice/i18n';
import type { UserInfo } from '@blueoffice/platform';
import { useEffect, useState, type CSSProperties } from 'react';

type AuthKey = keyof (typeof resources)['vi-VN']['translation']['auth'];

export function authText(lang: Language, key: AuthKey, vars: Record<string, string> = {}) {
  const s: string = resources[lang].translation.auth[key];
  return s.replace(/\{\{(\w+)\}\}/g, (_, k: string) => vars[k] ?? '');
}

/** What the chip needs from a session (`OidcSession`, `DesktopAuth`). */
export interface AccountSession {
  readonly user: UserInfo | null;
  onChange(fn: (u: UserInfo | null) => void): () => void;
  signIn(returnTo?: string): Promise<void>;
  signOut(): Promise<void>;
}

const chip: CSSProperties = {
  position: 'fixed',
  bottom: 30,
  right: 12,
  zIndex: 900,
  display: 'inline-flex',
  alignItems: 'center',
  gap: 6,
  height: 26,
  padding: '0 10px',
  border: '1px solid var(--bo-border, #d7dde7)',
  borderRadius: 13,
  background: 'var(--bo-surface, #fff)',
  color: 'var(--bo-text, #1b2430)',
  font: '500 12px/20px system-ui, sans-serif',
  boxShadow: '0 1px 3px rgba(0,0,0,.15)',
  cursor: 'pointer',
};

export function AccountChip({
  auth,
  language,
  returnTo,
  onError,
}: {
  auth: AccountSession;
  language: Language;
  /** Page to come back to after a redirect sign-in (web). */
  returnTo?: string;
  onError?: (e: unknown) => void;
}) {
  const [user, setUser] = useState<UserInfo | null>(auth.user);
  useEffect(() => auth.onChange(setUser), [auth]);
  if (!user) {
    return (
      <button
        type="button"
        data-testid="account-sign-in"
        style={chip}
        onClick={() => void auth.signIn(returnTo).catch((e: unknown) => onError?.(e))}
      >
        {authText(language, 'signIn')}
      </button>
    );
  }
  return (
    <button
      type="button"
      data-testid="account"
      data-user={user.id}
      style={chip}
      title={`${authText(language, 'account', { name: user.name })} — ${authText(language, 'signOut')}`}
      onClick={() => void auth.signOut()}
    >
      {user.name}
    </button>
  );
}
