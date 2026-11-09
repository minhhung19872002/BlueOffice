import { useTranslation } from '@blueoffice/i18n';
import type { SlideInkStroke } from '@blueoffice/platform';
import { Dialog } from '@blueoffice/ui';
import {
  Crosshair,
  Eraser,
  Highlighter,
  LayoutGrid,
  MousePointer2,
  PenLine,
  Square,
  Trash2,
  ZoomIn,
} from 'lucide-react';
import { useRef, useState, type ReactNode, type PointerEvent as ReactPointerEvent } from 'react';
import { HIGHLIGHTER_COLOR, PEN_COLOR, strokePath, type InkLayer, type PresenterTool } from './ink';
import {
  NO_ZOOM,
  dragBox,
  unzoom,
  zoomAt,
  zoomTransform,
  type Box,
  type ScreenCover,
  type ZoomTransform,
} from './presenting';
import { formatElapsed } from './show';
import styles from './showTools.module.css';

/** CSS transform of a zoom in stage px. */
export function zoomCss(z: ZoomTransform): string {
  return `translate(${z.x}px, ${z.y}px) scale(${z.scale})`;
}

/** Ink strokes of one slide drawn over it (viewBox in slide points). */
export function InkStrokes({
  strokes,
  slide,
}: {
  strokes: SlideInkStroke[];
  slide: { width: number; height: number };
}) {
  return (
    <svg
      className={styles.ink}
      viewBox={`0 0 ${slide.width} ${slide.height}`}
      preserveAspectRatio="none"
      data-testid="show-ink"
      data-strokes={strokes.length}
      aria-hidden="true"
    >
      {strokes.map((s, i) => (
        <path
          key={i}
          d={strokePath(s)}
          fill="none"
          stroke={`#${s.color}`}
          strokeOpacity={s.highlighter ? 0.5 : 1}
          strokeWidth={s.width}
          strokeLinecap="round"
          strokeLinejoin="round"
        />
      ))}
    </svg>
  );
}

export interface ShowSurfaceProps {
  /** The slide (ShowStage) at `width` × `height` CSS px. */
  children: ReactNode;
  width: number;
  height: number;
  /** Slide size in points. */
  slide: { width: number; height: number };
  /** Index of the slide shown (ink belongs to it). */
  index: number;
  tool: PresenterTool;
  onTool: (t: PresenterTool) => void;
  ink: InkLayer;
  /** Bump to redraw after ink edits. */
  onInk: () => void;
  /** Zoom with `x`, `y` as fractions of the stage size (same on every screen size). */
  zoom: ZoomTransform;
  onZoom: (z: ZoomTransform) => void;
  cover: ScreenCover;
  onCover: (c: 'black' | 'white') => void;
  onGrid: () => void;
  /** `penClr` of the show setup (`RRGGBB`). */
  penColor?: string | null;
  /** Audience screen: the presenter's ink, zoom and cover, no toolbar. */
  readOnly?: boolean;
  /** Ink strokes to show in read-only mode. */
  strokes?: SlideInkStroke[];
}

type Pt = { x: number; y: number };

/**
 * The slide with the presenter's tools over it: ink (pen, highlighter, eraser),
 * laser pointer, zoom into a dragged area or around a click, black / white
 * screen, and a small toolbar.
 */
