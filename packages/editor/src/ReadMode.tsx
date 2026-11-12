import type { PageSize } from '@blueoffice/platform';
import { useTranslation } from '@blueoffice/i18n';
import { useEffect, useLayoutEffect, useRef, useState } from 'react';
import styles from './Views.module.css';

export interface ReadModeProps {
  /** Document name shown in the bar. */
  name: string;
  /** Sizes of the laid-out pages (points). */
  pages: PageSize[];
  /** Page to start on (0-based). */
  start: number;
  /** Draw page `index` into `canvas`, `width` CSS pixels wide. */
  renderPage: (index: number, canvas: HTMLCanvasElement, width: number) => Promise<void>;
  /** Leave read mode on a page. */
  onExit: (page: number) => void;
  /**
   * Reflow the text into column pages `width` × `height` points (the engine's read
   * layout); resolves to the page of the caret in that layout. Absent: the printed
   * pages are shown as they are.
   */
  reflow?: (width: number, height: number) => Promise<number | null>;
}

/** Room around the pages (CSS px). */
const MARGIN = 24;
const GAP = 24;

/** Pages per screen: two side by side when they fit at a readable size. */
export function spreadSize(width: number, height: number, page: PageSize | undefined): 1 | 2 {
  if (!page) return 1;
  const h = height - 2 * MARGIN;
  const k = h / page.height;
  return 2 * page.width * k + GAP + 2 * MARGIN <= width && width > 900 ? 2 : 1;
}

/** Widest reading column (CSS px): longer lines are hard to read. */
const COLUMN_MAX = 720;
/** CSS pixels per point at 100 %. */
const PX_PER_PT = 96 / 72;

/**
 * Read mode column page (points) for a screen `width` × `height` CSS px: two columns
 * side by side on wide screens, each at most {@link COLUMN_MAX} wide, as tall as the
 * screen — text shows at 100 %.
 */
export function readColumn(width: number, height: number): { width: number; height: number } {
  const per = width > 900 ? 2 : 1;
  const col = Math.min(COLUMN_MAX, (width - 2 * MARGIN - (per - 1) * GAP) / per);
  return {
    width: Math.floor(Math.max(200, col) / PX_PER_PT),
    height: Math.floor(Math.max(200, height - 2 * MARGIN) / PX_PER_PT),
  };
}

/** First page of the screen holding `page`. */
export const spreadStart = (page: number, per: 1 | 2): number => page - (page % per);

/** First page of the screen `delta` screens away (clamped to the document). */
const stepTo = (page: number, delta: number, per: 1 | 2, total: number): number =>
  Math.min(
    spreadStart(Math.max(0, total - 1), per),
    Math.max(0, spreadStart(page, per) + delta * per),
  );

/**
 * Read mode: the document full window, one or two pages per screen, turned with
 * ←/→, PageUp/PageDown, Space, Home/End; Esc leaves.
 */
export function ReadMode({ name, pages, start, renderPage, onExit, reflow }: ReadModeProps) {
  const { t } = useTranslation();
  const host = useRef<HTMLDivElement>(null);
  const root = useRef<HTMLDivElement>(null);
  // Keys go to read mode, not to the document's text input.
  useEffect(() => root.current?.focus(), []);
  const [size, setSize] = useState({ w: 1200, h: 800 });
  const per = spreadSize(size.w, size.h, pages[0]);
  const [shownFrom, setFirst] = useState(start);
  // The screen starts on a page aligned to the spread size.
  const total = pages.length;
  // Fewer pages after a reflow: stay inside the document.
  const first = spreadStart(Math.min(shownFrom, Math.max(0, total - 1)), per);
  const shown = Array.from({ length: per }, (_, i) => first + i).filter((i) => i < total);

  useLayoutEffect(() => {
    const el = host.current;
    if (!el) return;
    const measure = () => setSize({ w: el.clientWidth || 1200, h: el.clientHeight || 800 });
    measure();
    window.addEventListener('resize', measure);
    return () => window.removeEventListener('resize', measure);
  }, []);

  // The text reflows to the screen's columns (again when the window is resized),
  // opening on the caret's page.
  const col = readColumn(size.w, size.h);
  useEffect(() => {
    if (!reflow) return;
    let live = true;
    void reflow(col.width, col.height).then((page) => {
      if (live && page !== null) setFirst(page);
    });
    return () => {
      live = false;
    };
  }, [reflow, col.width, col.height]);

  const go = (delta: number) => setFirst((raw) => stepTo(raw, delta, per, total));
  useEffect(() => {
    const go = (delta: number) => setFirst((raw) => stepTo(raw, delta, per, total));
    const onKey = (e: KeyboardEvent) => {
      switch (e.key) {
        case 'Escape':
          onExit(first);
          break;
        case 'ArrowRight':
        case 'ArrowDown':
        case 'PageDown':
        case ' ':
          go(1);
          break;
        case 'ArrowLeft':
        case 'ArrowUp':
        case 'PageUp':
          go(-1);
          break;
        case 'Home':
          setFirst(0);
          break;
        case 'End':
          setFirst(spreadStart(Math.max(0, total - 1), per));
          break;
        default:
          return;
      }
      e.preventDefault();
      e.stopPropagation();
    };
    window.addEventListener('keydown', onKey, true);
    return () => window.removeEventListener('keydown', onKey, true);
  }, [first, onExit, per, total]);

  const last = shown[shown.length - 1] ?? first;
  return (
    <div
      ref={root}
      tabIndex={-1}
      className={styles.read}
      role="dialog"
      aria-label={t('views.read.title')}
      data-testid="read-mode"
    >
      <div className={styles.readBar}>
        <strong>{name}</strong>
        <span data-testid="read-position" aria-live="polite">
          {shown.length > 1
            ? t('views.read.pages', { from: first + 1, to: last + 1, total })
            : t('views.read.page', { n: first + 1, total })}
        </span>
        <span className={styles.spacer} />
        <button
          type="button"
          onClick={() => go(-1)}
          disabled={first === 0}
          aria-label={t('views.read.previous')}
          data-testid="read-previous"
        >
          ‹
        </button>
        <button
          type="button"
          onClick={() => go(1)}
          disabled={last >= total - 1}
          aria-label={t('views.read.next')}
          data-testid="read-next"
        >
          ›
        </button>
        <button type="button" onClick={() => onExit(first)} data-testid="read-exit">
          {t('views.read.exit')}
        </button>
      </div>
      <div ref={host} className={styles.readPages}>
        {shown.map((i) => (
          <ReadPage
            key={i}
            index={i}
            page={pages[i]}
            height={size.h - 2 * MARGIN}
            maxWidth={(size.w - 2 * MARGIN - (per - 1) * GAP) / per}
            render={renderPage}
          />
        ))}
      </div>
    </div>
  );
}

function ReadPage({
  index,
  page,
  height,
  maxWidth,
  render,
}: {
  index: number;
  page: PageSize | undefined;
  height: number;
  maxWidth: number;
  render: ReadModeProps['renderPage'];
}) {
  const ref = useRef<HTMLCanvasElement>(null);
  const k = page ? Math.min(height / page.height, maxWidth / page.width) : 1;
  const width = Math.max(50, Math.round((page?.width ?? 595) * k));
  useEffect(() => {
    if (ref.current) void render(index, ref.current, width).catch(() => {});
  }, [index, width, render]);
  return (
    <canvas ref={ref} className={styles.readPage} data-testid="read-page" data-page={index + 1} />
  );
}
