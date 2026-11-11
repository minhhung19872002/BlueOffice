import type {
  PageDisplayList,
  SlideMorphInfo,
  SlideMorphOutline,
  SlideTransitionInfo,
  SlideTransitionKind,
} from '@blueoffice/platform';

/**
 * Slide transitions in the slide show (ECMA-376 Part 1, §19.3.1.50 transition and its effects,
 * [MS-PPTX] p14 / p15): keyframes of the incoming and the outgoing slide, played by the show stage
 * with the Web Animations API. Pieces (blinds, checkerboard, dissolve, …) are one `clip-path`
 * polygon whose rectangles grow; solids (cube, flip, …) use CSS 3-D transforms. Unit-tested.
 */

/** Keyframes of one layer (CSS properties, optional `offset`). */
export type Frames = Record<string, string | number>[];

export interface TransitionPlan {
  duration: number;
  /** Frames of the incoming slide (`null` = static). */
  incoming: Frames | null;
  /** Frames of the outgoing slide (`null` = static, under the incoming one). */
  outgoing: Frames | null;
  /** The outgoing slide is drawn over the incoming one (pull, page curl, …). */
  outgoingOnTop: boolean;
}

/** The transitions of the picker, in groups. */
export const TRANSITION_GROUPS: {
  group: 'subtle' | 'exciting' | 'dynamic';
  kinds: SlideTransitionKind[];
}[] = [
  {
    group: 'subtle',
    kinds: [
      'morph',
      'cut',
      'fade',
      'push',
      'wipe',
      'split',
      'reveal',
      'randomBar',
      'circle',
      'diamond',
      'plus',
      'pull',
      'cover',
      'flash',
      'strips',
    ],
  },
  {
    group: 'exciting',
    kinds: [
      'fallOver',
      'drape',
      'curtains',
      'wind',
      'prestige',
      'fracture',
      'crush',
      'peelOff',
      'pageCurlDouble',
      'pageCurlSingle',
      'airplane',
      'origami',
      'dissolve',
      'checker',
      'blinds',
      'wheel',
      'wheelReverse',
      'wedge',
      'ripple',
      'honeycomb',
      'glitter',
      'vortex',
      'shred',
      'switch',
      'flip',
      'gallery',
      'cube',
      'doors',
      'box',
      'comb',
      'zoom',
      'newsflash',
      'random',
    ],
  },
  {
    group: 'dynamic',
    kinds: ['pan', 'ferris', 'conveyor', 'rotate', 'window', 'orbit', 'flythrough', 'warp'],
  },
];

/** Every transition of the picker. */
export const ALL_TRANSITIONS: SlideTransitionKind[] = TRANSITION_GROUPS.flatMap((g) => g.kinds);

type Rect = [number, number, number, number];

const num = (v: number) => `${Math.round(v * 10000) / 100}%`;

/**
 * One `clip-path` polygon covering rectangles `[x0, y0, x1, y1]` (0..1 of the slide): each rectangle
 * traced in turn, then back along the starting corners (zero-width seams). The same number of
 * rectangles always gives the same number of points, so frames interpolate.
 */
export function rectsPolygon(rects: Rect[]): string {
  const pts: string[] = [];
  for (const [x0, y0, x1, y1] of rects) {
    pts.push(`${num(x0)} ${num(y0)}`, `${num(x1)} ${num(y0)}`, `${num(x1)} ${num(y1)}`);
    pts.push(`${num(x0)} ${num(y1)}`, `${num(x0)} ${num(y0)}`);
  }
  for (let i = rects.length - 2; i >= 0; i--) {
    const [x0, y0] = rects[i] ?? [0, 0];
    pts.push(`${num(x0)} ${num(y0)}`);
  }
  return `polygon(${pts.join(', ')})`;
}

/** A polygon of points `[x, y]` (0..1). */
function polygon(points: [number, number][]): string {
  return `polygon(${points.map(([x, y]) => `${num(x)} ${num(y)}`).join(', ')})`;
}

/** Fan polygons (pie sectors) around (cx, cy), each `[from, sweep]` in degrees clockwise from 12 o'clock. */
function sectors(cx: number, cy: number, list: [number, number][], steps = 8): string {
  const r = 1.5;
  const pts: [number, number][] = [];
  for (const [from, sweep] of list) {
    pts.push([cx, cy]);
    for (let k = 0; k <= steps; k++) {
      const a = ((from + (sweep * k) / steps) * Math.PI) / 180;
      pts.push([cx + Math.sin(a) * r, cy - Math.cos(a) * r]);
    }
    pts.push([cx, cy]);
  }
  return polygon(pts);
}