export function ShowSurface({
  children,
  width,
  height,
  slide,
  index,
  tool,
  onTool,
  ink,
  onInk,
  zoom,
  onZoom,
  cover,
  onCover,
  onGrid,
  penColor,
  readOnly = false,
  strokes,
}: ShowSurfaceProps) {
  const { t } = useTranslation();
  const [laser, setLaser] = useState<Pt | null>(null);
  const [band, setBand] = useState<Box | null>(null);
  const start = useRef<Pt | null>(null);
  const zoomed = zoom.scale > 1;
  const zpx: ZoomTransform = { scale: zoom.scale, x: zoom.x * width, y: zoom.y * height };
  const setZoomPx = (z: ZoomTransform) =>
    onZoom({ scale: z.scale, x: width > 0 ? z.x / width : 0, y: height > 0 ? z.y / height : 0 });
  const capturing = !readOnly && (tool !== 'pointer' || zoomed);
  // Slide points per CSS px without zoom.
  const k = width > 0 ? slide.width / width : 1;
  const local = (e: ReactPointerEvent<HTMLElement>): Pt => {
    const r = e.currentTarget.getBoundingClientRect();
    return { x: e.clientX - r.left, y: e.clientY - r.top };
  };
  const toSlide = (p: Pt): [number, number] => {
    const s = unzoom(p, zpx, k);
    return [s.x, s.y];
  };

  const onDown = (e: ReactPointerEvent<HTMLDivElement>) => {
    e.stopPropagation();
    const p = local(e);
    e.currentTarget.setPointerCapture?.(e.pointerId);
    if (tool === 'pen' || tool === 'highlighter') {
      const color = tool === 'pen' ? (penColor ?? PEN_COLOR) : HIGHLIGHTER_COLOR;
      ink.begin(index, tool, color, toSlide(p));
      onInk();
    } else if (tool === 'eraser') {
      if (ink.erase(index, toSlide(p), 6 * k)) onInk();
      start.current = p;
    } else if (tool === 'zoom') {
      start.current = p;
      setBand({ x: p.x, y: p.y, w: 0, h: 0 });
    } else if (zoomed) {
      start.current = p;
    }
  };
  const onMove = (e: ReactPointerEvent<HTMLDivElement>) => {
    const p = local(e);
    if (tool === 'laser') {
      setLaser(p);
      return;
    }
    if (ink.drawing) {
      ink.extend(toSlide(p));
      onInk();
    } else if (tool === 'eraser' && start.current) {
      if (ink.erase(index, toSlide(p), 6 * k)) onInk();
    } else if (tool === 'zoom' && start.current) {
      setBand(dragBox(start.current, p));
    }
  };
  const onUp = (e: ReactPointerEvent<HTMLDivElement>) => {
    e.stopPropagation();
    const p = local(e);
    if (ink.drawing) {
      ink.end();
      onInk();
    } else if (tool === 'zoom' && start.current) {
      const b = dragBox(start.current, p);
      // A click (or a tiny drag) zooms ×2 around the point.
      const z =
        b.w < 8 || b.h < 8 ? zoomAt(p, 2, { width, height }) : zoomTransform(b, { width, height });
      setZoomPx(z);
      onTool('pointer');
    } else if (zoomed && tool === 'pointer' && start.current) {
      onZoom(NO_ZOOM);
    }
    start.current = null;
    setBand(null);
  };

  const shown = readOnly ? (strokes ?? []) : ink.strokes(index);
  const button = (id: PresenterTool, label: string, icon: ReactNode) => (
    <button
      type="button"
      data-testid={`show-tool-${id}`}
      aria-label={label}
      title={label}
      aria-pressed={tool === id}
      onClick={() => onTool(tool === id ? 'pointer' : id)}
    >
      {icon}
    </button>
  );
  const captureClass =
    tool === 'pen' || tool === 'highlighter' || tool === 'eraser'
      ? styles.capturePen
      : tool === 'laser'
        ? styles.captureLaser
        : tool === 'zoom'
          ? styles.captureZoom
          : styles.captureZoomOut;

  return (
    <div
      className={styles.surface}
      style={{ width, height }}
      data-testid="show-surface"
      data-tool={tool}
      data-zoom={zoom.scale.toFixed(2)}
      data-cover={cover}
    >
      <div className={styles.zoomed} style={{ transform: zoomCss(zpx) }}>
        {children}
        <InkStrokes strokes={shown} slide={slide} />
      </div>
      {capturing && (
        <div
          className={`${styles.capture} ${captureClass}`}
          data-testid="show-capture"
          onPointerDown={onDown}
          onPointerMove={onMove}
          onPointerUp={onUp}
          onPointerLeave={() => setLaser(null)}
          onClick={(e) => e.stopPropagation()}
          onContextMenu={(e) => e.stopPropagation()}
        />
      )}
      {tool === 'laser' && laser && !readOnly && (
        <div
          className={styles.laser}
          data-testid="show-laser"
          style={{ left: laser.x, top: laser.y }}
        />
      )}
      {band && (
        <div
          className={styles.band}
          data-testid="show-zoom-band"
          style={{ left: band.x, top: band.y, width: band.w, height: band.h }}
        />
      )}
      {cover !== 'none' && (
        <div
          className={`${styles.cover} ${cover === 'black' ? styles.coverBlack : styles.coverWhite}`}
          data-testid="show-cover"
        />
      )}
      {!readOnly && (
        <div
          className={styles.toolbar}
          data-testid="show-toolbar"
          role="toolbar"
          aria-label={t('showTools.toolbar')}
          onClick={(e) => e.stopPropagation()}
          onContextMenu={(e) => e.stopPropagation()}
        >
          {button('pointer', t('showTools.pointer'), <MousePointer2 size={16} />)}
          {button('pen', t('showTools.pen'), <PenLine size={16} />)}
          {button('highlighter', t('showTools.highlighter'), <Highlighter size={16} />)}
          {button('laser', t('showTools.laser'), <Crosshair size={16} />)}
          {button('eraser', t('showTools.eraser'), <Eraser size={16} />)}
          <button
            type="button"
            data-testid="show-ink-clear"
            aria-label={t('showTools.clearInk')}
            title={t('showTools.clearInk')}
            onClick={() => {
              ink.clear(index);
              onInk();
            }}
          >
            <Trash2 size={16} />
          </button>
          {button('zoom', t('showTools.zoom'), <ZoomIn size={16} />)}
          <button
            type="button"
            data-testid="show-grid-open"
            aria-label={t('showTools.grid')}
            title={t('showTools.grid')}
            onClick={onGrid}
          >
            <LayoutGrid size={16} />
          </button>
          <button
            type="button"
            data-testid="show-black"
            aria-label={t('showTools.black')}
            title={t('showTools.black')}
            aria-pressed={cover === 'black'}
            onClick={() => onCover('black')}
          >
            <Square size={16} fill="#000" />
          </button>
          <button
            type="button"
            data-testid="show-white"
            aria-label={t('showTools.white')}
            title={t('showTools.white')}
            aria-pressed={cover === 'white'}
            onClick={() => onCover('white')}
          >
            <Square size={16} fill="#fff" />
          </button>
        </div>
      )}
    </div>
  );
}

