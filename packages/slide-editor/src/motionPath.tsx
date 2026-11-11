/**
 * Motion paths on the slide (ECMA-376 Part 1, §19.5.4 animMotion@path: `M`, `L`, `C`, `Z`, `E`
 * commands, offsets in slide widths / heights from the shape's position):
 *
 * - the paths of the selected shape are drawn from its centre (green start, red end);
 * - "Edit points": the anchors are dragged, each drop is one `setAnimation { path }`;
 * - "Draw motion path": clicks add corners, a drag draws freehand; double-click or Enter ends
 *   it (`addAnimation { effect: 'customPath', path }`), Escape cancels.
 *
 * The path helpers are pure and unit-tested.
 */
import type { SlideAnimInfo, SlideCommand } from '@blueoffice/platform';
import {
  useEffect,
  useRef,
  useState,
  type KeyboardEvent as ReactKeyboardEvent,
  type PointerEvent as ReactPointerEvent,
} from 'react';
import styles from './SlideEditor.module.css';

type Pt = [number, number];

/** One command of a path, absolute (`Z` has no points; `C`: two controls then the end). */
export interface PathSeg {
  c: 'M' | 'L' | 'C' | 'Z';
  pts: Pt[];
}

/** Parse a §19.5.4 path into absolute commands (relative ones are made absolute). */
export function parseMotionPath(path: string): PathSeg[] {
  const toks = path.match(/[MLCZEmlcze]|[-+]?(?:\d+\.?\d*|\.\d+)(?:[eE][-+]?\d+)?/g) ?? [];
  const out: PathSeg[] = [];
  let cmd = 'M';
  let cur: Pt = [0, 0];
  let start: Pt = [0, 0];
  let i = 0;
  const num = () => Number(toks[i++]);
  while (i < toks.length) {
    const t = toks[i] ?? '';
    if (/^[A-Za-z]$/.test(t)) {
      cmd = t;
      i++;
      if (cmd === 'Z' || cmd === 'z') {
        out.push({ c: 'Z', pts: [] });
        cur = start;
      }
      if (cmd === 'E' || cmd === 'e') break;
      continue;
    }
    const rel = cmd === cmd.toLowerCase();
    const n = cmd.toUpperCase() === 'C' ? 3 : 1;
    const pts: Pt[] = [];
    for (let k = 0; k < n; k++) {
      if (i + 1 >= toks.length) return out;
      const x = num();
      const y = num();
      if (!Number.isFinite(x) || !Number.isFinite(y)) return out;
      pts.push(rel ? [cur[0] + x, cur[1] + y] : [x, y]);
    }
    const up = cmd.toUpperCase();
    const c: PathSeg['c'] = up === 'M' ? 'M' : up === 'C' ? 'C' : 'L';
    cur = pts[pts.length - 1] ?? cur;
    if (c === 'M') start = cur;
    out.push({ c, pts });
  }
  return out;
}

const fmt = (v: number) => {
  const r = Math.round(v * 1e6) / 1e6;
  return Object.is(r, -0) || r === 0 ? '0' : String(r);
};

/** Write commands back as a §19.5.4 path (`E` ends an open path). */
export function formatMotionPath(segs: PathSeg[]): string {
  const body = segs
    .map((s) =>
      s.c === 'Z' ? 'Z' : `${s.c} ${s.pts.map(([x, y]) => `${fmt(x)} ${fmt(y)}`).join(' ')}`,
    )
    .join(' ');
  return segs.at(-1)?.c === 'Z' ? body : `${body} E`;
}

/** The anchors of a path (start and each end point), as indices into `segs`. */
export function anchors(segs: PathSeg[]): { seg: number; pt: Pt }[] {
  const out: { seg: number; pt: Pt }[] = [];
  segs.forEach((s, i) => {
    const p = s.pts[s.pts.length - 1];
    if (p) out.push({ seg: i, pt: p });
  });
  return out;
}

/**
 * Move the anchor ending command `seg` by (`dx`, `dy`) (slide fractions); the curve controls
 * next to it move along so the curve keeps its shape at that point.
 */