/** A deterministic permutation of 0..n (rank of each item), from `seed`. */
function ranks(n: number, seed: number): number[] {
  const order = Array.from({ length: n }, (_, i) => i);
  let s = (seed * 2654435761 + 12345) >>> 0;
  for (let i = n - 1; i > 0; i--) {
    s = (Math.imul(s, 1103515245) + 12345) >>> 0;
    const j = s % (i + 1);
    [order[i], order[j]] = [order[j] ?? 0, order[i] ?? 0];
  }
  const rank = new Array<number>(n).fill(0);
  order.forEach((item, r) => {
    rank[item] = r;
  });
  return rank;
}

/** Frames of a cell grid whose cells appear in order `rank` (`steps` keyframes). */
function cellFrames(
  cols: number,
  rows: number,
  rank: number[],
  steps: number,
  reverse = false,
): Frames {
  const n = cols * rows;
  const frames: Frames = [];
  for (let k = 0; k <= steps; k++) {
    const shown = Math.round((n * k) / steps);
    const rects: Rect[] = [];
    for (let r = 0; r < rows; r++) {
      for (let c = 0; c < cols; c++) {
        const i = r * cols + c;
        const on = reverse ? (rank[i] ?? 0) >= shown : (rank[i] ?? 0) < shown;
        const [x0, y0] = [c / cols, r / rows];
        rects.push(on ? [x0, y0, x0 + 1 / cols, y0 + 1 / rows] : [x0, y0, x0, y0]);
      }
    }
    frames.push({ clipPath: rectsPolygon(rects), offset: k / steps });
  }
  return frames;
}

/** Frames of rectangles moving between two states (`at(t)` for t = 0 … 1, `steps` keyframes). */
function rectFrames(at: (t: number) => Rect[], steps = 1): Frames {
  return Array.from({ length: steps + 1 }, (_, k) => ({
    clipPath: rectsPolygon(at(k / steps)),
    offset: k / steps,
  }));
}

const clamp01 = (v: number) => Math.min(1, Math.max(0, v));

/** Translation towards side `d` (`l`, `r`, `u`, `d` and the diagonals `lu`, `rd`, …). */
function offset(d: string, k: number): string {
  const dx = d.includes('l') ? -k : d.includes('r') ? k : 0;
  const dy = d.includes('u') ? -k : d.includes('d') ? k : 0;
  return `translate(${dx * 100}%, ${dy * 100}%)`;
}

const FULL = 'inset(0% 0% 0% 0%)';
const fadeIn: Frames = [{ opacity: 0 }, { opacity: 1 }];

/** Transitions `random` picks from (by the seed: the slide index). */
const RANDOM_POOL: SlideTransitionKind[] = [
  'fade',
  'push',
  'wipe',
  'split',
  'blinds',
  'circle',
  'dissolve',
  'cover',
];

/**
 * Keyframes of a slide transition (§19.3.1.50; directions are where the motion goes). `seed` picks
 * the effect of `random` and the order of dissolving pieces. Effects BlueSlide does not know play as
 * a fade.
 */
