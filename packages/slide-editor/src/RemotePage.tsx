/**
 * The phone remote (SLIDE-REMOTE-001): opened from the presenter's QR code (`/remote/<code>`).
 * The code is exchanged once for this phone's key (kept for the tab, so a reload reconnects),
 * then the page shows the current and next slides, the notes and the timer, and sends next /
 * previous / black / end to that presenter through the server.
 */
import { createI18n, I18nextProvider, useTranslation, type Language } from '@blueoffice/i18n';
import {
  claimRemote,
  connectRemote,
  RemoteShowError,
  type RemoteAction,
  type RemoteLink,
  type RemoteShowOptions,
  type RemoteState,
} from '@blueoffice/platform';
import { useEffect, useRef, useState } from 'react';
import { formatElapsed } from './show';
import styles from './remote.module.css';

export interface RemotePageProps {
  /** Pairing code from the link. */
  token: string;
  options: RemoteShowOptions;
  language?: Language;
  /** Where the phone's key is kept for this tab (default `sessionStorage`). */
  store?: Pick<Storage, 'getItem' | 'setItem' | 'removeItem'> | null;
  now?: () => number;
}

type Phase = 'pairing' | 'connected' | 'offline' | 'invalid' | 'revoked' | 'ended' | 'error';

function defaultStore(): RemotePageProps['store'] {
  try {
    return typeof sessionStorage !== 'undefined' ? sessionStorage : null;
  } catch {
    return null;
  }
}