export function moveAnchor(segs: PathSeg[], seg: number, dx: number, dy: number): PathSeg[] {
  const add = ([x, y]: Pt): Pt => [x + dx, y + dy];
  return segs.map((s, i) => {
    if (i === seg) {
      const pts = s.pts.slice();
      const last = pts.length - 1;
      if (last >= 0) pts[last] = add(pts[last] ?? [0, 0]);
      // The control leading into the point.
      if (s.c === 'C' && pts[1]) pts[1] = add(pts[1]);
      return { ...s, pts };
    }
    if (i === seg + 1 && s.c === 'C' && s.pts[0]) {
      // The control leaving the point.
      return { ...s, pts: [add(s.pts[0]), ...s.pts.slice(1)] };
    }
    return s;
  });
}

/**
 * The path of points drawn on the slide (points), relative to the first one, in slide fractions;
 * points closer than `minGap` points to the previous one are dropped.
 */
export function drawnPath(points: Pt[], width: number, height: number, minGap = 2): string | null {
  const W = width || 1;
  const H = height || 1;
  const kept: Pt[] = [];
  for (const p of points) {
    const last = kept.at(-1);
    if (!last || Math.hypot(p[0] - last[0], p[1] - last[1]) >= minGap) kept.push(p);
  }
  const first = kept[0];
  if (!first || kept.length < 2) return null;
  const segs: PathSeg[] = kept.map((p, i) => ({
    c: i === 0 ? 'M' : 'L',
    pts: [[(p[0] - first[0]) / W, (p[1] - first[1]) / H]],
  }));
  return formatMotionPath(segs);
}

/** SVG path data of a motion path drawn from `origin` (points) on a `width` × `height` slide. */
export function motionPathSvg(segs: PathSeg[], origin: Pt, width: number, height: number): string {
  const at = ([x, y]: Pt) => `${fmt(origin[0] + x * width)} ${fmt(origin[1] + y * height)}`;
  return segs.map((s) => (s.c === 'Z' ? 'Z' : `${s.c} ${s.pts.map(at).join(' ')}`)).join(' ');
}

export interface MotionPathLabels {
  path: (name: string) => string;
  point: (n: number) => string;
  drawHint: string;
}

export interface MotionPathOverlayProps {
  /** Slide size in points. */
  slide: { width: number; height: number };
  /** Displayed width in CSS pixels. */
  width: number;
  /** Path effects of the selected shape. */
  paths: SlideAnimInfo[];
  /** Index of the effect whose points are edited. */
  editing: number | null;
  /** Drawing a new path. */
  drawing: boolean;
  run: (cmd: SlideCommand) => void;
  /** Drawing ended (finished or cancelled). */
  onDrawn: () => void;
  labels: MotionPathLabels;
}

