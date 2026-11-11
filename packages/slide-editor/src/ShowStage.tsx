import { GlyphStore, ImageStore, renderPage } from '@blueoffice/canvas-host';
import type {
  EngineHost,
  PageDisplayList,
  SlideMorphOutline,
  SlideActionSound,
  SlideShowInfo,
  SlideShowLink,
  SlideTransitionSound,
} from '@blueoffice/platform';
import { useEffect, useRef, useState } from 'react';
import { hotAreas } from './actions';
import actionStyles from './actions.module.css';
import { showMediaCarry } from './mediaControl';
import { ShowMediaClip } from './showMedia';
import {
  effectsAt,
  partVisibility,
  stepDuration,
  tintsShown,
  transitionPlan,
  triggerEffects,
  visibility,
  type PlannedEffect,
  type Frames,
  type ShowPos,
} from './player';
import { morphLayers, outlineFrame, type TransitionPlan } from './transitionFx';
import { zoomFrames, type ZoomMove } from './presenting';
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

  /**
   * The slides are drawn again the next time they are shown (a followed link
   * changed colour); what is known about them stays until then.
   */
  stale(): void {
    this.map.clear();
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

/** Room around a word / letter drawn on a canvas of its own (points). */
const PIECE_MARGIN = 3;

/**
 * One drawing of the stage. Layers over the backdrop are see-through (`transparent`); a word or
 * letter (`rect`, points) gets a canvas the size of its box, placed where it is on the slide.
 */
export function LayerCanvas({
  page,
  glyphs,
  images,
  width,
  height,
  transparent = false,
  rect,
}: {
  page: PageDisplayList;
  glyphs: GlyphStore;
  images: ImageStore;
  width: number;
  height: number;
  transparent?: boolean;
  rect?: [number, number, number, number];
}) {
  const ref = useRef<HTMLCanvasElement>(null);
  const k = page.width > 0 ? width / page.width : 0;
  const box = rect
    ? {
        x: rect[0] - PIECE_MARGIN,
        y: rect[1] - PIECE_MARGIN,
        w: rect[2] + 2 * PIECE_MARGIN,
        h: rect[3] + 2 * PIECE_MARGIN,
      }
    : null;
  const cw = box ? box.w * k : width;
  const ch = box ? box.h * k : height;
  const bx = box?.x ?? 0;
  const by = box?.y ?? 0;
  const piece = box !== null;
  useEffect(() => {
    let cancelled = false;
    void (async () => {
      if (cw <= 0 || ch <= 0 || k <= 0) return;
      await Promise.all([glyphs.ensure(page), images.ensure(page)]);
      const canvas = ref.current;
      const ctx = canvas?.getContext('2d');
      if (cancelled || !canvas || !ctx) return;
      const dpr = window.devicePixelRatio || 1;
      canvas.width = Math.max(1, Math.round(cw * dpr));
      canvas.height = Math.max(1, Math.round(ch * dpr));
      renderPage(ctx, page, glyphs, k * dpr, images, {
        transparent: transparent || piece,
        origin: [bx, by],
      });
    })().catch(() => {});
    return () => {
      cancelled = true;
    };
  }, [page, glyphs, images, cw, ch, k, bx, by, transparent, piece]);
  return (
    <canvas
      ref={ref}
      className={styles.showCanvas}
      style={
        piece
          ? { left: bx * k, top: by * k, width: cw, height: ch, right: 'auto', bottom: 'auto' }
          : { width, height }
      }
    />
  );
}

/** `ease-in-out` (cubic-bezier(0.42, 0, 0.58, 1)) at `x`, as the transition layers use. */
function easeInOut(x: number): number {
  // Solve the bezier's x(t) = x by bisection, then return y(t).
  const bx = (t: number) => 3 * (1 - t) * (1 - t) * t * 0.42 + 3 * (1 - t) * t * t * 0.58 + t ** 3;
  const by = (t: number) => 3 * (1 - t) * t * t + t ** 3;
  let [lo, hi] = [0, 1];
  for (let i = 0; i < 24; i++) {
    const mid = (lo + hi) / 2;
    if (bx(mid) < x) lo = mid;
    else hi = mid;
  }
  return by((lo + hi) / 2);
}

/**
 * The outlines of a matched shape changing point by point during a morph ([MS-PPTX] p159:morph),
 * drawn in slide points over `duration` ms.
 */
export function MorphOutlineSvg({
  outline,
  duration,
  width,
  height,
  slide,
}: {
  outline: SlideMorphOutline;
  duration: number;
  width: number;
  height: number;
  slide: { width: number; height: number };
}) {
  // Without animation frames (or time) the outlines are drawn as they end.
  const animated = typeof requestAnimationFrame === 'function' && duration > 0;
  const [k, setK] = useState(animated ? 0 : 1);
  useEffect(() => {
    if (!animated) return;
    let raf = 0;
    const start = performance.now();
    const tick = (now: number) => {
      const x = Math.min(1, (now - start) / duration);
      setK(easeInOut(x));
      if (x < 1) raf = requestAnimationFrame(tick);
    };
    raf = requestAnimationFrame(tick);
    return () => cancelAnimationFrame(raf);
  }, [outline, duration, animated]);
  return (
    <svg
      className={styles.showCanvas}
      width={width}
      height={height}
      viewBox={`0 0 ${slide.width} ${slide.height}`}
      data-testid="morph-outline"
      data-progress={Math.round(k * 100)}
    >
      {outlineFrame(outline, k).map((f, i) => (
        <path
          key={i}
          d={f.d}
          fill={f.fill ?? 'none'}
          fillRule="evenodd"
          stroke={f.stroke ?? 'none'}
          strokeWidth={f.width}
          strokeLinejoin="round"
        />
      ))}
    </svg>
  );
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
  /** Show without animation: every effect of the slide shown at once (§19.2.1.30 showAnimation). */
  still?: boolean;
  /**
   * Run the action of a hot area (a click or the pointer coming over a shape or a
   * piece of linked text, SLIDE-ACTION-001); without it links are not followed.
   * Its sound and highlight are played here.
   */
  onLink?: (link: SlideShowLink) => void;
  /**
   * Play recorded narration and camera clips (§19.2.1.30 showNarration; SLIDE-REC-001); off:
   * they neither play nor show.
   */
  narration?: boolean;
}

/** Transition into the slide; a zoom (SLIDE-ZOOM-001) replaces the slide's own. */
function stagePlan(
  show: SlideShowInfo | null,
  slide: number,
  zoom: ZoomMove | undefined,
): TransitionPlan {
  if (zoom && show) {
    const f = zoomFrames(zoom, show.width, show.height);
    return {
      duration: zoom.ms,
      incoming: f.incoming,
      outgoing: f.outgoing,
      outgoingOnTop: zoom.out,
    };
  }
  return transitionPlan(show?.transition ?? null, slide);
}

/** The transition sound playing (§19.3.1.42 sndAc), shared by the show's slides. */
let transitionAudio: { el: HTMLAudioElement; url: string } | null = null;

function stopTransitionSound() {
  if (!transitionAudio) return;
  transitionAudio.el.pause();
  URL.revokeObjectURL(transitionAudio.url);
  transitionAudio = null;
}

/** Start (or stop) the sound of a transition. */
function playTransitionSound(engine: EngineHost, sound: SlideTransitionSound | null) {
  if (!sound) return;
  if (sound.stop || sound.part) stopTransitionSound();
  const part = sound.part;
  if (
    sound.stop ||
    !part ||
    typeof Audio === 'undefined' ||
    typeof URL.createObjectURL !== 'function'
  ) {
    return;
  }
  void engine
    .getMedia(part)
    .then((bytes) => {
      const copy = new Uint8Array(bytes.byteLength);
      copy.set(bytes);
      const url = URL.createObjectURL(new Blob([copy.buffer], { type: 'audio/wav' }));
      const el = new Audio(url);
      el.loop = sound.loop;
      transitionAudio = { el, url };
      void el.play()?.catch(() => {});
    })
    .catch(() => {});
}

/**
 * The sound of an action (§20.1.2.2.32 snd in hlinkClick / hlinkHover): the same
 * player as the transition sounds, so a new sound replaces the one playing and
 * `endSnd` stops it.
 */
export function playActionSound(engine: EngineHost, sound: SlideActionSound | undefined) {
  if (sound) playTransitionSound(engine, { ...sound, loop: false });
}

/** The sound of an animation effect (§19.5 sndTgt): played over whatever else plays. */
function playEffectSound(engine: EngineHost, part: string) {
  if (typeof Audio === 'undefined' || typeof URL.createObjectURL !== 'function') return;
  void engine
    .getMedia(part)
    .then((bytes) => {
      const copy = new Uint8Array(bytes.byteLength);
      copy.set(bytes);
      const url = URL.createObjectURL(new Blob([copy.buffer], { type: 'audio/wav' }));
      const el = new Audio(url);
      el.onended = () => URL.revokeObjectURL(url);
      void el.play()?.catch(() => {});
    })
    .catch(() => {});
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
  still = false,
  onLink,
  narration = true,
}: ShowStageProps) {
  const [show, setShow] = useState<SlideShowInfo | null>(null);
  /** Hot area flashing after its action ran (`highlightClick`). */
  const [flashed, setFlashed] = useState<string | null>(null);
  const flashTimer = useRef<ReturnType<typeof setTimeout> | null>(null);
  const flash = (id: string) => {
    if (flashTimer.current) clearTimeout(flashTimer.current);
    setFlashed(id);
    flashTimer.current = setTimeout(() => setFlashed(null), 220);
  };
  useEffect(
    () => () => {
      if (flashTimer.current) clearTimeout(flashTimer.current);
    },
    [],
  );
  /** Layer visibility and outgoing slide once the step started / ended (by step key). */
  const [phase, setPhase] = useState<{
    key: string;
    vis: boolean[];
    outgoing: boolean;
    tints: Record<number, boolean>;
    /** Pieces of layers cut into parts (`"layer:piece"`). */
    parts: Record<string, boolean>;
  } | null>(null);
  const layerRefs = useRef<(HTMLDivElement | null)[]>([]);
  /** Body (index 0) and letters (1…) of layers cut into text, by layer. */
  const pieceRefs = useRef<Record<number, (HTMLDivElement | null)[]>>({});
  /** Colour drawings, by effect index. */
  const tintRefs = useRef<Record<number, HTMLDivElement | null>>({});
  /** Body (index 0) and pieces of layers cut into parts, by layer. */
  const partRefs = useRef<Record<number, (HTMLDivElement | null)[]>>({});
  /** Effect sounds waiting for their effect to start. */
  const soundTimers = useRef<ReturnType<typeof setTimeout>[]>([]);
  const incomingRef = useRef<HTMLDivElement>(null);
  const outgoingRef = useRef<HTMLDivElement>(null);
  const morphRefs = useRef<(HTMLDivElement | null)[]>([]);
  const onIdleRef = useRef(onIdle);
  useEffect(() => {
    onIdleRef.current = onIdle;
  }, [onIdle]);
  /** Where effect sounds come from, and whether the stage plays sound at all. */
  const soundRef = useRef({ engine, media });
  useEffect(() => {
    soundRef.current = { engine, media };
  }, [engine, media]);
  const size = show ?? slide;
  const height = size.width > 0 ? Math.round((width * size.height) / size.width) : 0;
  const ready = show !== null && show.slide === pos.slide;
  const zoomKey = pos.zoom ? `${pos.zoom.out ? 'out' : 'in'}${pos.zoom.rect.join(',')}` : '';
  const key = `${pos.slide}:${pos.click}:${pos.enter}:${from ?? ''}:${zoomKey}`;
  const playing = !still && pos.enter !== 'jump';
  // Without animation the slide shows with every effect played.
  const atClick = still && show && show.slide === pos.slide ? show.clicks : pos.click;
  const hasOutgoing = pos.enter === 'transition' && from !== null && from !== pos.slide;
  // Before the step starts: the state before its effects (or after them, for a jump).
  const current =
    phase && phase.key === key
      ? phase
      : {
          key,
          vis: ready && show ? visibility(show, playing ? pos.click - 1 : atClick, playing) : [],
          outgoing: hasOutgoing,
          tints: ready && show ? tintsShown(show, playing ? pos.click - 1 : atClick) : {},
          parts:
            ready && show ? partVisibility(show, playing ? pos.click - 1 : atClick, playing) : {},
        };
  // Layers shown or hidden by trigger effects (§19.5 interactive sequences) on this step.
  const [triggered, setTriggered] = useState<{
    key: string;
    vis: Record<number, boolean>;
    parts: Record<string, boolean>;
  }>({
    key: '',
    vis: {},
    parts: {},
  });
  // Clips hidden while not playing (SLIDE-MEDIA-001), by `slide:shapeId`.
  const [hiddenMedia, setHiddenMedia] = useState<Record<string, boolean>>({});
  // Recorded clips left out when narrations are off.
  const muted = new Set(
    narration ? [] : (show?.media ?? []).filter((m) => m.recorded).map((m) => m.id),
  );
  const layerHidden = (i: number) => {
    const l = show?.layers[i];
    return (
      !!l &&
      !l.backdrop &&
      l.ids.length > 0 &&
      l.ids.every((id) => muted.has(id) || hiddenMedia[`${pos.slide}:${id}`] === true)
    );
  };
  const vis = current.vis.map(
    (v, i) => (triggered.key === key ? (triggered.vis[i] ?? v) : v) && !layerHidden(i),
  );
  // A clip playing across slides stops outside its span, and with the show.
  useEffect(() => {
    if (media) showMediaCarry.showSlide(pos.slide);
  }, [media, pos.slide]);
  useEffect(
    () => () => {
      if (media) showMediaCarry.stop();
    },
    [media],
  );
  const playTrigger = (shapeId: number, bookmark: string | null = null) => {
    if (!show) return;
    const fx = triggerEffects(show, shapeId, bookmark);
    const shown: Record<number, boolean> = {};
    const shownParts: Record<string, boolean> = {};
    for (const e of fx) {
      shown[e.layer] = true;
      for (const p of e.parts ?? []) shownParts[`${e.layer}:${p}`] = true;
      playEffect(e, 0, 'both');
    }
    setTriggered((t) => ({
      key,
      vis: { ...(t.key === key ? t.vis : {}), ...shown },
      parts: { ...(t.key === key ? t.parts : {}), ...shownParts },
    }));
    const end = fx.reduce(
      (m, e) =>
        Math.max(m, e.delay + e.duration * (Number.isFinite(e.iterations) ? e.iterations : 1)),
      0,
    );
    setTimeout(() => {
      setTriggered((t) => {
        if (t.key !== key) return t;
        const after: Record<number, boolean> = { ...t.vis };
        const parts: Record<string, boolean> = { ...t.parts };
        for (const e of fx) {
          if (e.visibleAfter === null) continue;
          if (e.parts) for (const p of e.parts) parts[`${e.layer}:${p}`] = e.visibleAfter;
          else after[e.layer] = e.visibleAfter;
        }
        return { key, vis: after, parts };
      });
    }, end + 30);
  };
  const outgoing = current.outgoing && from !== null ? from : null;

  /**
   * Start effect `e` `at` ms from now: on its layer, or on the layer's body and letters (word by
   * word / letter by letter), and its colour drawing. The animations started.
   */
  function playEffect(e: PlannedEffect, at: number, fill: FillMode): Animation[] {
    const opts = {
      delay: at + e.delay,
      duration: e.duration,
      iterations: e.iterations,
      easing: 'ease-out',
      fill,
    };
    const out: (Animation | null)[] = [];
    const parts = pieceRefs.current[e.layer];
    const pieces = partRefs.current[e.layer];
    if (e.sound && soundRef.current.media) {
      const part = e.sound;
      const host = soundRef.current.engine;
      soundTimers.current.push(setTimeout(() => playEffectSound(host, part), opts.delay));
    }
    if (e.parts && pieces) {
      for (const k of e.parts) out.push(animate(pieces[k], e.frames, opts));
    } else if (e.pieces && parts) {
      out.push(animate(parts[0], e.frames, opts));
      e.pieces.forEach((p, k) => {
        out.push(animate(parts[k + 1], p.frames, { ...opts, delay: opts.delay + p.delay }));
      });
    } else {
      out.push(animate(layerRefs.current[e.layer], e.frames, opts));
    }
    if (e.tint) {
      out.push(animate(tintRefs.current[e.index], e.tint, { ...opts, fill: 'both' }));
    }
    return out.filter((a): a is Animation => a !== null);
  }

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
    const endless: Animation[] = [];
    const start = visibility(show, playing ? pos.click - 1 : atClick, playing);
    const startParts = partVisibility(show, playing ? pos.click - 1 : atClick, playing);
    let end: ReturnType<typeof setTimeout> | undefined;
    const timer = setTimeout(() => {
      if (cancelled) return;
      let delay = 0;
      if (pos.enter === 'transition') {
        const t = stagePlan(show, pos.slide, pos.zoom);
        if (media) playTransitionSound(engine, show.transition?.sound ?? null);
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
          // Morph: the overlay's shapes move from the previous slide to this one.
          if (!pos.zoom && show.transition?.kind === 'morph' && show.morph) {
            morphLayers(show.morph, show.width, show.height).forEach((l, i) => {
              const a = animate(morphRefs.current[i], l.frames, opts);
              if (a) anims.push(a);
            });
          }
          delay = t.duration;
        }
      }
      let total = delay;
      if (playing) {
        const next = [...start];
        const nextParts = { ...startParts };
        const seen = new Set<string>();
        for (const e of effectsAt(show, pos.click)) {
          // The layer is shown while its effect runs; the first effect's frames
          // hide it until it starts, later ones on the same layer hold only after.
          next[e.layer] = true;
          const keys = e.parts ? e.parts.map((p) => `${e.layer}:${p}`) : [String(e.layer)];
          for (const k of keys) if (e.parts) nextParts[k] = true;
          const again = keys.some((k) => seen.has(k));
          const started = playEffect(e, delay, again ? 'forwards' : 'both');
          for (const k of keys) seen.add(k);
          if (Number.isFinite(e.iterations)) anims.push(...started);
          // "Until the next click": runs on; cancelled with the step.
          else endless.push(...started);
        }
        setPhase({
          key,
          vis: next,
          outgoing: hasOutgoing,
          tints: tintsShown(show, pos.click - 1),
          parts: nextParts,
        });
        total += stepDuration(show, pos.click);
      }
      const finish = () => {
        if (cancelled) return;
        for (const a of anims) a.cancel();
        // Endless effects keep running after the step (their layer stays shown).
        setPhase({
          key,
          vis: visibility(show, atClick),
          outgoing: false,
          tints: tintsShown(show, atClick),
          parts: partVisibility(show, atClick),
        });
        onIdleRef.current?.(show);
      };
      if (total > 0) end = setTimeout(finish, total + 30);
      else finish();
    }, 0);
    return () => {
      cancelled = true;
      clearTimeout(timer);
      clearTimeout(end);
      for (const t of soundTimers.current) clearTimeout(t);
      soundTimers.current = [];
      for (const a of anims) a.cancel();
      for (const a of endless) a.cancel();
    };
  }, [
    show,
    pos.slide,
    pos.click,
    pos.enter,
    pos.zoom,
    playing,
    hasOutgoing,
    key,
    media,
    engine,
    atClick,
  ]);

  return (
    <div
      className={styles.showStage}
      style={{ width, height }}
      data-testid="show-stage"
      data-slide={pos.slide}
      data-click={pos.click}
      data-clicks={show?.clicks ?? 0}
      data-transition={show?.transition?.kind ?? 'none'}
      data-zoom={pos.zoom ? (pos.zoom.out ? 'out' : 'in') : undefined}
      role="img"
      aria-label={label}
    >
      {outgoing !== null && (
        <div
          ref={outgoingRef}
          className={styles.showLayer}
          style={{
            zIndex: stagePlan(show, pos.slide, pos.zoom).outgoingOnTop ? 2 : 0,
          }}
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
      {outgoing !== null && show?.transition?.kind === 'morph' && show.morph && (
        <div className={styles.showLayer} style={{ zIndex: 3 }} data-testid="morph-overlay">
          {morphLayers(show.morph, show.width, show.height).map((l, i) => (
            <div
              key={i}
              ref={(el) => {
                morphRefs.current[i] = el;
              }}
              className={styles.showLayer}
            >
              {l.outline ? (
                <MorphOutlineSvg
                  outline={l.outline}
                  duration={stagePlan(show, pos.slide, pos.zoom).duration}
                  width={width}
                  height={height}
                  slide={{ width: show.width, height: show.height }}
                />
              ) : (
                <LayerCanvas
                  page={l.page}
                  rect={l.rect}
                  glyphs={glyphs}
                  images={images}
                  width={width}
                  height={height}
                  transparent={!l.backdrop}
                />
              )}
            </div>
          ))}
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
              {l.parts ? (
                l.parts.pages.map((pg, k) => {
                  const r = l.parts?.rects[k];
                  const pk = `${i}:${k}`;
                  const shown =
                    (triggered.key === key ? triggered.parts[pk] : undefined) ??
                    current.parts[pk] ??
                    true;
                  return (
                    <div
                      key={`part-${k}`}
                      ref={(el) => {
                        (partRefs.current[i] ??= [])[k] = el;
                      }}
                      className={styles.showLayer}
                      data-testid={k === 0 ? 'show-part-body' : 'show-part'}
                      data-part={k}
                      data-visible={shown}
                      style={{ visibility: shown ? 'visible' : 'hidden' }}
                    >
                      {r && (
                        <LayerCanvas
                          page={pg}
                          rect={r}
                          glyphs={glyphs}
                          images={images}
                          width={width}
                          height={height}
                          transparent
                        />
                      )}
                    </div>
                  );
                })
              ) : l.text ? (
                <>
                  <div
                    ref={(el) => {
                      (pieceRefs.current[i] ??= [])[0] = el;
                    }}
                    className={styles.showLayer}
                    data-testid="show-layer-body"
                  >
                    <LayerCanvas
                      page={l.text.body}
                      glyphs={glyphs}
                      images={images}
                      width={width}
                      height={height}
                      transparent={!l.backdrop}
                    />
                  </div>
                  {l.text.pieces.map((p, k) => (
                    <div
                      key={k}
                      ref={(el) => {
                        (pieceRefs.current[i] ??= [])[k + 1] = el;
                      }}
                      className={styles.showLayer}
                      data-testid="show-piece"
                    >
                      <LayerCanvas
                        page={p.page}
                        rect={p.rect}
                        glyphs={glyphs}
                        images={images}
                        width={width}
                        height={height}
                        transparent
                      />
                    </div>
                  ))}
                </>
              ) : (
                <LayerCanvas
                  page={l.page}
                  glyphs={glyphs}
                  images={images}
                  width={width}
                  height={height}
                  transparent={!l.backdrop}
                />
              )}
              {show.animations.map((a, k) =>
                a.tint && a.layer === i ? (
                  <div
                    key={`tint-${k}`}
                    ref={(el) => {
                      tintRefs.current[k] = el;
                    }}
                    className={styles.showLayer}
                    data-testid="show-tint"
                    data-shown={current.tints[k] === true}
                    style={{ opacity: current.tints[k] === true ? 1 : 0 }}
                  >
                    <LayerCanvas
                      page={a.tint}
                      glyphs={glyphs}
                      images={images}
                      width={width}
                      height={height}
                      transparent
                    />
                  </div>
                ) : null,
              )}
            </div>
          ))}
        {ready &&
          media &&
          show.media
            .filter((m) => !muted.has(m.id))
            .map((m) => (
              <ShowMediaClip
                key={`${pos.slide}-${m.id}`}
                engine={engine}
                clip={m}
                scale={width / (show.width || 1)}
                slide={pos.slide}
                click={pos.click}
                stage={{ width, height }}
                labels={mediaLabels}
                onBookmark={(id, name) => playTrigger(id, name)}
                onHidden={(id, hidden) =>
                  setHiddenMedia((h) =>
                    h[`${pos.slide}:${id}`] === hidden
                      ? h
                      : { ...h, [`${pos.slide}:${id}`]: hidden },
                  )
                }
              />
            ))}
      </div>
      {ready &&
        media &&
        (show.triggers ?? []).map((tr) => {
          const k = width / (show.width || 1);
          const [x, y, w, h] = tr.rect;
          return (
            <button
              key={tr.shapeId}
              type="button"
              className={styles.showTrigger}
              data-testid={`show-trigger-${tr.shapeId}`}
              style={{ left: x * k, top: y * k, width: w * k, height: h * k }}
              onClick={(e) => {
                // A trigger plays its effects instead of advancing the show.
                e.stopPropagation();
                playTrigger(tr.shapeId);
              }}
            />
          );
        })}
      {ready &&
        media &&
        onLink &&
        hotAreas(show.links ?? []).map((a) => {
          const k = width / (show.width || 1);
          const [x, y, w, h] = a.rect;
          const act = (l: SlideShowLink) => {
            if (l.highlight) flash(a.id);
            playActionSound(engine, l.sound);
            onLink(l);
          };
          return (
            <button
              key={`link-${a.id}`}
              type="button"
              className={`${styles.showTrigger} ${flashed === a.id ? actionStyles.flash : ''}`}
              data-testid={`show-link-${a.id}`}
              data-action={(a.click ?? a.hover)?.action.kind}
              data-hover={a.hover?.action.kind}
              title={a.title}
              // A box that only reacts to hovering lets clicks advance the show.
              style={{
                left: x * k,
                top: y * k,
                width: w * k,
                height: h * k,
                cursor: a.click ? 'pointer' : 'default',
              }}
              onMouseEnter={a.hover ? () => act(a.hover!) : undefined}
              onClick={
                a.click
                  ? (e) => {
                      // A hyperlink goes where it leads instead of advancing the show.
                      e.stopPropagation();
                      act(a.click!);
                    }
                  : undefined
              }
            />
          );
        })}
    </div>
  );
}
