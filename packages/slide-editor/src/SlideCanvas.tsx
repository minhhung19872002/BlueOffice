import { GlyphStore, ImageStore, renderPage } from '@blueoffice/canvas-host';
import type { EngineHost } from '@blueoffice/platform';
import { useEffect, useRef } from 'react';

export interface SlideCanvasProps {
  engine: EngineHost;
  glyphs: GlyphStore;
  images: ImageStore;
  index: number;
  /** Slide size in points. */
  slide: { width: number; height: number };
  /** Displayed width in CSS pixels. */
  width: number;
  /** Bumped when the presentation changes (forces a redraw). */
  revision?: number;
  className?: string;
  label?: string;
}

/** One slide drawn from its engine display list. */
export function SlideCanvas({
  engine,
  glyphs,
  images,
  index,
  slide,
  width,
  revision = 0,
  className,
  label,
}: SlideCanvasProps) {
  const ref = useRef<HTMLCanvasElement>(null);
  const height = slide.width > 0 ? Math.round((width * slide.height) / slide.width) : 0;

  useEffect(() => {
    let cancelled = false;
    void (async () => {
      if (width <= 0 || height <= 0) return;
      const page = await engine.getPage(index);
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
  }, [engine, glyphs, images, index, width, height, revision]);

  return (
    <canvas
      ref={ref}
      className={className}
      style={{ width, height }}
      role="img"
      aria-label={label}
    />
  );
}
