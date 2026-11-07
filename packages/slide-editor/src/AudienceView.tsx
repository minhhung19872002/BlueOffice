import { GlyphStore, ImageStore } from '@blueoffice/canvas-host';
import { useTranslation } from '@blueoffice/i18n';
import type { DocInfo, Platform } from '@blueoffice/platform';
import { useEffect, useMemo, useState } from 'react';
import { fromBase64, isAudienceMsg, type AudienceMsg } from './audience';
import { fit } from './show';
import { SlideCanvas } from './SlideCanvas';
import styles from './SlideEditor.module.css';

export interface AudienceViewProps {
  platform: Platform;
  /** Channel id given by the presenter window. */
  channel: string;
}

/**
 * Audience screen of the presenter view (second window / screen): only the
 * current slide, following the presenter (slide, black screen, end).
 */
export function AudienceView({ platform, channel }: AudienceViewProps) {
  const { t } = useTranslation();
  const engine = platform.engine;
  const glyphs = useMemo(() => new GlyphStore(engine), [engine]);
  const images = useMemo(() => new ImageStore(engine), [engine]);
  const [info, setInfo] = useState<DocInfo | null>(null);
  const [show, setShow] = useState({ slide: 0, black: false, ended: false });
  const [size, setSize] = useState({ width: window.innerWidth, height: window.innerHeight });

  useEffect(() => {
    platform.window.setTitle(t('slideEditor.audienceTitle'));
    const onResize = () => setSize({ width: window.innerWidth, height: window.innerHeight });
    window.addEventListener('resize', onResize);
    return () => window.removeEventListener('resize', onResize);
  }, [platform, t]);

  useEffect(() => {
    const bus = platform.channel?.(channel);
    if (!bus) return;
    let loaded = false;
    let cancelled = false;
    const off = bus.onMessage((m) => {
      if (!isAudienceMsg(m)) return;
      const msg: AudienceMsg = m;
      if (msg.type === 'deck' && !loaded) {
        loaded = true;
        void (async () => {
          await engine.init();
          const doc = await engine.open(fromBase64(msg.data), msg.name);
          if (cancelled) return;
          setInfo(doc);
          setShow({ slide: msg.slide, black: msg.black, ended: false });
        })();
      } else if (msg.type === 'show') {
        setShow({ slide: msg.slide, black: msg.black, ended: msg.ended });
      }
    });
    // Ask until the presenter answers (it may still be setting up).
    bus.post({ type: 'hello' } satisfies AudienceMsg);
    const timer = setInterval(() => {
      if (loaded) clearInterval(timer);
      else bus.post({ type: 'hello' } satisfies AudienceMsg);
    }, 700);
    return () => {
      cancelled = true;
      clearInterval(timer);
      off();
      bus.close();
    };
  }, [platform, channel, engine]);

  const count = info?.pageCount ?? 0;
  const page = info?.pages[Math.min(show.slide, Math.max(0, count - 1))] ?? {
    width: 960,
    height: 540,
  };
  const box = fit(page, size);
  const visible = info && !show.ended && !show.black && show.slide < count;
  return (
    <div className={`bo-root ${styles.show}`} data-app="slide" data-testid="audience-view">
      {!info ? (
        <p className={styles.endText}>{t('slideEditor.audienceWaiting')}</p>
      ) : visible ? (
        <div key={show.slide} className={styles.fade}>
          <SlideCanvas
            engine={engine}
            glyphs={glyphs}
            images={images}
            index={show.slide}
            slide={page}
            width={box.width}
            revision={1}
            label={t('slideEditor.slideOf', { n: show.slide + 1, total: count })}
          />
        </div>
      ) : (
        (show.ended || show.slide >= count) && (
          <p className={styles.endText}>{t('slideEditor.end')}</p>
        )
      )}
    </div>
  );
}