export function transitionPlan(t: SlideTransitionInfo | null, seed = 0): TransitionPlan {
  const none: TransitionPlan = {
    duration: 0,
    incoming: null,
    outgoing: null,
    outgoingOnTop: false,
  };
  if (!t) return none;
  const duration = Math.max(1, t.durMs);
  const plan = (
    incoming: Frames | null,
    outgoing: Frames | null,
    outgoingOnTop = false,
  ): TransitionPlan => ({
    duration,
    incoming,
    outgoing,
    outgoingOnTop,
  });
  if (!t.known) return plan(fadeIn, null);
  let kind = t.kind as SlideTransitionKind;
  if (kind === 'random') kind = RANDOM_POOL[Math.abs(seed) % RANDOM_POOL.length] ?? 'fade';
  const dir = t.dir ?? '';
  const sign = dir.startsWith('r') ? -1 : 1;
  const per = 'perspective(1600px)';
  switch (kind) {
    case 'morph':
      // The shapes move in the morph overlay (`morphLayers`); the slide itself shows at the end.
      return plan(
        [
          { opacity: 0, offset: 0 },
          { opacity: 0, offset: 0.999 },
          { opacity: 1, offset: 1 },
        ],
        [{ opacity: 0 }, { opacity: 0 }],
      );
    case 'cut':
      // Through black: the outgoing slide goes at once, the incoming one comes halfway.
      return dir === 'black'
        ? plan(
            [
              { opacity: 0, offset: 0 },
              { opacity: 0, offset: 0.5 },
              { opacity: 1, offset: 0.5001 },
              { opacity: 1, offset: 1 },
            ],
            [{ opacity: 0 }, { opacity: 0 }],
            true,
          )
        : none;
    case 'fade':
      return dir === 'black'
        ? plan(
            [
              { opacity: 0, offset: 0 },
              { opacity: 0, offset: 0.5 },
              { opacity: 1, offset: 1 },
            ],
            [
              { opacity: 1, offset: 0 },
              { opacity: 0, offset: 0.5 },
              { opacity: 0, offset: 1 },
            ],
            true,
          )
        : plan(fadeIn, null);
    case 'push':
      return plan(
        [{ transform: offset(dir || 'l', -1) }, { transform: 'translate(0%, 0%)' }],
        [{ transform: 'translate(0%, 0%)' }, { transform: offset(dir || 'l', 1) }],
      );
    case 'cover':
      return plan(
        [{ transform: offset(dir || 'l', -1) }, { transform: 'translate(0%, 0%)' }],
        null,
      );
    case 'pull':
      return plan(
        null,
        [{ transform: 'translate(0%, 0%)' }, { transform: offset(dir || 'l', 1) }],
        true,
      );
    case 'wipe': {
      const start =
        dir === 'r'
          ? 'inset(0% 100% 0% 0%)'
          : dir === 'u'
            ? 'inset(100% 0% 0% 0%)'
            : dir === 'd'
              ? 'inset(0% 0% 100% 0%)'
              : 'inset(0% 0% 0% 100%)';
      return plan([{ clipPath: start }, { clipPath: FULL }], null);
    }
    case 'split': {
      const vert = dir.startsWith('vert');
      if (dir.endsWith('In')) {
        // From the two edges towards the middle.
        const at = (p: number): Rect[] =>
          vert
            ? [
                [0, 0, p / 2, 1],
                [1 - p / 2, 0, 1, 1],
              ]
            : [
                [0, 0, 1, p / 2],
                [0, 1 - p / 2, 1, 1],
              ];
        return plan(rectFrames(at), null);
      }
      const start = vert ? 'inset(0% 50% 0% 50%)' : 'inset(50% 0% 50% 0%)';
      return plan([{ clipPath: start }, { clipPath: FULL }], null);
    }
    case 'reveal': {
      const black = dir.endsWith('Black');
      const away = dir.startsWith('r') ? 'translate(8%, 0%)' : 'translate(-8%, 0%)';
      return plan(
        [
          { opacity: 0, offset: 0 },
          { opacity: 0, offset: black ? 0.5 : 0.3 },
          { opacity: 1, offset: 1 },
        ],
        [
          { opacity: 1, transform: 'translate(0%, 0%)', offset: 0 },
          { opacity: 0, transform: away, offset: black ? 0.5 : 0.7 },
          { opacity: 0, transform: away, offset: 1 },
        ],
        true,
      );
    }
    case 'randomBar': {
      const n = 24;
      const rank = ranks(n, seed + 7);
      const vert = dir === 'vert';
      const frames: Frames = [];
      for (let k = 0; k <= n; k += 3) {
        const rects: Rect[] = rank.map((r, j) => {
          const on = r < k;
          return vert
            ? [j / n, 0, on ? (j + 1) / n : j / n, 1]
            : [0, j / n, 1, on ? (j + 1) / n : j / n];
        });
        frames.push({ clipPath: rectsPolygon(rects), offset: k / n });
      }
      return plan(frames, null);
    }
    case 'blinds': {
      const n = 6;
      const vert = dir === 'vert';
      return plan(
        rectFrames((p) =>
          Array.from({ length: n }, (_, i): Rect =>
            vert ? [i / n, 0, (i + p) / n, 1] : [0, i / n, 1, (i + p) / n],
          ),
        ),
        null,
      );
    }
    case 'checker': {
      const [cols, rows] = [8, 6];
      const vert = dir === 'vert';
      const at = (t: number): Rect[] => {
        const out: Rect[] = [];
        for (let r = 0; r < rows; r++) {
          for (let c = 0; c < cols; c++) {
            // Every other square starts halfway (a checkerboard).
            const p = clamp01((t - ((r + c) % 2) * 0.5) * 2);
            const [x0, y0] = [c / cols, r / rows];
            out.push(
              vert
                ? [x0, y0, x0 + 1 / cols, y0 + p / rows]
                : [x0, y0, x0 + p / cols, y0 + 1 / rows],
            );
          }
        }
        return out;
      };
      return plan(rectFrames(at, 2), null);
    }
    case 'comb': {
      const n = 8;
      const vert = dir === 'vert';
      return plan(
        rectFrames((p) =>
          Array.from({ length: n }, (_, i): Rect => {
            const even = i % 2 === 0;
            const [a, b] = even ? [1 - p, 1] : [0, p];
            return vert ? [i / n, a, (i + 1) / n, b] : [a, i / n, b, (i + 1) / n];
          }),
        ),
        null,
      );
    }
    case 'circle':
      return plan(
        [{ clipPath: 'circle(0% at 50% 50%)' }, { clipPath: 'circle(72% at 50% 50%)' }],
        null,
      );
    case 'diamond':
      return plan(
        [
          {
            clipPath: polygon([
              [0.5, 0.5],
              [0.5, 0.5],
              [0.5, 0.5],
              [0.5, 0.5],
            ]),
          },
          {
            clipPath: polygon([
              [0.5, -0.5],
              [1.5, 0.5],
              [0.5, 1.5],
              [-0.5, 0.5],
            ]),
          },
        ],
        null,
      );
    case 'plus': {
      const cross = (k: number): [number, number][] => {
        const [a, b] = [0.5 - k, 0.5 + k];
        const [o, e] = [0.5 - 2 * k, 0.5 + 2 * k];
        return [
          [a, o],
          [b, o],
          [b, a],
          [e, a],
          [e, b],
          [b, b],
          [b, e],
          [a, e],
          [a, b],
          [o, b],
          [o, a],
          [a, a],
        ];
      };
      return plan([{ clipPath: polygon(cross(0)) }, { clipPath: polygon(cross(0.5)) }], null);
    }
    case 'wedge':
      return plan(
        Array.from({ length: 5 }, (_, k) => ({
          clipPath: sectors(0.5, 0.5, [[-(k / 4) * 180, (k / 4) * 360]], 16),
          offset: k / 4,
        })),
        null,
      );
    case 'wheel':
    case 'wheelReverse': {
      const spokes = Math.max(1, Number(dir) || 4);
      const cw = kind === 'wheel' ? 1 : -1;
      return plan(
        Array.from({ length: 9 }, (_, k) => ({
          clipPath: sectors(
            0.5,
            0.5,
            Array.from({ length: spokes }, (_, i): [number, number] => [
              (i * 360) / spokes,
              (cw * (k / 8) * 360) / spokes,
            ]),
          ),
          offset: k / 8,
        })),
        null,
      );
    }
    case 'strips': {
      const corner = dir || 'lu';
      const tri = (s: number): [number, number][] => {
        const x = corner.includes('l') ? 0 : 1;
        const y = corner.includes('u') ? 0 : 1;
        const dx = x === 0 ? s : -s;
        const dy = y === 0 ? s : -s;
        return [
          [x, y],
          [x + dx, y],
          [x, y + dy],
        ];
      };
      return plan([{ clipPath: polygon(tri(0)) }, { clipPath: polygon(tri(2)) }], null);
    }
    case 'newsflash':
      return plan(
        [
          { transform: 'rotate(-720deg) scale(0)', opacity: 0 },
          { transform: 'rotate(0deg) scale(1)', opacity: 1 },
        ],
        null,
      );
    case 'zoom':
      return dir === 'in'
        ? plan(
            [
              { transform: 'scale(0.3)', opacity: 0 },
              { transform: 'scale(1)', opacity: 1 },
            ],
            null,
          )
        : plan(
            [
              { transform: 'scale(1.6)', opacity: 0 },
              { transform: 'scale(1)', opacity: 1 },
            ],
            null,
          );
    case 'dissolve':
      return plan(cellFrames(16, 10, ranks(160, seed + 1), 16), null);
    case 'honeycomb': {
      // Cells from the middle outwards, a little irregular.
      const [cols, rows] = [12, 8];
      const noise = ranks(cols * rows, seed + 3);
      const dist = Array.from({ length: cols * rows }, (_, i) => {
        const [c, r] = [i % cols, Math.floor(i / cols)];
        return (
          Math.hypot(c + 0.5 - cols / 2, (r + 0.5 - rows / 2) * 1.4) +
          (noise[i] ?? 0) / (cols * rows)
        );
      });
      return plan(cellFrames(cols, rows, rankBy(dist), 12), null);
    }
    case 'glitter': {
      const [cols, rows] = [16, 10];
      const noise = ranks(cols * rows, seed + 5);
      const along = Array.from({ length: cols * rows }, (_, i) => {
        const [c, r] = [i % cols, Math.floor(i / cols)];
        const pos = dir.startsWith('l')
          ? c / cols
          : dir.startsWith('r')
            ? 1 - c / cols
            : dir === 'u'
              ? r / rows
              : 1 - r / rows;
        return pos + ((noise[i] ?? 0) / (cols * rows)) * 0.4;
      });
      // From the side the motion leaves: `l` sweeps towards the left.
      return plan(cellFrames(cols, rows, rankBy(along.map((v) => 1 - v)), 16), null);
    }
    case 'fracture':
      return plan(null, cellFrames(10, 7, ranks(70, seed + 9), 12, true), true);
    case 'shred': {
      const out = dir.endsWith('Out');
      const strips = dir.startsWith('strip');
      const frames = strips
        ? rectFrames((p) =>
            Array.from({ length: 16 }, (_, i): Rect => {
              const q = out ? 1 - p : p;
              return i % 2 === 0 ? [i / 16, 0, (i + 1) / 16, q] : [i / 16, 1 - q, (i + 1) / 16, 1];
            }),
          )
        : cellFrames(12, 8, ranks(96, seed + 11), 12, out);
      return out ? plan(null, frames, true) : plan(frames, null);
    }
    case 'flash':
      return plan(
        [
          { opacity: 0, filter: 'brightness(4)', offset: 0 },
          { opacity: 1, filter: 'brightness(4)', offset: 0.35 },
          { opacity: 1, filter: 'brightness(1)', offset: 1 },
        ],
        null,
      );
    case 'vortex':
      return plan(
        [
          { transform: `rotate(${sign * -540}deg) scale(0.1)`, opacity: 0, filter: 'blur(6px)' },
          { transform: 'rotate(0deg) scale(1)', opacity: 1, filter: 'blur(0px)' },
        ],
        [{ opacity: 1 }, { opacity: 0 }],
        true,
      );
    case 'ripple': {
      const at =
        dir === 'lu'
          ? '0% 0%'
          : dir === 'ru'
            ? '100% 0%'
            : dir === 'ld'
              ? '0% 100%'
              : dir === 'rd'
                ? '100% 100%'
                : '50% 50%';
      return plan(
        [
          { clipPath: `circle(0% at ${at})`, transform: 'scale(1.04)' },
          { clipPath: `circle(80% at ${at})`, transform: 'scale(0.98)', offset: 0.7 },
          { clipPath: `circle(150% at ${at})`, transform: 'scale(1)' },
        ],
        null,
      );
    }
    case 'switch':
      return plan(
        [
          { transform: `${per} translateX(${sign * 60}%) rotateY(${sign * -70}deg)`, opacity: 0 },
          { transform: `${per} translateX(0%) rotateY(0deg)`, opacity: 1 },
        ],
        [
          { transform: `${per} translateX(0%) rotateY(0deg)`, opacity: 1 },
          { transform: `${per} translateX(${sign * -60}%) rotateY(${sign * 70}deg)`, opacity: 0 },
        ],
      );
    case 'flip':
      return plan(
        [
          { transform: `${per} rotateY(${sign * 90}deg)`, opacity: 0, offset: 0 },
          { transform: `${per} rotateY(${sign * 90}deg)`, opacity: 1, offset: 0.5 },
          { transform: `${per} rotateY(0deg)`, opacity: 1, offset: 1 },
        ],
        [
          { transform: `${per} rotateY(0deg)`, opacity: 1, offset: 0 },
          { transform: `${per} rotateY(${sign * -90}deg)`, opacity: 1, offset: 0.5 },
          { transform: `${per} rotateY(${sign * -90}deg)`, opacity: 0, offset: 0.5001 },
          { transform: `${per} rotateY(${sign * -90}deg)`, opacity: 0, offset: 1 },
        ],
        true,
      );
    case 'gallery':
      return plan(
        [
          { transform: `${per} translateX(${sign * 100}%) rotateY(${sign * -30}deg) scale(0.8)` },
          { transform: `${per} translateX(0%) rotateY(0deg) scale(1)` },
        ],
        [
          { transform: `${per} translateX(0%) rotateY(0deg) scale(1)` },
          { transform: `${per} translateX(${sign * -100}%) rotateY(${sign * 30}deg) scale(0.8)` },
        ],
      );
    case 'cube':
    case 'box':
    case 'rotate':
    case 'orbit': {
      // A turning prism: the faces hinge on the edge they share (inside for a box).
      const vertical = dir === 'u' || dir === 'd';
      const s = dir === 'r' || dir === 'd' ? -1 : 1;
      const inv = kind === 'box' || kind === 'orbit' ? -1 : 1;
      const axis = vertical ? 'rotateX' : 'rotateY';
      const turn = (k: number) => `${axis}(${(vertical ? -1 : 1) * s * inv * k * 90}deg)`;
      const edgeOut = vertical ? (s > 0 ? '50% 0%' : '50% 100%') : s > 0 ? '0% 50%' : '100% 50%';
      const edgeIn = vertical ? (s > 0 ? '50% 100%' : '50% 0%') : s > 0 ? '100% 50%' : '0% 50%';
      const shrink = kind === 'orbit' ? ' scale(0.85)' : '';
      return plan(
        [
          {
            transform: `${per} ${offset(vertical ? (s > 0 ? 'd' : 'u') : s > 0 ? 'r' : 'l', 1)} ${turn(-1)}${shrink}`,
            transformOrigin: edgeIn,
          },
          { transform: `${per} translate(0%, 0%) ${turn(0)}`, transformOrigin: edgeIn },
        ],
        [
          { transform: `${per} translate(0%, 0%) ${turn(0)}`, transformOrigin: edgeOut },
          {
            transform: `${per} ${offset(vertical ? (s > 0 ? 'u' : 'd') : s > 0 ? 'l' : 'r', 1)} ${turn(1)}${shrink}`,
            transformOrigin: edgeOut,
          },
        ],
      );
    }
    case 'doors':
    case 'window': {
      const vert = dir !== 'horz';
      // The outgoing slide opens from the middle towards both edges.
      const at = (p: number): Rect[] => {
        const q = (1 - p) / 2;
        return vert
          ? [
              [0, 0, q, 1],
              [1 - q, 0, 1, 1],
            ]
          : [
              [0, 0, 1, q],
              [0, 1 - q, 1, 1],
            ];
      };
      const from = kind === 'doors' ? 0.85 : 0.6;
      return plan(
        [
          { transform: `scale(${from})`, opacity: 0.4 },
          { transform: 'scale(1)', opacity: 1 },
        ],
        rectFrames(at),
        true,
      );
    }
    case 'ferris':
      return plan(
        [
          { transform: `rotate(${sign * -25}deg)`, transformOrigin: '50% 400%' },
          { transform: 'rotate(0deg)', transformOrigin: '50% 400%' },
        ],
        [
          { transform: 'rotate(0deg)', transformOrigin: '50% 400%' },
          { transform: `rotate(${sign * 25}deg)`, transformOrigin: '50% 400%' },
        ],
      );
    case 'conveyor':
      return plan(
        [
          { transform: `${per} translateX(${sign * 100}%) rotateY(${sign * -25}deg)` },
          { transform: `${per} translateX(0%) rotateY(0deg)` },
        ],
        [
          { transform: `${per} translateX(0%) rotateY(0deg)` },
          { transform: `${per} translateX(${sign * -100}%) rotateY(${sign * 25}deg)` },
        ],
      );
    case 'pan': {
      const d = dir || 'd';
      return plan(
        [
          { transform: `${offset(d, -1)} scale(0.9)` },
          { transform: 'translate(0%, 0%) scale(0.95)', offset: 0.6 },
          { transform: 'translate(0%, 0%) scale(1)' },
        ],
        [{ transform: 'translate(0%, 0%) scale(1)' }, { transform: `${offset(d, 1)} scale(0.9)` }],
      );
    }
    case 'warp':
      return dir === 'out'
        ? plan(
            [
              { transform: 'scale(1.8)', opacity: 0 },
              { transform: 'scale(1)', opacity: 1 },
            ],
            [
              { transform: 'scale(1)', opacity: 1 },
              { transform: 'scale(0.1)', opacity: 0 },
            ],
            true,
          )
        : plan(
            [
              { transform: 'scale(0.1)', opacity: 0 },
              { transform: 'scale(1)', opacity: 1 },
            ],
            [
              { transform: 'scale(1)', opacity: 1 },
              { transform: 'scale(1.8)', opacity: 0 },
            ],
          );
    case 'flythrough': {
      const out = dir.startsWith('out');
      const bounce = dir.endsWith('Bounce');
      const end: Frames = bounce
        ? [
            { transform: 'scale(0.95)', opacity: 1, offset: 0.8 },
            { transform: 'scale(1)', opacity: 1, offset: 1 },
          ]
        : [{ transform: 'scale(1)', opacity: 1, offset: 1 }];
      return plan(
        [{ transform: out ? 'scale(0.3)' : 'scale(3)', opacity: 0, offset: 0 }, ...end],
        [
          { transform: 'scale(1)', opacity: 1 },
          { transform: out ? 'scale(3)' : 'scale(0.3)', opacity: 0 },
        ],
        out,
      );
    }
    // [MS-PPTX] p15:prstTrans presets: the outgoing slide leaves over the incoming one.
    case 'fallOver':
      return plan(
        null,
        [
          { transform: `${per} rotateX(0deg)`, transformOrigin: '50% 100%', opacity: 1 },
          {
            transform: `${per} rotateX(-90deg) rotateZ(${sign * 6}deg)`,
            transformOrigin: '50% 100%',
            opacity: 0.6,
          },
        ],
        true,
      );
    case 'drape':
      return plan(
        null,
        [
          { transform: 'translateY(0%) skewX(0deg)', transformOrigin: '50% 0%' },
          { transform: `translateY(100%) skewX(${sign * 12}deg)`, transformOrigin: '50% 0%' },
        ],
        true,
      );
    case 'curtains':
      return plan(
        null,
        [
          {
            ...{
              clipPath: rectsPolygon([
                [0, 0, 0.5, 1],
                [0.5, 0, 1, 1],
              ]),
            },
            transform: 'scaleY(1)',
          },
          {
            ...{
              clipPath: rectsPolygon([
                [0, 0, 0, 1],
                [1, 0, 1, 1],
              ]),
            },
            transform: 'scaleY(1.06)',
          },
        ],
        true,
      );
    case 'wind':
      return plan(
        [
          { transform: `translateX(${sign * 30}%)`, opacity: 0 },
          { transform: 'translateX(0%)', opacity: 1 },
        ],
        [
          { transform: 'translateX(0%) skewX(0deg)', opacity: 1 },
          { transform: `translateX(${sign * -110}%) skewX(${sign * 25}deg)`, opacity: 0 },
        ],
        true,
      );
    case 'prestige':
      return plan(
        [
          { transform: 'translateY(-15%)', opacity: 0 },
          { transform: 'translateY(0%)', opacity: 1 },
        ],
        [
          {
            transform: `${per} rotateX(0deg) translateY(0%)`,
            transformOrigin: '50% 100%',
            opacity: 1,
          },
          {
            transform: `${per} rotateX(60deg) translateY(30%)`,
            transformOrigin: '50% 100%',
            opacity: 0,
          },
        ],
        true,
      );
    case 'crush':
      return plan(
        null,
        [
          { transform: 'scale(1) rotate(0deg)', filter: 'blur(0px)', opacity: 1 },
          { transform: `scale(0) rotate(${sign * 25}deg)`, filter: 'blur(4px)', opacity: 0.8 },
        ],
        true,
      );
    case 'peelOff': {
      const corner = (s: number): [number, number][] =>
        sign > 0
          ? [
              [0, 0],
              [s, 0],
              [0, s],
            ]
          : [
              [1, 0],
              [1 - s, 0],
              [1, s],
            ];
      return plan(
        null,
        [
          { clipPath: polygon(corner(2)), transform: 'translate(0%, 0%)' },
          { clipPath: polygon(corner(0)), transform: `translate(${sign * -10}%, -10%)` },
        ],
        true,
      );
    }
    case 'pageCurlSingle': {
      // The page lifts from the bottom corner: what is left of it is a shrinking triangle.
      const left = (s: number): [number, number][] =>
        sign > 0
          ? [
              [0, 0],
              [s, 0],
              [0, s],
            ]
          : [
              [1, 0],
              [1 - s, 0],
              [1, s],
            ];
      return plan(null, [{ clipPath: polygon(left(2)) }, { clipPath: polygon(left(0)) }], true);
    }
    case 'pageCurlDouble':
      return plan(
        null,
        rectFrames((p) => [
          [0, 0, 0.5 * (1 - p), 1],
          [0.5 + 0.5 * p, 0, 1, 1],
        ]),
        true,
      );
    case 'airplane':
      return plan(
        null,
        [
          { transform: 'translate(0%, 0%) rotate(0deg) scale(1)', opacity: 1, offset: 0 },
          { transform: 'translate(0%, 0%) rotate(0deg) scale(0.3)', opacity: 1, offset: 0.4 },
          {
            transform: `translate(${sign * 90}%, -90%) rotate(${sign * -30}deg) scale(0.1)`,
            opacity: 0,
            offset: 1,
          },
        ],
        true,
      );
    case 'origami':
      return plan(
        null,
        [
          { transform: 'scale(1) rotate(0deg)', opacity: 1, offset: 0 },
          { transform: 'scale(0.5) rotate(0deg)', opacity: 1, offset: 0.5 },
          { transform: `scale(0.05) rotate(${sign * 45}deg)`, opacity: 0, offset: 1 },
        ],
        true,
      );
    default:
      return plan(fadeIn, null);
  }
}