/** Motion paths over the slide, their point editor and the path drawing tool. */
export function MotionPathOverlay({
  slide,
  width,
  paths,
  editing,
  drawing,
  run,
  onDrawn,
  labels,
}: MotionPathOverlayProps) {
  const k = slide.width > 0 ? width / slide.width : 1;
  const height = slide.height * k;
  const svgRef = useRef<SVGSVGElement>(null);
  const [drag, setDrag] = useState<{ index: number; seg: number; dx: number; dy: number } | null>(
    null,
  );
  const dragStart = useRef<Pt | null>(null);
  // Drawing: points so far (slide points), the pointer, and whether the button is down.
  const [drawn, setDrawn] = useState<Pt[]>([]);
  const [hover, setHover] = useState<Pt | null>(null);
  const down = useRef(false);
  useEffect(() => {
    if (drawing) svgRef.current?.focus();
  }, [drawing]);

  const toSlide = (e: { clientX: number; clientY: number }): Pt => {
    const r = svgRef.current?.getBoundingClientRect();
    return r ? [(e.clientX - r.left) / k, (e.clientY - r.top) / k] : [0, 0];
  };
  const finish = (pts: Pt[]) => {
    const path = drawnPath(pts, slide.width, slide.height);
    if (path) run({ cmd: 'addAnimation', effect: 'customPath', path });
    setDrawn([]);
    setHover(null);
    onDrawn();
  };

  const onKey = (e: ReactKeyboardEvent) => {
    if (!drawing) return;
    // The drawing tool owns the keys while it is on (not the editor's shortcuts).
    e.stopPropagation();
    if (e.key === 'Escape') {
      e.preventDefault();
      setDrawn([]);
      onDrawn();
    } else if (e.key === 'Enter') {
      e.preventDefault();
      finish(drawn);
    }
  };

  const items = paths
    .filter((a) => a.path && a.rect)
    .map((a) => {
      const segs0 = parseMotionPath(a.path ?? '');
      const segs =
        drag && drag.index === a.index ? moveAnchor(segs0, drag.seg, drag.dx, drag.dy) : segs0;
      const [x, y, w, h] = a.rect ?? [0, 0, 0, 0];
      return { a, segs, origin: [x + w / 2, y + h / 2] as Pt };
    });

  return (
    <svg
      ref={svgRef}
      className={drawing ? styles.motionDraw : styles.motionPaths}
      width={width}
      height={height}
      viewBox={`0 0 ${slide.width} ${slide.height}`}
      data-testid="motion-paths"
      tabIndex={drawing ? 0 : -1}
      aria-label={drawing ? labels.drawHint : undefined}
      onKeyDown={onKey}
      onPointerDown={
        drawing
          ? (e: ReactPointerEvent) => {
              e.preventDefault();
              e.stopPropagation();
              down.current = true;
              setDrawn((d) => [...d, toSlide(e)]);
            }
          : undefined
      }
      onPointerMove={
        drawing
          ? (e: ReactPointerEvent) => {
              const p = toSlide(e);
              setHover(p);
              // A drag draws freehand.
              if (down.current) setDrawn((d) => [...d, p]);
            }
          : undefined
      }
      onPointerUp={
        drawing
          ? () => {
              down.current = false;
            }
          : undefined
      }
      onDoubleClick={drawing ? () => finish(drawn) : undefined}
    >
      {items.map(({ a, segs, origin }) => (
        <g key={a.index} aria-label={labels.path(a.shapeName)}>
          <path
            className={styles.motionLine}
            d={motionPathSvg(segs, origin, slide.width, slide.height)}
            data-testid={`motion-path-${a.index}`}
          />
          {anchors(segs).map(({ seg, pt }, n) => {
            const cx = origin[0] + pt[0] * slide.width;
            const cy = origin[1] + pt[1] * slide.height;
            const first = n === 0;
            const last = n === anchors(segs).length - 1;
            const edit = editing === a.index;
            if (!edit && !first && !last) return null;
            return (
              <circle
                key={seg}
                cx={cx}
                cy={cy}
                r={(edit ? 5 : 4) / k}
                className={`${first ? styles.motionStart : last ? styles.motionEnd : styles.motionPoint} ${edit ? styles.motionGrab : ''}`}
                data-testid={edit ? `motion-point-${a.index}-${n}` : undefined}
                aria-label={edit ? labels.point(n + 1) : undefined}
                onPointerDown={
                  edit
                    ? (e) => {
                        e.stopPropagation();
                        (e.target as Element).setPointerCapture?.(e.pointerId);
                        dragStart.current = toSlide(e);
                        setDrag({ index: a.index, seg, dx: 0, dy: 0 });
                      }
                    : undefined
                }
                onPointerMove={
                  edit
                    ? (e) => {
                        const s = dragStart.current;
                        if (!s || !drag) return;
                        const p = toSlide(e);
                        setDrag({
                          ...drag,
                          dx: (p[0] - s[0]) / (slide.width || 1),
                          dy: (p[1] - s[1]) / (slide.height || 1),
                        });
                      }
                    : undefined
                }
                onPointerUp={
                  edit
                    ? () => {
                        const d = drag;
                        dragStart.current = null;
                        setDrag(null);
                        if (d && (d.dx !== 0 || d.dy !== 0)) {
                          run({
                            cmd: 'setAnimation',
                            index: a.index,
                            path: formatMotionPath(moveAnchor(segs0Of(a), d.seg, d.dx, d.dy)),
                          });
                        }
                      }
                    : undefined
                }
              />
            );
          })}
        </g>
      ))}
      {drawing && drawn.length > 0 && (
        <polyline
          className={styles.motionLine}
          data-testid="motion-drawing"
          points={[...drawn, ...(hover ? [hover] : [])].map(([x, y]) => `${x},${y}`).join(' ')}
        />
      )}
    </svg>
  );
}

function segs0Of(a: SlideAnimInfo): PathSeg[] {
  return parseMotionPath(a.path ?? '');
}
