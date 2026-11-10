/**
 * Presenter side of the phone remote and the online presentation (SLIDE-REMOTE-001): the
 * pairing dialog (QR code + link), the online presentation dialog, and the hook keeping the
 * server relay (`PresenterLink`, `@blueoffice/platform` `remoteShow`) in step with the show.
 */
import { renderPage, type GlyphStore, type ImageStore } from '@blueoffice/canvas-host';
import { useTranslation } from '@blueoffice/i18n';
import {
  encodeQr,
  qrPath,
  RemoteShowError,
  type EngineHost,
  type PresenterLink,
  type RemoteAction,
  type RemoteState,
  type ServerRemoteShow,
} from '@blueoffice/platform';
import { Dialog } from '@blueoffice/ui';
import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import styles from './remote.module.css';

/** A slide as a small JPEG data URL (`null` where the platform cannot draw: tests). */
export async function slideThumbnail(
  engine: EngineHost,
  glyphs: GlyphStore,
  images: ImageStore,
  index: number,
  width: number,
): Promise<string | null> {
  const page = await engine.getPage(index);
  await Promise.all([glyphs.ensure(page), images.ensure(page)]);
  if (typeof document === 'undefined' || page.width <= 0) return null;
  const canvas = document.createElement('canvas');
  const scale = width / page.width;
  canvas.width = Math.max(1, Math.round(page.width * scale));
  canvas.height = Math.max(1, Math.round(page.height * scale));
  const ctx = (() => {
    try {
      return canvas.getContext('2d');
    } catch {
      return null;
    }
  })();
  if (!ctx) return null;
  renderPage(ctx, page, glyphs, scale, images);
  try {
    return canvas.toDataURL('image/jpeg', 0.72);
  } catch {
    return null;
  }
}

/**
 * Base URL of the web app the phone and the viewers open: the page's own address without the
 * app route (`/slide`, `/edit`).
 */
export function appBase(href: string): string {
  const u = new URL(href);
  u.search = '';
  u.hash = '';
  u.pathname = u.pathname.replace(/\/(slide|slides|edit|doc|sheet)(\/.*)?$/, '/');
  if (!u.pathname.endsWith('/')) u.pathname += '/';
  return u.toString();
}

/** The address is only reachable from this computer (a phone cannot open it). */
export function isLocalOnly(url: string): boolean {
  const h = new URL(url).hostname;
  return h === 'localhost' || h === '127.0.0.1' || h === '[::1]' || h.endsWith('.localhost');
}

/** A QR code drawn as SVG (ISO/IEC 18004, encoder in `@blueoffice/platform` `qr`). */
export function QrCode({
  text,
  size = 200,
  label,
}: {
  text: string;
  size?: number;
  label: string;
}) {
  const code = useMemo(() => encodeQr(text), [text]);
  const quiet = 4;
  const n = code.size + quiet * 2;
  return (
    <svg
      className={styles.qr}
      width={size}
      height={size}
      viewBox={`0 0 ${n} ${n}`}
      role="img"
      aria-label={label}
      data-testid="remote-qr"
      data-version={code.version}
      shapeRendering="crispEdges"
    >
      <rect width={n} height={n} fill="#fff" />
      <path d={qrPath(code, quiet)} fill="#000" />
    </svg>
  );
}

export type RemoteErrorKey =
  | 'unavailable'
  | 'unauthorized'
  | 'tooMany'
  | 'forbidden'
  | 'tooLarge'
  | 'rateLimited'
  | 'network'
  | 'invalidToken';

function errorKey(e: unknown): RemoteErrorKey {
  return e instanceof RemoteShowError ? e.code : 'network';
}

export interface RemotePresentingState {
  /** The server offers it (`null`: still asking). */
  available: boolean | null;
  busy: boolean;
  error: RemoteErrorKey | null;
  /** Pairing link for the phone. */
  pairing: { url: string; expiresAt: number } | null;
  /** A phone is paired / connected now. */
  paired: boolean;
  phone: boolean;
  /** Link of the online presentation. */
  online: { url: string } | null;
  viewers: number;
  canBroadcast: boolean;
}

export interface UseRemotePresentingOptions {
  provider: ServerRemoteShow | undefined;
  /** The deck as it is now (online presentation). */
  deck: () => Promise<{ name: string; bytes: Uint8Array }>;
  onCommand: (action: RemoteAction) => void;
  /** Base URL of the web app (default: from the page's address). */
  appUrl?: string;
}