/** Ranks of `values` (smallest first). */
function rankBy(values: number[]): number[] {
  const order = values.map((v, i) => [v, i] as const).sort((a, b) => a[0] - b[0]);
  const rank = new Array<number>(values.length).fill(0);
  order.forEach(([, i], r) => {
    rank[i] = r;
  });
  return rank;
}

/** One layer of the morph overlay: a display list and its keyframes. */
export interface MorphLayer {
  page: PageDisplayList;
  frames: Frames;
  /** A backdrop (drawn on paper; the others are see-through). */
  backdrop?: boolean;
  /** A word / letter: drawn on a canvas the size of this box `[x, y, w, h]` (points). */
  rect?: [number, number, number, number];
  /** The outlines of a matched shape changing point by point (drawn instead of `page`). */
  outline?: SlideMorphOutline;
}

/** One path of a morphing outline at progress `k` (0 = previous slide, 1 = this one). */
export interface OutlineFrame {
  d: string;
  fill: string | null;
  stroke: string | null;
  width: number;
}

const css = (c: [number, number, number, number]) =>
  `rgba(${c[0]}, ${c[1]}, ${c[2]}, ${Math.round((c[3] / 255) * 1000) / 1000})`;

/**
 * The outlines of a morph at progress `k`: every point, the fill, the line colour and width
 * interpolated linearly between the two slides (a path missing a fill or line on one side takes
 * the other side's, fading in or out).
 */
