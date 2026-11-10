/**
 * A viewer of an online presentation (SLIDE-REMOTE-001), opened from the presenter's link
 * (`/watch/<token>`): the deck is downloaded once with the link token and drawn by the same
 * engine as the editor; the slide, the animation step, black / white screens, ink and zoom
 * follow the presenter. Viewers have no controls: nothing they do moves the show.
 */
import { GlyphStore, ImageStore } from '@blueoffice/canvas-host';
import { createI18n, I18nextProvider, useTranslation, type Language } from '@blueoffice/i18n';
import {
  connectWatch,
  fetchWatchDeck,
  RemoteShowError,
  type DocInfo,
  type Platform,
  type RemoteShowOptions,
} from '@blueoffice/platform';
import { useEffect, useMemo, useState } from 'react';
import type { AudienceMsg, AudienceTools } from './audience';
import { AudienceScreen } from './AudienceView';
import type { ShowPos } from './player';
import { ShowCache } from './ShowStage';
import styles from './SlideEditor.module.css';

export interface WatchViewProps {
  platform: Platform;
  /** Link token. */
  token: string;
  options: RemoteShowOptions;
  language?: Language;
}

/** `show` message of the presenter: an `AudienceMsg` plus whether the show runs. */
type ShowMsg = Extract<AudienceMsg, { type: 'show' }> & { live?: boolean };

function Watch({ platform, token, options }: WatchViewProps) {
  const { t } = useTranslation();
  const engine = platform.engine;
  const glyphs = useMemo(() => new GlyphStore(engine), [engine]);
  const images = useMemo(() => new ImageStore(engine), [engine]);
  useEffect(() => () => images.clear(), [images]);
  const [info, setInfo] = useState<DocInfo | null>(null);
  const [rev, setRev] = useState(0);
  const cache = useMemo(() => new ShowCache(engine, String(rev)), [engine, rev]);
  const [phase, setPhase] = useState<'loading' | 'live' | 'ended' | 'invalid' | 'error'>('loading');
  const [live, setLive] = useState(false);
  const [show, setShow] = useState({ slide: 0, black: false, ended: false });
  const [tools, setTools] = useState<AudienceTools | null>(null);
  const [pos, setPos] = useState<ShowPos & { from: number | null }>({
    slide: 0,
    click: 0,
    enter: 'jump',
    from: null,
  });
  const [size, setSize] = useState({ width: window.innerWidth, height: window.innerHeight });

  useEffect(() => {
    platform.window.setTitle(t('remoteShow.watchTitle'));
    const onResize = () => setSize({ width: window.innerWidth, height: window.innerHeight });
    window.addEventListener('resize', onResize);
    return () => window.removeEventListener('resize', onResize);
  }, [platform, t]);

  useEffect(() => {
    let cancelled = false;
    let conn: { close(): void } | null = null;
    const load = async () => {
      await engine.init();
      const bytes = await fetchWatchDeck(options, token);
      const doc = await engine.open(bytes, 'deck.pptx');
      images.clear();
      if (cancelled) return;
      setInfo(doc);
      setRev((r) => r + 1);
    };
    void (async () => {
      try {
        await load();
      } catch (e) {
        if (!cancelled)
          setPhase(e instanceof RemoteShowError && e.code === 'invalidToken' ? 'invalid' : 'error');
        return;
      }
      if (cancelled) return;
      setPhase('live');
      conn = connectWatch(options, token, (e) => {
        if (cancelled) return;
        if (e.type === 'show') {
          const m = e as unknown as ShowMsg;
          setLive(m.live !== false);
          setShow({ slide: m.slide, black: m.black, ended: m.ended });
          setTools(m.tools ?? null);
          setPos((p) => ({
            slide: m.slide,
            click: m.click ?? 0,
            enter: m.enter ?? 'jump',
            from: m.enter === 'transition' && p.slide !== m.slide ? p.slide : null,
            ...(m.zoom ? { zoom: m.zoom } : {}),
          }));
        } else if (e.type === 'reload') {
          // The presenter sent the deck again (edited): open it again, keep following.
          void load().catch(() => {});
        } else if (e.type === 'ended') {
          setPhase('ended');
        }
      });
    })();
    return () => {
      cancelled = true;
      conn?.close();
    };
  }, [engine, images, options, token]);

  if (phase !== 'live' && phase !== 'loading') {
    return (
      <div
        className={`bo-root ${styles.show}`}
        data-app="slide"
        data-testid="watch-view"
        data-phase={phase}
      >
        <p className={styles.endText} data-testid="watch-message">
          {phase === 'ended'
            ? t('remoteShow.watchEnded')
            : phase === 'invalid'
              ? t('remoteShow.watchInvalid')
              : t('remoteShow.errors.network')}
        </p>
      </div>
    );
  }
  return (
    <AudienceScreen
      engine={engine}
      glyphs={glyphs}
      images={images}
      cache={cache}
      info={info}
      show={show}
      tools={tools}
      pos={pos}
      size={size}
      waiting={live ? null : t('remoteShow.watchWaiting')}
      testId="watch-view"
    />
  );
}

/** Online presentation viewer with its own translations. */
export function WatchView(props: WatchViewProps) {
  const [i18n] = useState(() => createI18n(props.language));
  return (
    <I18nextProvider i18n={i18n}>
      <Watch {...props} />
    </I18nextProvider>
  );
}