/** The presentation on the server, created on first use and ended with the editor. */
export function useRemotePresenting({
  provider,
  deck,
  onCommand,
  appUrl,
}: UseRemotePresentingOptions) {
  const [state, setState] = useState<RemotePresentingState>({
    available: provider ? null : false,
    busy: false,
    error: null,
    pairing: null,
    paired: false,
    phone: false,
    online: null,
    viewers: 0,
    canBroadcast: true,
  });
  const linkRef = useRef<PresenterLink | null>(null);
  const starting = useRef<Promise<PresenterLink> | null>(null);
  const commandRef = useRef(onCommand);
  useEffect(() => {
    commandRef.current = onCommand;
  }, [onCommand]);
  const base = appUrl ?? (typeof location !== 'undefined' ? appBase(location.href) : '/');

  useEffect(() => {
    if (!provider) return;
    let cancelled = false;
    void provider.available().then((a) => {
      if (!cancelled) setState((s) => ({ ...s, available: a }));
    });
    return () => {
      cancelled = true;
    };
  }, [provider]);

  // The presentation ends with the editor.
  useEffect(
    () => () => {
      void linkRef.current?.end();
      linkRef.current = null;
    },
    [],
  );

  const link = useCallback(async (): Promise<PresenterLink> => {
    if (linkRef.current && !linkRef.current.isEnded) return linkRef.current;
    if (!provider) throw new RemoteShowError('unavailable');
    if (!starting.current) {
      starting.current = provider.start().then((l) => {
        linkRef.current = l;
        setState((s) => ({ ...s, canBroadcast: l.canBroadcast }));
        l.onEvent((e) => {
          if (e.type === 'command') commandRef.current(e.action);
          else if (e.type === 'peers')
            setState((s) => ({ ...s, phone: e.remote, paired: e.paired, viewers: e.viewers }));
          else if (e.type === 'ended') {
            linkRef.current = null;
            setState((s) => ({
              ...s,
              pairing: null,
              online: null,
              phone: false,
              paired: false,
              viewers: 0,
            }));
          }
        });
        return l;
      });
      starting.current.finally(() => {
        starting.current = null;
      });
    }
    return starting.current;
  }, [provider]);

  const run = useCallback(async (f: () => Promise<void>) => {
    setState((s) => ({ ...s, busy: true, error: null }));
    try {
      await f();
      setState((s) => ({ ...s, busy: false }));
    } catch (e) {
      setState((s) => ({ ...s, busy: false, error: errorKey(e) }));
    }
  }, []);

  /** A fresh pairing code (QR); a phone paired before is revoked. */
  const pair = useCallback(
    () =>
      run(async () => {
        const l = await link();
        const p = await l.pair();
        setState((s) => ({
          ...s,
          pairing: { url: `${base}remote/${p.token}`, expiresAt: Date.parse(p.expiresAt) },
          paired: false,
          phone: false,
        }));
      }),
    [run, link, base],
  );

  const revoke = useCallback(
    () =>
      run(async () => {
        const l = linkRef.current;
        if (l) await l.revokeRemote();
        setState((s) => ({ ...s, pairing: null, paired: false, phone: false }));
      }),
    [run],
  );

  const startOnline = useCallback(
    () =>
      run(async () => {
        const l = await link();
        const d = await deck();
        const token = await l.broadcast(d.bytes, d.name);
        setState((s) => ({ ...s, online: { url: `${base}watch/${token}` } }));
      }),
    [run, link, deck, base],
  );

  const endOnline = useCallback(
    () =>
      run(async () => {
        const l = linkRef.current;
        if (l) await l.endBroadcast();
        setState((s) => ({ ...s, online: null, viewers: 0 }));
      }),
    [run],
  );

  const send = useCallback((msg: RemoteState | { type: 'show'; [k: string]: unknown }) => {
    linkRef.current?.send(msg);
  }, []);

  return { state, pair, revoke, startOnline, endOnline, send };
}

function Countdown({ until }: { until: number }) {
  const { t } = useTranslation();
  const [now, setNow] = useState(() => Date.now());
  useEffect(() => {
    const id = setInterval(() => setNow(Date.now()), 1000);
    return () => clearInterval(id);
  }, []);
  const left = Math.max(0, Math.round((until - now) / 1000));
  return (
    <span data-testid="remote-expiry">
      {left > 0
        ? t('remoteShow.expiresIn', {
            time: `${Math.floor(left / 60)}:${String(left % 60).padStart(2, '0')}`,
          })
        : t('remoteShow.expired')}
    </span>
  );
}

export interface RemoteDialogProps {
  state: RemotePresentingState;
  onPair: () => void;
  onRevoke: () => void;
  onClose: () => void;
}

