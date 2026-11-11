/**
 * Screen-reader access to BlueSlide (WCAG 2.2 1.3.1, 4.1.2): slides are pixels on canvases, so
 * the engine's `slideOutline` (shapes in reading order — title first — with their text and the
 * alternative text of pictures, charts, SmartArt diagrams…) names and describes each thumbnail
 * and fills a visually hidden copy of the slide on the stage; a polite live region announces the
 * selected shape.
 */
import { useEffect, useState, type CSSProperties } from 'react';
import { useTranslation } from '@blueoffice/i18n';
import type {
  EngineHost,
  SlideA11yShape,
  SlideOutline,
  SlideShapeInfo,
} from '@blueoffice/platform';

/** Visually hidden but in the accessibility tree (not `display: none`). */
export const HIDDEN: CSSProperties = {
  position: 'absolute',
  left: 0,
  top: 0,
  width: 1,
  height: 1,
  margin: -1,
  padding: 0,
  border: 0,
  overflow: 'hidden',
  clipPath: 'inset(50%)',
  whiteSpace: 'nowrap',
  pointerEvents: 'none',
};

/** Most slides described at once. */
export const MAX_OUTLINE_SLIDES = 1000;

type Translate = (key: string, opts?: Record<string, unknown>) => string;

const isTitle = (s: SlideA11yShape) => s.placeholder === 'title' || s.placeholder === 'ctrTitle';
/** Slide text with its paragraph / line breaks as spaces. */
const flat = (t: string) => t.replaceAll('\u000b', ' ').replace(/\s+/g, ' ').trim();

/** The title of a slide: its title placeholder's text. */
export function slideTitle(shapes: readonly SlideA11yShape[]): string {
  const s = shapes.find((x) => isTitle(x) && x.text);
  return s?.text ? flat(s.text) : '';
}

/** What a shape is, for screen readers: its text, or its kind and alternative text / name. */
export function shapeSummary(s: SlideA11yShape, t: Translate): string {
  if (s.text && s.kind === 'shape') return flat(s.text);
  const kind = t(`a11y.shapeKind.${s.kind}`);
  const what = s.alt ?? (s.text ? flat(s.text) : s.name);
  return what ? `${kind}: ${what}` : kind;
}

/** Name of thumbnail `index` (0-based). */
export function thumbLabel(
  index: number,
  shapes: readonly SlideA11yShape[] | undefined,
  t: Translate,
) {
  const title = shapes ? slideTitle(shapes) : '';
  const n = t('a11y.slideN', { n: index + 1 });
  return title ? `${n}: ${title}` : n;
}

/** Announcement of the selected shapes (`''`: nothing selected). */
export function selectionAnnouncement(sel: readonly SlideShapeInfo[], t: Translate): string {
  if (sel.length === 0) return '';
  if (sel.length > 1) return t('a11y.shapesSelected', { count: sel.length });
  const s = sel[0]!;
  const text = s.text ? flat(s.text) : '';
  const name = text && s.kind === 'shape' ? text : `${t(`a11y.shapeKind.${s.kind}`)} ${s.name}`;
  return t('a11y.shapeSelected', { name });
}

/** The outline of the deck, read again when it changes (`version`). */
export function useSlideOutline(
  engine: Pick<EngineHost, 'slideCommand'> | null,
  count: number,
  version: unknown,
): SlideA11yShape[][] {
  const [slides, setSlides] = useState<SlideA11yShape[][]>([]);
  useEffect(() => {
    if (!engine || count === 0) return;
    let cancelled = false;
    const timer = setTimeout(() => {
      void (
        engine.slideCommand({
          cmd: 'slideOutline',
          from: 0,
          to: Math.min(count, MAX_OUTLINE_SLIDES) - 1,
        }) as unknown as Promise<SlideOutline>
      )
        .then((o) => {
          if (cancelled) return;
          const out: SlideA11yShape[][] = [];
          for (const s of o.slides ?? []) out[s.index] = s.shapes;
          setSlides(out);
        })
        .catch(() => {});
    }, 100);
    return () => {
      cancelled = true;
      clearTimeout(timer);
    };
  }, [engine, count, version]);
  return count === 0 ? [] : slides;
}

/** Hidden description of a thumbnail: the slide's shapes after its title. */
export function ThumbDescription({ id, shapes }: { id: string; shapes?: SlideA11yShape[] }) {
  const { t } = useTranslation();
  const tr: Translate = (k, o) => String(t(k, o));
  const text = (shapes ?? [])
    .filter((s) => !(isTitle(s) && s.text))
    .map((s) => shapeSummary(s, tr))
    .filter(Boolean)
    .join('. ');
  return (
    <span id={id} style={HIDDEN}>
      {text}
    </span>
  );
}

/** Hidden copy of the slide on the stage, and the live region of the selection. */
export function SlideStageMirror({
  index,
  shapes,
  selection,
}: {
  index: number;
  shapes?: SlideA11yShape[];
  selection: readonly SlideShapeInfo[];
}) {
  const { t } = useTranslation();
  const tr: Translate = (k, o) => String(t(k, o));
  // The live region only speaks when this text changes (another selection).
  const said = selectionAnnouncement(selection, tr);
  return (
    <>
      <section
        aria-label={tr('a11y.slideShapes', { n: index + 1 })}
        data-testid="slide-a11y-mirror"
        style={HIDDEN}
      >
        {(shapes ?? []).map((s) =>
          isTitle(s) && s.text ? (
            <h2 key={s.id} data-shape={s.id}>
              {flat(s.text)}
            </h2>
          ) : s.kind === 'shape' && s.text ? (
            <p key={s.id} data-shape={s.id}>
              {flat(s.text)}
            </p>
          ) : (
            <span key={s.id} role="img" aria-label={shapeSummary(s, tr)} data-shape={s.id} />
          ),
        )}
      </section>
      <div aria-live="polite" aria-atomic="true" data-testid="slide-a11y-live" style={HIDDEN}>
        {said}
      </div>
    </>
  );
}