function Remote({ token, options, store = defaultStore(), now = Date.now }: RemotePageProps) {
  const { t } = useTranslation();
  const [phase, setPhase] = useState<Phase>('pairing');
  const [presenter, setPresenter] = useState(false);
  const [state, setState] = useState<RemoteState | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const [clock, setClock] = useState(now());
  const link = useRef<RemoteLink | null>(null);

  useEffect(() => {
    const id = setInterval(() => setClock(now()), 1000);
    return () => clearInterval(id);
  }, [now]);

  useEffect(() => {
    let cancelled = false;
    const slot = `bo.remote.${token}`;
    const read = () => {
      try {
        return store?.getItem(slot) ?? null;
      } catch {
        return null;
      }
    };
    void (async () => {
      let key = read();
      const reused = key !== null;
      if (!key) {
        try {
          key = await claimRemote(options, token);
          try {
            store?.setItem(slot, key);
          } catch {
            // Not kept: a reload needs a new code.
          }
        } catch (e) {
          if (!cancelled) {
            setPhase(
              e instanceof RemoteShowError && e.code === 'invalidToken' ? 'invalid' : 'error',
            );
            if (e instanceof RemoteShowError && e.code === 'rateLimited')
              setNotice(t('remoteShow.errors.rateLimited'));
          }
          return;
        }
      }
      if (cancelled) return;
      let opened = false;
      link.current = connectRemote(options, key, (e) => {
        if (cancelled) return;
        switch (e.type) {
          case 'remote':
            opened = true;
            setState(e);
            break;
          case 'peers':
            opened = true;
            setPhase('connected');
            setPresenter(e.presenter);
            break;
          case 'error':
            setNotice(t(`remoteShow.phoneErrors.${e.code}`, { defaultValue: e.code }));
            break;
          case 'revoked':
            setPhase('revoked');
            break;
          case 'offline':
            setPhase('offline');
            break;
          case 'ended':
            try {
              store?.removeItem(slot);
            } catch {
              // ignore
            }
            setPhase((p) => (p === 'revoked' ? p : !opened && reused ? 'invalid' : 'ended'));
            break;
          default:
            break;
        }
      });
    })();
    return () => {
      cancelled = true;
      link.current?.close();
      link.current = null;
    };
  }, [token, options, store, t]);

  // A notice (rate limit, presenter away) fades after a moment.
  useEffect(() => {
    if (!notice) return;
    const id = setTimeout(() => setNotice(null), 4000);
    return () => clearTimeout(id);
  }, [notice]);

  const send = (a: RemoteAction) => {
    link.current?.send(a);
  };
  const live = phase === 'connected' && presenter;
  const showing = !!state?.showing;
  const title =
    phase === 'pairing'
      ? t('remoteShow.phonePairing')
      : phase === 'invalid'
        ? t('remoteShow.phoneInvalid')
        : phase === 'revoked'
          ? t('remoteShow.phoneRevoked')
          : phase === 'ended'
            ? t('remoteShow.phoneEnded')
            : phase === 'offline'
              ? t('remoteShow.phoneOffline')
              : phase === 'error'
                ? t('remoteShow.errors.network')
                : !presenter
                  ? t('remoteShow.presenterAway')
                  : null;
  const done = phase === 'invalid' || phase === 'revoked' || phase === 'ended' || phase === 'error';

  return (
    <main className={styles.phone} data-testid="remote-page" data-phase={phase}>
      <header className={styles.phoneHead}>
        <h1>{t('remoteShow.phoneTitle')}</h1>
        {state?.startedAt && showing && (
          <span className={styles.timer} data-testid="remote-timer">
            {formatElapsed(clock - state.startedAt)}
          </span>
        )}
      </header>
      {title && (
        <p className={styles.banner} role="status" data-testid="remote-banner">
          {title}
        </p>
      )}
      {notice && (
        <p className={styles.banner} role="alert" data-testid="remote-notice">
          {notice}
        </p>
      )}
      {!done && state && (
        <>
          <p data-testid="remote-position" aria-live="polite">
            {showing
              ? state.ended
                ? t('slideEditor.end')
                : t('remoteShow.position', { n: state.at, total: state.of, slide: state.slide + 1 })
              : t('remoteShow.notShowing', { n: state.slide + 1, total: state.total })}
            {showing && state.clicks > 0 && !state.ended
              ? ` · ${t('remoteShow.steps', { n: state.click, total: state.clicks })}`
              : ''}
          </p>
          <div className={styles.thumbs}>
            <figure className={styles.thumb}>
              {state.current ? (
                <img
                  src={state.current}
                  alt={t('remoteShow.current')}
                  data-testid="remote-current"
                />
              ) : (
                <div className={styles.thumbEmpty} />
              )}
              <figcaption>{t('remoteShow.current')}</figcaption>
            </figure>
            <figure className={styles.thumb}>
              {state.next ? (
                <img src={state.next} alt={t('slideEditor.next')} data-testid="remote-next-thumb" />
              ) : (
                <div className={styles.thumbEmpty} />
              )}
              <figcaption>{t('slideEditor.next')}</figcaption>
            </figure>
          </div>
          <div className={styles.notes} data-testid="remote-notes">
            {state.notes || t('slideEditor.noNotes')}
          </div>
        </>
      )}
      {!done && (
        <div className={styles.buttons}>
          {showing ? (
            <>
              <button
                type="button"
                className={styles.primary}
                style={{ gridColumn: '1 / -1' }}
                data-testid="remote-next"
                disabled={!live}
                onClick={() => send('next')}
              >
                {t('slideEditor.next')}
              </button>
              <button
                type="button"
                data-testid="remote-prev"
                disabled={!live}
                onClick={() => send('prev')}
              >
                {t('slideEditor.previous')}
              </button>
              <button
                type="button"
                data-testid="remote-black"
                aria-pressed={state?.black ?? false}
                disabled={!live}
                onClick={() => send('black')}
              >
                {t('slideEditor.blackScreen')}
              </button>
              <button
                type="button"
                data-testid="remote-end"
                style={{ gridColumn: '1 / -1' }}
                disabled={!live}
                onClick={() => send('end')}
              >
                {t('remoteShow.endShow')}
              </button>
            </>
          ) : (
            <button
              type="button"
              className={styles.primary}
              style={{ gridColumn: '1 / -1' }}
              data-testid="remote-start"
              disabled={!live || !state}
              onClick={() => send('start')}
            >
              {t('remoteShow.startShow')}
            </button>
          )}
        </div>
      )}
    </main>
  );
}

/** The phone remote page with its own translations. */
export function RemotePage(props: RemotePageProps) {
  const [i18n] = useState(() => createI18n(props.language));
  return (
    <I18nextProvider i18n={i18n}>
      <Remote {...props} />
    </I18nextProvider>
  );
}