export function outlineFrame(o: SlideMorphOutline, k: number): OutlineFrame[] {
  const lerp = (a: number, b: number) => a + (b - a) * k;
  const mix = (
    a: [number, number, number, number] | undefined,
    b: [number, number, number, number] | undefined,
  ): string | null => {
    if (!a && !b) return null;
    const x = a ?? ([...(b ?? [0, 0, 0, 0]).slice(0, 3), 0] as [number, number, number, number]);
    const y = b ?? ([...x.slice(0, 3), 0] as [number, number, number, number]);
    return css(
      [0, 1, 2, 3].map((i) => Math.round(lerp(x[i] ?? 0, y[i] ?? 0))) as [
        number,
        number,
        number,
        number,
      ],
    );
  };
  const r = (v: number) => Math.round(v * 100) / 100;
  return o.from.map((pa, pi) => {
    const pb = o.to[pi] ?? pa;
    const d = pa.rings
      .map((ring, ri) => {
        const other = pb.rings[ri] ?? ring;
        const pts = ring.map((p, i) => {
          const q = other[i] ?? p;
          return `${r(lerp(p[0], q[0]))} ${r(lerp(p[1], q[1]))}`;
        });
        return `M ${pts.join(' L ')}${pa.closed[ri] ? ' Z' : ''}`;
      })
      .join(' ');
    return {
      d,
      fill: mix(pa.fill, pb.fill),
      stroke: mix(pa.stroke?.[0], pb.stroke?.[0]),
      width: lerp(pa.stroke?.[1] ?? pb.stroke?.[1] ?? 0, pb.stroke?.[1] ?? pa.stroke?.[1] ?? 0),
    };
  });
}

