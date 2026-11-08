import { GlyphStore, ImageStore, renderPage } from '@blueoffice/canvas-host';
import type { EngineHost, PageDisplayList, SlideShowInfo } from '@blueoffice/platform';
import { useEffect, useRef, useState } from 'react';
import { MediaClip } from './MediaLayer';
import {
  effectsAt,
  stepDuration,
  transitionPlan,
  visibility,
  type Frames,
  type ShowPos,
} from './player';
import { SlideCanvas } from './SlideCanvas';
import styles from './SlideEditor.module.css';

/** Show data of slides, fetched once per deck revision (`show` command). */
export class ShowCache {
  private readonly map = new Map<number, Promise<SlideShowInfo | null>>();
  private readonly done = new Map<number, SlideShowInfo>();

  /** `tag` only distinguishes caches built for different runs (React memo key). */
  constructor(
    private readonly engine: EngineHost,
    readonly tag = '',
  ) {}

  get(slide: number): Promise<SlideShowInfo | null> {
    let p = this.map.get(slide);
    if (!p) {
      p = this.engine
        .slideCommand({ cmd: 'show', slide })
        .then((r) => {
          const s = r.show ?? null;
          if (s) this.done.set(slide, s);
          return s;
        })
        .catch(() => null);
      this.map.set(slide, p);
    }
    return p;
  }

  /** Already fetched show data. */
  peek(slide: number): SlideShowInfo | undefined {
    return this.done.get(slide);
  }

  clear(): void {
    this.map.clear();
    this.done.clear();
  }
}

/** Web Animations, when the platform has them (not in jsdom). */
function animate(
  el: Element | null | undefined,
  frames: Frames,
  opts: KeyframeAnimationOptions,
): Animation | null {
  if (!el || typeof (el as HTMLElement).animate !== 'function') return null;
  const a = (el as HTMLElement).animate(frames as Keyframe[], opts);
  // Cancelling rejects `finished`; nothing waits on it.
  a.finished?.catch(() => {});
  return a;
}

function LayerCanvas({
  page,
  glyphs,
  images,
  width,
  height,
}: {
  page: PageDisplayList;
  glyphs: GlyphStore;
  images: ImageStore;
  width: number;
  height: number;
}) {
  const ref = useRef<HTMLCanvasElement>(null);
  useEffect(() => {
    let cancelled = false;
    void (async () => {
      if (width <= 0 || height <= 0) return;
      await Promise.all([glyphs.ensure(page), images.ensure(page)]);
      const canvas = ref.current;
      const ctx = canvas?.getContext('2d');
      if (cancelled || !canvas || !ctx) return;
      const dpr = window.devicePixelRatio || 1;
      canvas.width = Math.round(width * dpr);
      canvas.height = Math.round(height * dpr);
      renderPage(ctx, page, glyphs, (width / page.width) * dpr, images);
    })().catch(() => {});
    return () => {
      cancelled = true;
    };
  }, [page, glyphs, images, width, height]);
  return <canvas ref={ref} className={styles.showCanvas} style={{ width, height }} />;
}

export interface ShowStageProps {
  engine: EngineHost;
  glyphs: GlyphStore;
  images: ImageStore;
  cache: ShowCache;
  pos: ShowPos;
  /** Slide shown before `pos.slide` (outgoing slide of the transition). */
  from: number | null;
  /** Slide size in points (fallback before the show data arrives). */
  slide: { width: number; height: number };
  width: number;
  /** Play media clips (slide show) or leave the poster frames (thumbnails, previews). */
  media?: boolean;
  /** All effects of the step are done (auto-advance, previews). */
  onIdle?: (show: SlideShowInfo) => void;
  label?: string;
  mediaLabels?: { play: string; unsupported: string };
}

/**
 * One slide of the slide show: its layers, the transition from the previous
 * slide and the effects of the current click step.
 */
