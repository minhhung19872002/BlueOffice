/**
 * Ink replay (CORE-INK-001): the slide without its ink (engine backdrop), then
 * every stroke drawn again in time order — recorded InkML times (`T`) within a
 * stroke, short pauses between strokes.
 */
import { renderPage, type GlyphStore, type ImageStore } from '@blueoffice/canvas-host';
import type { PageDisplayList, SlideInkObject } from '@blueoffice/platform';
import { useEffect, useMemo, useRef, useState } from 'react';
import styles from './draw.module.css';
import { replayTimeline, visibleAt, type ReplayStroke } from './inkTools';

export interface InkReplayProps {
  backdrop: PageDisplayList;
  objects: SlideInkObject[];
  glyphs: GlyphStore;
  images: ImageStore;
  /** Displayed width in CSS pixels. */
  width: number;
  onDone: () => void;
  labels: { region: string; stop: string; again: string; progress: string };
}

/** Draw the visible part of each stroke (slide points, context already scaled). */
export function drawReplay(
  ctx: CanvasRenderingContext2D,
  strokes: ReplayStroke[],
  counts: number[],
): void {
  strokes.forEach((r, i) => {
    const n = counts[i] ?? 0;
    if (n === 0) return;
    const s = r.stroke;
    const pts = s.points.slice(0, n);
    ctx.save();
    ctx.globalAlpha = Math.max(0, Math.min(1, s.opacity));
    ctx.strokeStyle = `#${s.color}`;
    ctx.fillStyle = `#${s.color}`;
    ctx.lineCap = s.rectTip ? 'square' : 'round';
    ctx.lineJoin = 'round';
    if (pts.length === 1) {
      const p = pts[0]!;
      ctx.beginPath();
      ctx.arc(p[0], p[1], p[2] / 2, 0, Math.PI * 2);
      ctx.fill();
    } else if (s.opacity < 1) {
      // Translucent ink: one path, so overlapping segments do not darken.
      ctx.lineWidth = pts.reduce((a, p) => a + p[2], 0) / pts.length;
      ctx.beginPath();
      ctx.moveTo(pts[0]![0], pts[0]![1]);
      for (const p of pts.slice(1)) ctx.lineTo(p[0], p[1]);
      ctx.stroke();
    } else {
      // Opaque ink: segment by segment, following the pressure width.
      for (let k = 1; k < pts.length; k++) {
        const a = pts[k - 1]!;
        const b = pts[k]!;
        ctx.lineWidth = (a[2] + b[2]) / 2;
        ctx.beginPath();
        ctx.moveTo(a[0], a[1]);
        ctx.lineTo(b[0], b[1]);
        ctx.stroke();
      }
    }
    ctx.restore();
  });
}

export function InkReplay({
  backdrop,
  objects,
  glyphs,
  images,
  width,
  onDone,
  labels,
}: InkReplayProps) {
  const base = useRef<HTMLCanvasElement>(null);
  const top = useRef<HTMLCanvasElement>(null);
  const timeline = useMemo(() => replayTimeline(objects), [objects]);
  const [run, setRun] = useState(0);
  const [progress, setProgress] = useState(0);
  const height = backdrop.width > 0 ? Math.round((width * backdrop.height) / backdrop.width) : 0;
  const dpr = typeof window !== 'undefined' ? window.devicePixelRatio || 1 : 1;
  const scale = (width / (backdrop.width || 1)) * dpr;

  // The slide without ink, drawn once.
  useEffect(() => {
    let cancelled = false;
    void (async () => {
      await Promise.all([glyphs.ensure(backdrop), images.ensure(backdrop)]);
      const c = base.current;
      const ctx = c?.getContext('2d');
      if (cancelled || !c || !ctx) return;
      c.width = Math.round(width * dpr);
      c.height = Math.round(height * dpr);
      renderPage(ctx, backdrop, glyphs, scale, images);
    })().catch(() => {});
    return () => {
      cancelled = true;
    };
  }, [backdrop, glyphs, images, width, height, dpr, scale]);

  // The strokes, frame by frame.
  useEffect(() => {
    const c = top.current;
    const ctx = c?.getContext('2d');
    if (!c || !ctx || typeof requestAnimationFrame === 'undefined') return;
    c.width = Math.round(width * dpr);
    c.height = Math.round(height * dpr);
    let raf = 0;
    let t0: number | null = null;
    const frame = (now: number) => {
      t0 ??= now;
      const t = now - t0;
      ctx.setTransform(1, 0, 0, 1, 0, 0);
      ctx.clearRect(0, 0, c.width, c.height);
      ctx.setTransform(scale, 0, 0, scale, 0, 0);
      drawReplay(ctx, timeline.strokes, visibleAt(timeline.strokes, t));
      setProgress(timeline.total > 0 ? Math.min(1, t / timeline.total) : 1);
      if (t < timeline.total) raf = requestAnimationFrame(frame);
    };
    raf = requestAnimationFrame(frame);
    return () => cancelAnimationFrame(raf);
  }, [timeline, width, height, dpr, scale, run]);

  useEffect(() => {
    const key = (e: KeyboardEvent) => {
      if (e.key === 'Escape') onDone();
    };
    window.addEventListener('keydown', key);
    return () => window.removeEventListener('keydown', key);
  }, [onDone]);

  return (
    <div
      className={styles.replay}
      role="region"
      aria-label={labels.region}
      data-testid="ink-replay"
    >
      <canvas ref={base} className={styles.replayCanvas} style={{ width, height }} />
      <canvas ref={top} className={styles.replayCanvas} style={{ width, height }} />
      <div className={styles.replayBar}>
        <progress
          className={styles.replayProgress}
          max={1}
          value={progress}
          aria-label={labels.progress}
        />
        <button type="button" onClick={() => setRun((n) => n + 1)}>
          {labels.again}
        </button>
        <button type="button" onClick={onDone} data-testid="ink-replay-stop">
          {labels.stop}
        </button>
      </div>
    </div>
  );
}