/** "Control from a phone": QR code + link of the remote page, pairing status, revoke. */
export function RemoteDialog({ state, onPair, onRevoke, onClose }: RemoteDialogProps) {
  const { t } = useTranslation();
  const p = state.pairing;
  // Opened without a phone: a pairing code at once (once the server said it can).
  const asked = useRef(false);
  const needsCode = state.available === true && !p && !state.paired && !state.busy;
  useEffect(() => {
    if (!needsCode || asked.current) return;
    asked.current = true;
    onPair();
  }, [needsCode, onPair]);
  return (
    <Dialog
      open
      onOpenChange={(o) => !o && onClose()}
      title={t('remoteShow.remoteTitle')}
      testId="remote-dialog"
      width={440}
      footer={
        <>
          {(p || state.paired) && (
            <button type="button" data-testid="remote-revoke" onClick={onRevoke}>
              {t('remoteShow.revoke')}
            </button>
          )}
          {state.available && (
            <button
              type="button"
              data-testid="remote-new-code"
              onClick={onPair}
              disabled={state.busy}
            >
              {t('remoteShow.newCode')}
            </button>
          )}
          <button type="button" data-testid="remote-close" onClick={onClose}>
            {t('showTools.close')}
          </button>
        </>
      }
    >
      {state.available === false ? (
        <p className={styles.muted} data-testid="remote-unavailable">
          {t('remoteShow.unavailable')}
        </p>
      ) : (
        <>
          <p className={styles.muted}>{t('remoteShow.remoteHint')}</p>
          {state.error && (
            <p role="alert" className={styles.error} data-testid="remote-error">
              {t(`remoteShow.errors.${state.error}`)}
            </p>
          )}
          {p && !state.paired && (
            <div className={styles.pairing}>
              <QrCode text={p.url} label={t('remoteShow.qrLabel')} />
              <a className={styles.link} href={p.url} data-testid="remote-url">
                {p.url}
              </a>
              <Countdown until={p.expiresAt} />
              {isLocalOnly(p.url) && (
                <p className={styles.warn} data-testid="remote-local-warning">
                  {t('remoteShow.localOnly')}
                </p>
              )}
            </div>
          )}
          <p className={styles.status} data-testid="remote-status" role="status">
            {state.phone
              ? t('remoteShow.phoneConnected')
              : state.paired
                ? t('remoteShow.phonePaired')
                : p
                  ? t('remoteShow.waitingPhone')
                  : state.busy
                    ? t('remoteShow.starting')
                    : t('remoteShow.notPaired')}
          </p>
        </>
      )}
    </Dialog>
  );
}

export interface OnlineDialogProps {
  state: RemotePresentingState;
  onStart: () => void;
  onEnd: () => void;
  onClose: () => void;
}

/** "Present online": the viewers' link, how many follow, end. */
export function OnlineDialog({ state, onStart, onEnd, onClose }: OnlineDialogProps) {
  const { t } = useTranslation();
  const [copied, setCopied] = useState(false);
  const url = state.online?.url ?? null;
  return (
    <Dialog
      open
      onOpenChange={(o) => !o && onClose()}
      title={t('remoteShow.onlineTitle')}
      testId="online-dialog"
      width={460}
      footer={
        <>
          {url ? (
            <button type="button" data-testid="online-end" onClick={onEnd} disabled={state.busy}>
              {t('remoteShow.endOnline')}
            </button>
          ) : (
            state.available && (
              <button
                type="button"
                data-testid="online-start"
                onClick={onStart}
                disabled={state.busy || !state.canBroadcast}
              >
                {t('remoteShow.startOnline')}
              </button>
            )
          )}
          <button type="button" data-testid="online-close" onClick={onClose}>
            {t('showTools.close')}
          </button>
        </>
      }
    >
      {state.available === false ? (
        <p className={styles.muted} data-testid="online-unavailable">
          {t('remoteShow.unavailable')}
        </p>
      ) : (
        <>
          <p className={styles.muted}>{t('remoteShow.onlineHint')}</p>
          {!state.canBroadcast && (
            <p className={styles.warn} data-testid="online-forbidden">
              {t('remoteShow.errors.forbidden')}
            </p>
          )}
          {state.error && (
            <p role="alert" className={styles.error} data-testid="online-error">
              {t(`remoteShow.errors.${state.error}`)}
            </p>
          )}
          {url && (
            <div className={styles.pairing}>
              <QrCode text={url} label={t('remoteShow.qrLabel')} size={160} />
              <div className={styles.row}>
                <input
                  className={styles.urlField}
                  readOnly
                  value={url}
                  data-testid="online-url"
                  aria-label={t('remoteShow.viewerLink')}
                  onFocus={(e) => e.currentTarget.select()}
                />
                <button
                  type="button"
                  data-testid="online-copy"
                  onClick={() => {
                    void navigator.clipboard
                      ?.writeText(url)
                      .then(() => setCopied(true))
                      .catch(() => {});
                  }}
                >
                  {copied ? t('remoteShow.copied') : t('remoteShow.copy')}
                </button>
              </div>
              <p className={styles.status} role="status" data-testid="online-viewers">
                {t('remoteShow.viewers', { n: state.viewers })}
              </p>
              {isLocalOnly(url) && <p className={styles.warn}>{t('remoteShow.localOnly')}</p>}
            </div>
          )}
        </>
      )}
    </Dialog>
  );
}