/** Go-to-slide grid: the slides of the playing order as thumbnails. */
export function SlideGrid({
  order,
  at,
  thumb,
  onPick,
  onClose,
}: {
  order: number[];
  /** Current place in the order. */
  at: number;
  thumb: (slide: number, width: number) => ReactNode;
  onPick: (at: number) => void;
  onClose: () => void;
}) {
  const { t } = useTranslation();
  return (
    <div
      className={styles.grid}
      data-testid="show-grid"
      role="dialog"
      aria-label={t('showTools.grid')}
      onClick={(e) => e.stopPropagation()}
      onContextMenu={(e) => e.stopPropagation()}
    >
      <div className={styles.gridHead}>
        <strong>{t('showTools.grid')}</strong>
        <button type="button" data-testid="show-grid-close" onClick={onClose}>
          {t('showTools.close')}
        </button>
      </div>
      <div className={styles.gridItems}>
        {order.map((s, i) => (
          <button
            key={`${i}-${s}`}
            type="button"
            className={styles.gridItem}
            data-testid={`show-grid-item-${i}`}
            data-slide={s}
            aria-current={i === at}
            onClick={() => onPick(i)}
          >
            {thumb(s, 200)}
            <span>{t('showTools.slideN', { n: s + 1 })}</span>
          </button>
        ))}
      </div>
    </div>
  );
}

/** Rehearsal timer over the show: time on this slide, total, pause, repeat, next. */
export function RehearsalBar({
  slideMs,
  totalMs,
  paused,
  onNext,
  onPause,
  onRepeat,
}: {
  slideMs: number;
  totalMs: number;
  paused: boolean;
  onNext: () => void;
  onPause: () => void;
  onRepeat: () => void;
}) {
  const { t } = useTranslation();
  return (
    <div
      className={styles.rehearsal}
      data-testid="rehearsal-bar"
      role="timer"
      aria-label={t('showTools.rehearsing')}
      onClick={(e) => e.stopPropagation()}
      onContextMenu={(e) => e.stopPropagation()}
    >
      <button type="button" data-testid="rehearsal-next" onClick={onNext}>
        {t('showTools.next')}
      </button>
      <button type="button" data-testid="rehearsal-pause" aria-pressed={paused} onClick={onPause}>
        {paused ? t('showTools.resume') : t('showTools.pause')}
      </button>
      <span data-testid="rehearsal-slide-time" title={t('showTools.slideTime')}>
        {formatElapsed(slideMs)}
      </span>
      <button type="button" data-testid="rehearsal-repeat" onClick={onRepeat}>
        {t('showTools.repeat')}
      </button>
      <span data-testid="rehearsal-total" title={t('showTools.totalTime')}>
        {formatElapsed(totalMs)}
      </span>
    </div>
  );
}

export interface EndShowState {
  /** Ink was drawn: offer to keep it. */
  ink: boolean;
  /** Rehearsed timings to keep, if the show was a rehearsal. */
  timings: { slide: number; ms: number }[] | null;
}

/** At the end of a show: keep the ink as shapes, keep the rehearsed timings. */
export function EndShowDialog({
  state,
  onDone,
}: {
  state: EndShowState | null;
  onDone: (keep: { ink: boolean; timings: boolean }) => void;
}) {
  const { t } = useTranslation();
  const [ink, setInk] = useState(true);
  const [timings, setTimings] = useState(true);
  const total = (state?.timings ?? []).reduce((s, x) => s + x.ms, 0);
  return (
    <Dialog
      open={state !== null}
      onOpenChange={(o) => !o && onDone({ ink: false, timings: false })}
      title={t('showTools.endTitle')}
      testId="end-show-dialog"
      footer={
        <>
          <button
            type="button"
            data-testid="end-show-discard"
            onClick={() => onDone({ ink: false, timings: false })}
          >
            {t('showTools.discard')}
          </button>
          <button type="button" data-testid="end-show-ok" onClick={() => onDone({ ink, timings })}>
            {t('showTools.keep')}
          </button>
        </>
      }
    >
      <fieldset className={styles.form}>
        {state?.ink && (
          <label className={styles.row}>
            <input
              type="checkbox"
              data-testid="end-keep-ink"
              checked={ink}
              onChange={(e) => setInk(e.target.checked)}
            />
            {t('showTools.keepInk')}
          </label>
        )}
        {state?.timings && (
          <label className={styles.row}>
            <input
              type="checkbox"
              data-testid="end-keep-timings"
              checked={timings}
              onChange={(e) => setTimings(e.target.checked)}
            />
            {t('showTools.keepTimings', { total: formatElapsed(total) })}
          </label>
        )}
      </fieldset>
    </Dialog>
  );
}