/**
 * The morph overlay ([MS-PPTX] p159:morph) for a `width` × `height` point slide: the backdrops and
 * the unmatched shapes cross-fade; each matched shape is drawn twice — as on the previous slide,
 * carried to its new box, and as on this slide, carried from its old box — and the two cross-fade,
 * so position, size and rotation move and colour and shape blend. The box at time t is the linear
 * mix of the two boxes (rotation the short way), as `morph::interpolate` in the engine.
 */
export function morphLayers(m: SlideMorphInfo, width: number, height: number): MorphLayer[] {
  const W = width || 1;
  const H = height || 1;
  const out: Frames = [{ opacity: 1 }, { opacity: 0 }];
  const inn: Frames = [{ opacity: 0 }, { opacity: 1 }];
  const layers: MorphLayer[] = [
    { page: m.fromBackdrop, frames: out, backdrop: true },
    { page: m.toBackdrop, frames: inn, backdrop: true },
    ...m.gone.map((page) => ({ page, frames: out })),
    // Words / letters not found again (by word / by character).
    ...(m.goneText ?? []).map((p) => ({ page: p.page, frames: out, rect: p.rect })),
  ];
  const turn = (a: number, b: number) => {
    const d = (((b - a) % 360) + 360) % 360;
    return d > 180 ? d - 360 : d;
  };
  const pct = (v: number) => `${Math.round(v * 10000) / 100}%`;
  const t = (dx: number, dy: number, rot: number, sx: number, sy: number, undo: number) =>
    `translate(${pct(dx / W)}, ${pct(dy / H)}) rotate(${Math.round(rot * 1000) / 1000}deg) ` +
    `scale(${Math.round(sx * 10000) / 10000}, ${Math.round(sy * 10000) / 10000}) rotate(${Math.round(-undo * 1000) / 1000}deg)`;
  for (const p of m.pairs) {
    // Outlines changing point by point, under the shape's text.
    if (p.outline) layers.push({ page: p.fromPage, frames: [], outline: p.outline });
    const [fx, fy, fw, fh, fr] = p.from;
    const [tx, ty, tw, th, tr] = p.to;
    const d = turn(fr, tr);
    const sx = Math.max(tw, 0.01) / Math.max(fw, 0.01);
    const sy = Math.max(th, 0.01) / Math.max(fh, 0.01);
    const from = `${pct(fx / W)} ${pct(fy / H)}`;
    const to = `${pct(tx / W)} ${pct(ty / H)}`;
    // The box at time k as a transform of the shape drawn at its old (`fromSide`) or new box.
    const at = (k: number, fromSide: boolean) =>
      fromSide
        ? t((tx - fx) * k, (ty - fy) * k, fr + d * k, 1 + (sx - 1) * k, 1 + (sy - 1) * k, fr)
        : t(
            (fx - tx) * (1 - k),
            (fy - ty) * (1 - k),
            tr - d * (1 - k),
            1 / sx + (1 - 1 / sx) * k,
            1 / sy + (1 - 1 / sy) * k,
            tr,
          );
    const fade = [0, 0.35, 0.65, 1];
    layers.push({
      page: p.fromPage,
      frames: fade.map((k) => ({
        transform: at(k, true),
        transformOrigin: from,
        opacity: k < 0.5 ? 1 : 0,
        offset: k,
      })),
      ...(p.clip ? { rect: p.clip[0] } : {}),
    });
    layers.push({
      page: p.toPage,
      frames: fade.map((k) => ({
        transform: at(k, false),
        transformOrigin: to,
        opacity: k < 0.5 ? 0 : 1,
        offset: k,
      })),
      ...(p.clip ? { rect: p.clip[1] } : {}),
    });
  }
  for (const page of m.new) layers.push({ page, frames: inn });
  for (const p of m.newText ?? []) layers.push({ page: p.page, frames: inn, rect: p.rect });
  return layers;
}
