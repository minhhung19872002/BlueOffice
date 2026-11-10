/**
 * Design suggestions (CORE-DESIGN-001, "Gợi ý thiết kế"): a task pane listing the layouts the
 * engine proposes for the current slide (`designIdeas`) as live thumbnails; a click applies one
 * (`designApply`, one undo step). The suggestions only rearrange the slide's own shapes, crop its
 * pictures and add accent shapes in theme colours — no content is invented.
 */
import { GlyphStore, ImageStore, renderPage } from '@blueoffice/canvas-host';
import { useTranslation } from '@blueoffice/i18n';
import type {
  PageDisplayList,
  SlideCommand,
  SlideDesignIdeas,
  SlideEditResult,
} from '@blueoffice/platform';
import { useEffect, useRef, useState } from 'react';
import styles from './SlideEditor.module.css';

/** Thumbnail width asked from the engine, points. */
export const IDEA_WIDTH = 200;

/** One suggestion's thumbnail drawn from its display list. */
function IdeaCanvas({
  page,
  glyphs,
  images,
  width,
}: {
  page: PageDisplayList;
  glyphs: GlyphStore;
  images: ImageStore;
  width: number;
}) {
  const ref = useRef<HTMLCanvasElement>(null);
  const height = page.width > 0 ? Math.round((width * page.height) / page.width) : 0;
  useEffect(() => {
    let cancelled = false;
    void (async () => {
      await Promise.all([glyphs.ensure(page), images.ensure(page)]);
      const canvas = ref.current;
      const ctx = canvas?.getContext('2d');
      if (cancelled || !canvas || !ctx || width <= 0 || height <= 0) return;
      const dpr = window.devicePixelRatio || 1;
      canvas.width = Math.round(width * dpr);
      canvas.height = Math.round(height * dpr);
      renderPage(ctx, page, glyphs, (width / page.width) * dpr, images);
    })().catch(() => {});
    return () => {
      cancelled = true;
    };
  }, [page, glyphs, images, width, height]);
  return <canvas ref={ref} style={{ width, height }} aria-hidden />;
}

export interface DesignIdeasPaneProps {
  glyphs: GlyphStore;
  images: ImageStore;
  /** Current slide (the ideas are asked again when it changes). */
  slide: number;
  /** Bumped when the current slide changes. */
  revision: number;
  /** Runs a slide command and returns its result. */
  run: (cmd: SlideCommand) => Promise<SlideEditResult | null>;
  /** Displayed thumbnail width (CSS pixels). */
  width?: number;
}

/** The design suggestions pane. */
export function DesignIdeasPane({
  glyphs,
  images,
  slide,
  revision,
  run,
  width = 220,
}: DesignIdeasPaneProps) {
  const { t } = useTranslation();
  const [ideas, setIdeas] = useState<SlideDesignIdeas | null>(null);
  const [error, setError] = useState<string | null>(null);
  // The ideas of the slide as it is now; an applied idea changes the slide and asks again.
  const shown = useRef(-1);
  useEffect(() => {
    let live = true;
    const ask = () =>
      void run({ cmd: 'designIdeas', width: IDEA_WIDTH }).then((r) => {
        if (!live) return;
        setIdeas(r?.designIdeas ?? { kind: null, ideas: [] });
        setError(r?.error ?? null);
      });
    // A new slide is asked at once; edits of the same slide (typing) are waited out.
    if (shown.current !== slide) {
      shown.current = slide;
      setIdeas(null);
      ask();
      return () => {
        live = false;
      };
    }
    const timer = setTimeout(ask, 400);
    return () => {
      live = false;
      clearTimeout(timer);
    };
  }, [run, slide, revision]);
  if (!ideas) return <p className={styles.designIdeasHint}>{t('designIdeas.loading')}</p>;
  if (!ideas.kind || ideas.ideas.length === 0) {
    return (
      <p className={styles.designIdeasHint} data-testid="design-ideas-empty">
        {error === 'masterView' ? t('designIdeas.masterView') : t('designIdeas.empty')}
      </p>
    );
  }
  return (
    <div className={styles.designIdeas} data-testid="design-ideas">
      <p className={styles.designIdeasHint}>
        {t(`designIdeas.kind.${ideas.kind}`)} · {t('designIdeas.intro')}
      </p>
      <div className={styles.designIdeasList} role="list">
        {ideas.ideas.map((idea, i) => (
          <button
            key={idea.id}
            type="button"
            role="listitem"
            className={styles.designIdea}
            data-testid={`design-idea-${i}`}
            data-idea={idea.id}
            aria-label={t('designIdeas.apply', { n: i + 1 })}
            title={t('designIdeas.apply', { n: i + 1 })}
            onClick={() => void run({ cmd: 'designApply', idea: idea.id })}
          >
            <IdeaCanvas page={idea.page} glyphs={glyphs} images={images} width={width} />
          </button>
        ))}
      </div>
    </div>
  );
}
