/**
 * Thin notice above the editor of a hosted file: view mode (opened read-only
 * or edit permission revoked mid-session), revoked access, offline /
 * reconnecting. Nothing is shown while editing online.
 */
import type { CloseReason, CollabStatus } from '@blueoffice/collab';
import { resources, type Language } from '@blueoffice/i18n';
import { useEffect, useState, type CSSProperties } from 'react';
import type { SessionMode, SessionModeValue } from './mode';

type HostedKey = keyof (typeof resources)['vi-VN']['translation']['hosted'];

/** `hosted.*` string (no React context needed: the banner sits outside the editors). */
export function hostedText(lang: Language, key: HostedKey, vars: Record<string, string> = {}) {
  const s: string = resources[lang].translation.hosted[key];
  return s.replace(/\{\{(\w+)\}\}/g, (_, k: string) => vars[k] ?? '');
}

/** Observable connection state for {@link HostedBanner}. */
export class HostedStatus {
  private status: CollabStatus | null = null;
  private ended: CloseReason | null = null;
  private readonly listeners = new Set<() => void>();
  get value(): { status: CollabStatus | null; ended: CloseReason | null } {
    return { status: this.status, ended: this.ended };
  }
  setStatus(s: CollabStatus): void {
    this.status = s;
    this.listeners.forEach((l) => l());
  }
  setEnded(r: CloseReason): void {
    this.ended = r;
    this.listeners.forEach((l) => l());
  }
  subscribe(fn: () => void): () => void {
    this.listeners.add(fn);
    return () => this.listeners.delete(fn);
  }
}

const style: CSSProperties = {
  position: 'fixed',
  top: 8,
  left: '50%',
  transform: 'translateX(-50%)',
  zIndex: 1000,
  maxWidth: 'calc(100vw - 32px)',
  padding: '4px 12px',
  borderRadius: 6,
  font: '500 13px/20px system-ui, sans-serif',
  boxShadow: '0 1px 4px rgba(0,0,0,.2)',
  pointerEvents: 'none',
};

export function HostedBanner({
  mode,
  status,
  language,
}: {
  mode: SessionMode;
  status?: HostedStatus;
  language: Language;
}) {
  const [m, setM] = useState<SessionModeValue>(mode.value);
  const [initial] = useState<SessionModeValue>(mode.value);
  const [conn, setConn] = useState(status?.value ?? { status: null, ended: null });
  useEffect(() => mode.subscribe(setM), [mode]);
  useEffect(() => status?.subscribe(() => setConn(status.value)), [status]);

  let key: HostedKey | null = null;
  if (m === 'revoked' || conn.ended === 'revoked') key = 'revoked';
  else if (conn.ended === 'expired') key = 'expired';
  else if (conn.ended === 'reload') key = 'reload';
  else if (m === 'view' || m === 'comment')
    key = initial === 'edit' ? 'viewRevoked' : m === 'comment' ? 'comment' : 'view';
  else if (conn.status === 'offline') key = 'offline';
  if (!key) return null;
  const warn = key === 'revoked' || key === 'expired' || key === 'offline';
  return (
    <div
      data-testid="hosted-banner"
      data-mode={m}
      data-state={key}
      aria-live="polite"
      style={{
        ...style,
        background: warn ? '#fef3c7' : '#e0f2fe',
        color: warn ? '#78350f' : '#0c4a6e',
      }}
    >
      {hostedText(language, key)}
    </div>
  );
}