export function ShowStage({
  engine,
  glyphs,
  images,
  cache,
  pos,
  from,
  slide,
  width,
  media = true,
  onIdle,
  label,
  mediaLabels,
}: ShowStageProps) {
  const [show, setShow] = useState<SlideShowInfo | null>(null);
  /** Layer visibility and outgoing slide once the step started / ended (by step key). */
  const [phase, setPhase] = useState<{ key: string; vis: boolean[]; outgoing: boolean } | null>(
    null,
  );
  const layerRefs = useRef<(HTMLDivElement | null)[]>([]);
  const incomingRef = useRef<HTMLDivElement>(null);
  const outgoingRef = useRef<HTMLDivElement>(null);
  const onIdleRef = useRef(onIdle);
  useEffect(() => {
    onIdleRef.current = onIdle;
  }, [onIdle]);
  const size = show ?? slide;
  const height = size.width > 0 ? Math.round((width * size.height) / size.width) : 0;
  const ready = show !== null && show.slide === pos.slide;
  const key = `${pos.slide}:${pos.click}:${pos.enter}:${from ?? ''}`;
  const playing = pos.enter !== 'jump';
  const hasOutgoing = pos.enter === 'transition' && from !== null && from !== pos.slide;
  // Before the step starts: the state before its effects (or after them, for a jump).
  const current =
    phase && phase.key === key
      ? phase
      : {
          key,
          vis: ready && show ? visibility(show, playing ? pos.click - 1 : pos.click) : [],
          outgoing: hasOutgoing,
        };
  const vis = current.vis;
  const outgoing = current.outgoing && from !== null ? from : null;

  useEffect(() => {
    let cancelled = false;
    void cache.get(pos.slide).then((s) => {
      if (cancelled) return;
      setShow(s && s.slide === pos.slide ? s : null);
    });
    return () => {
      cancelled = true;
    };
  }, [cache, pos.slide]);

  // Play the step: transition + the slide's own effects, one click, or jump.
  useEffect(() => {
    if (!show || show.slide !== pos.slide) return;
    let cancelled = false;
    const anims: Animation[] = [];
    const start = visibility(show, playing ? pos.click - 1 : pos.click);
    let end: ReturnType<typeof setTimeout> | undefined;
    const timer = setTimeout(() => {
      if (cancelled) return;
      let delay = 0;
      if (pos.enter === 'transition') {
        const t = transitionPlan(show.transition);
        if (t.duration > 0) {
          const opts = { duration: t.duration, easing: 'ease-in-out', fill: 'both' as const };
          if (t.incoming) {
            const a = animate(incomingRef.current, t.incoming, opts);
            if (a) anims.push(a);
          }
          if (t.outgoing) {
            const a = animate(outgoingRef.current, t.outgoing, opts);
            if (a) anims.push(a);
          }
          delay = t.duration;
        }
      }
      let total = delay;
      if (playing) {
        const next = [...start];
        const seen = new Set<number>();
        for (const e of effectsAt(show, pos.click)) {
          // The layer is shown while its effect runs; the first effect's frames
          // hide it until it starts, later ones on the same layer hold only after.
          next[e.layer] = true;
          const a = animate(layerRefs.current[e.layer], e.frames, {
            delay: delay + e.delay,
            duration: e.duration,
            easing: 'ease-out',
            fill: seen.has(e.layer) ? 'forwards' : 'both',
          });
          seen.add(e.layer);
          if (a) anims.push(a);
        }
        setPhase({ key, vis: next, outgoing: hasOutgoing });
        total += stepDuration(show, pos.click);
      }
      const finish = () => {
        if (cancelled) return;
        for (const a of anims) a.cancel();
        setPhase({ key, vis: visibility(show, pos.click), outgoing: false });
        onIdleRef.current?.(show);
      };
      if (total > 0) end = setTimeout(finish, total + 30);
      else finish();
    }, 0);
    return () => {
      cancelled = true;
      clearTimeout(timer);
      clearTimeout(end);
      for (const a of anims) a.cancel();
    };
  }, [show, pos.slide, pos.click, pos.enter, playing, hasOutgoing, key]);

  return (
    <div
      className={styles.showStage}
      style={{ width, height }}
      data-testid="show-stage"
      data-slide={pos.slide}
      data-click={pos.click}
      data-clicks={show?.clicks ?? 0}
      data-transition={show?.transition?.kind ?? 'none'}
      role="img"
      aria-label={label}
    >
      {outgoing !== null && (
        <div
          ref={outgoingRef}
          className={styles.showLayer}
          style={{ zIndex: transitionPlan(show?.transition ?? null).outgoingOnTop ? 2 : 0 }}
        >
          <SlideCanvas
            engine={engine}
            glyphs={glyphs}
            images={images}
            index={outgoing}
            slide={slide}
            width={width}
          />
        </div>
      )}
      <div ref={incomingRef} className={styles.showLayer} style={{ zIndex: 1 }}>
        {ready &&
          show.layers.map((l, i) => (
            <div
              key={i}
              ref={(el) => {
                layerRefs.current[i] = el;
              }}
              className={styles.showLayer}
              data-testid="show-layer"
              data-ids={l.ids.join(' ')}
              data-visible={vis[i] ?? true}
              style={{ visibility: (vis[i] ?? true) ? 'visible' : 'hidden' }}
            >
              <LayerCanvas
                page={l.page}
                glyphs={glyphs}
                images={images}
                width={width}
                height={height}
              />
            </div>
          ))}
        {ready &&
          media &&
          show.media.map((m) => (
            <MediaClip
              key={`${pos.slide}-${m.id}`}
              engine={engine}
              clip={m}
              scale={width / (show.width || 1)}
              labels={mediaLabels}
            />
          ))}
      </div>
    </div>
  );
}
