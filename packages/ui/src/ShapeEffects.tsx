import { useEffect, useState, type ReactNode } from 'react';
import styles from './ShapeEffects.module.css';

/**
 * Shape effects and WordArt galleries (CORE-SHAPE-002), shared by the editors that draw
 * DrawingML shapes: "Shape effects ▾" (reflection, soft edges, 3-D rotation) and
 * "Text effects ▾" (warp gallery with thumbnails the engine draws, text outline, text
 * fill). Every entry runs a command id through `exec`; the ids are built by the helpers
 * below so the host can register matching commands. Presets, names and thumbnails are
 * BlueOffice's own; every string comes from the host (`label`).
 */

/** A reflection preset: fade length (fraction of the shape) and gap (points). */
export interface ReflectionPreset {
  id: string;
  size: number | null;
  distance: number;
}

/** Reflection gallery. */
export const REFLECTION_PRESETS: ReflectionPreset[] = [
  { id: 'none', size: null, distance: 0 },
  { id: 'short', size: 0.3, distance: 0 },
  { id: 'half', size: 0.55, distance: 0 },
  { id: 'long', size: 0.9, distance: 0 },
  { id: 'shortGap', size: 0.3, distance: 4 },
  { id: 'halfGap', size: 0.55, distance: 4 },
  { id: 'longGap', size: 0.9, distance: 8 },
];

/** Soft edge radii (points); 0 removes the effect. */
export const SOFT_EDGE_PRESETS: number[] = [0, 2.5, 5, 10, 25, 50];

/**
 * 3-D rotation gallery: §20.1.10.47 ST_PresetCameraType values (the names shown are the
 * host's own), `none` removes the scene.
 */
export const ROTATION_PRESETS: string[] = [
  'none',
  'isometricLeftDown',
  'isometricRightUp',
  'isometricTopUp',
  'isometricBottomDown',
  'isometricOffAxis1Left',
  'isometricOffAxis1Right',
  'perspectiveFront',
  'perspectiveLeft',
  'perspectiveRight',
  'perspectiveAbove',
  'perspectiveBelow',
  'perspectiveRelaxed',
  'obliqueTopLeft',
  'obliqueTopRight',
  'obliqueBottomLeft',
  'obliqueBottomRight',
];

/** Colours of the text outline / fill swatches (BlueOffice's own palette). */
export const TEXT_COLORS: { id: string; hex: string }[] = [
  { id: 'black', hex: '000000' },
  { id: 'white', hex: 'FFFFFF' },
  { id: 'red', hex: 'C00000' },
  { id: 'orange', hex: 'ED7D31' },
  { id: 'gold', hex: 'FFC000' },
  { id: 'green', hex: '00B050' },
  { id: 'blue', hex: '0070C0' },
  { id: 'purple', hex: '7030A0' },
];

/** Two-colour gradients of the text fill menu. */
export const TEXT_GRADIENTS: { id: string; from: string; to: string; angle: number }[] = [
  { id: 'sunrise', from: 'FFC000', to: 'C00000', angle: 90 },
  { id: 'ocean', from: '00B0F0', to: '002060', angle: 90 },
  { id: 'meadow', from: '92D050', to: '00703C', angle: 90 },
  { id: 'dusk', from: 'B4A7D6', to: '7030A0', angle: 0 },
  { id: 'steel', from: 'F2F2F2', to: '595959', angle: 90 },
];

/** Command ids of the galleries. */
export const effectIds = {
  reflection: (id: string) => `shape.effect.reflection.${id}`,
  softEdge: (r: number) => `shape.effect.softEdge.${r}`,
  rotation: (preset: string) => `shape.effect.rotation.${preset}`,
  warp: (preset: string) => `text.effect.warp.${preset}`,
  outline: (color: string) => `text.effect.outline.${color}`,
  fill: (id: string) => `text.effect.fill.${id}`,
};

/** A text warp thumbnail (SVG path in a 120 × 72 box, from the engine). */
export interface WarpPreview {
  name: string;
  d: string;
}

/** Host strings: `label(key, vars)` (keys listed in the i18n `shapeEffects` section). */
export type EffectLabel = (key: string, vars?: Record<string, string | number>) => string;

interface Entry {
  id: string;
  name: string;
  thumb: ReactNode;
}

function Section({
  title,
  entries,
  exec,
  close,
  testId,
  wide,
}: {
  title: string;
  entries: Entry[];
  exec: (id: string) => void;
  close: () => void;
  testId: string;
  wide?: boolean;
}) {
  return (
    <div className={styles.section}>
      <div className={styles.sectionTitle}>{title}</div>
      <div className={wide ? styles.gridWide : styles.grid} role="listbox" aria-label={title}>
        {entries.map((e) => (
          <button
            key={e.id}
            type="button"
            role="option"
            aria-selected={false}
            className={wide ? styles.thumbWide : styles.thumb}
            title={e.name}
            aria-label={e.name}
            data-command={e.id}
            data-testid={`${testId}-${e.id}`}
            onClick={() => {
              close();
              exec(e.id);
            }}
          >
            {e.thumb}
          </button>
        ))}
      </div>
    </div>
  );
}

const STROKE = 'var(--bo-text-muted)';
const ACCENT = 'var(--bo-accent)';

function reflectionThumb(p: ReflectionPreset, uid: string): ReactNode {
  const top = 6;
  const h = 18;
  const y2 = top + h + p.distance / 2;
  const grad = `${uid}-r-${p.id}`;
  return (
    <svg viewBox="0 0 48 48" width="40" height="40" aria-hidden="true">
      <rect x="10" y={top} width="28" height={h} rx="2" fill={ACCENT} />
      {p.size !== null && (
        <>
          <defs>
            <linearGradient id={grad} x1="0" y1="0" x2="0" y2="1">
              <stop offset="0" stopColor={ACCENT} stopOpacity="0.5" />
              <stop offset={p.size} stopColor={ACCENT} stopOpacity="0" />
            </linearGradient>
          </defs>
          <rect x="10" y={y2} width="28" height={h} rx="2" fill={`url(#${grad})`} />
        </>
      )}
    </svg>
  );
}

function softEdgeThumb(r: number, uid: string): ReactNode {
  const f = `${uid}-s-${String(r).replace('.', '_')}`;
  return (
    <svg viewBox="0 0 48 48" width="40" height="40" aria-hidden="true">
      {r > 0 && (
        <defs>
          <filter id={f} x="-20%" y="-20%" width="140%" height="140%">
            <feGaussianBlur stdDeviation={Math.min(6, r / 4 + 0.5)} />
          </filter>
        </defs>
      )}
      <rect
        x="8"
        y="10"
        width="32"
        height="28"
        rx="2"
        fill={ACCENT}
        filter={r > 0 ? `url(#${f})` : undefined}
      />
    </svg>
  );
}

/** A unit square seen by the camera: a rough sketch (orthographic turn) for the gallery. */
function rotationThumb(preset: string): ReactNode {
  const n = preset.toLowerCase();
  if (preset === 'none') {
    return (
      <svg viewBox="0 0 48 48" width="40" height="40" aria-hidden="true">
        <rect x="10" y="12" width="28" height="24" fill="none" stroke={STROKE} />
      </svg>
    );
  }
  const side = (a: string, b: string) => (n.includes(a) ? 1 : n.includes(b) ? -1 : 0);
  const v = side('top', 'bottom') + side('above', 'below');
  const hz = side('left', 'right');
  const oblique = n.includes('oblique');
  const lon = (n.includes('isometric') ? 45 : 20) * (hz || (n.includes('isometric') ? 1 : 0));
  const lat = (n.includes('isometric') ? 35 : 20) * (v || (n.includes('isometric') ? 1 : 0));
  const rad = (d: number) => (d * Math.PI) / 180;
  // Front face corners, turned about y (lon) then x (lat), projected orthographically.
  const pts = [
    [-14, -12],
    [14, -12],
    [14, 12],
    [-14, 12],
  ].map(([x = 0, y = 0]) => {
    if (oblique) return [24 + x, 24 + y];
    const x1 = x * Math.cos(rad(lon));
    const z1 = -x * Math.sin(rad(lon));
    const y1 = y * Math.cos(rad(lat)) - z1 * Math.sin(rad(lat));
    return [24 + x1, 24 + y1];
  });
  const d = `M${pts.map(([x = 0, y = 0]) => `${x.toFixed(1)} ${y.toFixed(1)}`).join('L')}Z`;
  const back = oblique
    ? pts.map(([x = 0, y = 0]) => [x - 5 * hz, y - 5 * v])
    : pts.map(([x = 0, y = 0]) => [x - 4 * Math.sin(rad(lon)), y - 4 * Math.sin(rad(lat))]);
  const db = `M${back.map(([x = 0, y = 0]) => `${x.toFixed(1)} ${y.toFixed(1)}`).join('L')}Z`;
  return (
    <svg viewBox="0 0 48 48" width="40" height="40" aria-hidden="true">
      <path d={db} fill="none" stroke={STROKE} strokeDasharray="2 2" />
      <path d={d} fill={ACCENT} fillOpacity="0.35" stroke={ACCENT} />
    </svg>
  );
}

let uidSeq = 0;
function useUid(): string {
  const [id] = useState(() => `bo-fx-${++uidSeq}`);
  return id;
}

/** "Shape effects ▾" popover content. */
export function ShapeEffectsMenu({
  label,
  exec,
  close,
}: {
  label: EffectLabel;
  exec: (id: string) => void;
  close: () => void;
}) {
  const uid = useUid();
  return (
    <div className={styles.gallery} data-testid="shape-effects-gallery">
      <Section
        title={label('reflection')}
        testId="fx-reflection"
        exec={exec}
        close={close}
        entries={REFLECTION_PRESETS.map((p) => ({
          id: effectIds.reflection(p.id),
          name: label(`reflectionPreset.${p.id}`),
          thumb: reflectionThumb(p, uid),
        }))}
      />
      <Section
        title={label('softEdges')}
        testId="fx-soft"
        exec={exec}
        close={close}
        entries={SOFT_EDGE_PRESETS.map((r) => ({
          id: effectIds.softEdge(r),
          name: r === 0 ? label('none') : label('points', { n: r }),
          thumb: softEdgeThumb(r, uid),
        }))}
      />
      <Section
        title={label('rotation3d')}
        testId="fx-rotation"
        exec={exec}
        close={close}
        entries={ROTATION_PRESETS.map((p) => ({
          id: effectIds.rotation(p),
          name: label(`camera.${p}`),
          thumb: rotationThumb(p),
        }))}
      />
    </div>
  );
}

const swatch = (hex: string, outline: boolean): ReactNode => (
  <svg viewBox="0 0 48 48" width="40" height="40" aria-hidden="true">
    <text
      x="24"
      y="34"
      textAnchor="middle"
      fontSize="28"
      fontWeight="700"
      fill={outline ? 'none' : `#${hex}`}
      stroke={outline ? `#${hex}` : STROKE}
      strokeWidth={outline ? 1.5 : 0.5}
    >
      A
    </text>
  </svg>
);

/** "Text effects ▾" popover content; `loadPreviews` asks the engine for the warp thumbnails. */
export function TextEffectsMenu({
  label,
  exec,
  close,
  loadPreviews,
}: {
  label: EffectLabel;
  exec: (id: string) => void;
  close: () => void;
  loadPreviews: () => Promise<WarpPreview[] | null>;
}) {
  const [previews, setPreviews] = useState<WarpPreview[] | null>(null);
  const uid = useUid();
  useEffect(() => {
    let live = true;
    loadPreviews()
      .then((p) => {
        if (live) setPreviews(p ?? []);
      })
      .catch(() => {
        if (live) setPreviews([]);
      });
    return () => {
      live = false;
    };
  }, [loadPreviews]);
  const warp: Entry[] = [
    {
      id: effectIds.warp('none'),
      name: label('noWarp'),
      thumb: (
        <svg viewBox="0 0 120 72" width="60" height="36" aria-hidden="true">
          <text x="60" y="46" textAnchor="middle" fontSize="30" fill={STROKE}>
            abc
          </text>
        </svg>
      ),
    },
    ...(previews ?? []).map((p) => ({
      id: effectIds.warp(p.name),
      name: label(`warp.${p.name}`),
      thumb: (
        <svg viewBox="0 0 120 72" width="60" height="36" aria-hidden="true">
          <path d={p.d} fill={ACCENT} />
        </svg>
      ),
    })),
  ];
  const gradThumb = (g: (typeof TEXT_GRADIENTS)[number]) => {
    const id = `${uid}-g-${g.id}`;
    const v = g.angle === 90;
    return (
      <svg viewBox="0 0 48 48" width="40" height="40" aria-hidden="true">
        <defs>
          <linearGradient id={id} x1="0" y1="0" x2={v ? '0' : '1'} y2={v ? '1' : '0'}>
            <stop offset="0" stopColor={`#${g.from}`} />
            <stop offset="1" stopColor={`#${g.to}`} />
          </linearGradient>
        </defs>
        <text x="24" y="36" textAnchor="middle" fontSize="32" fontWeight="700" fill={`url(#${id})`}>
          A
        </text>
      </svg>
    );
  };
  return (
    <div className={styles.gallery} data-testid="text-effects-gallery">
      <Section
        title={label('transform')}
        testId="fx-warp"
        exec={exec}
        close={close}
        wide
        entries={warp}
      />
      {previews === null && <div className={styles.placeholder}>{label('loading')}</div>}
      <Section
        title={label('textOutline')}
        testId="fx-outline"
        exec={exec}
        close={close}
        entries={[
          {
            id: effectIds.outline('none'),
            name: label('noOutline'),
            thumb: swatch('FFFFFF', false),
          },
          ...TEXT_COLORS.map((c) => ({
            id: effectIds.outline(c.hex),
            name: label(`color.${c.id}`),
            thumb: swatch(c.hex, true),
          })),
        ]}
      />
      <Section
        title={label('textFill')}
        testId="fx-fill"
        exec={exec}
        close={close}
        entries={[
          { id: effectIds.fill('auto'), name: label('autoFill'), thumb: swatch('000000', false) },
          { id: effectIds.fill('none'), name: label('noFill'), thumb: swatch('000000', true) },
          ...TEXT_COLORS.map((c) => ({
            id: effectIds.fill(c.hex),
            name: label(`color.${c.id}`),
            thumb: swatch(c.hex, false),
          })),
          ...TEXT_GRADIENTS.map((g) => ({
            id: effectIds.fill(`gradient.${g.id}`),
            name: label(`gradient.${g.id}`),
            thumb: gradThumb(g),
          })),
        ]}
      />
    </div>
  );
}

/** Engine edit of a gallery entry (same shape as the platform's `SlideEffectOp`). */
export type EffectOp =
  | { kind: 'reflection'; size?: number; distance?: number }
  | { kind: 'softEdge'; radius?: number }
  | { kind: 'rotation3d'; preset?: string }
  | { kind: 'textWarp'; preset?: string }
  | { kind: 'textOutline'; color?: string; width?: number }
  | {
      kind: 'textFill';
      fill:
        | { type: 'auto' }
        | { type: 'none' }
        | { type: 'solid'; color: string }
        | { type: 'gradient'; from: string; to: string; angle?: number };
    };

/**
 * The engine edit of a gallery command id (`shapeEffects` op), or `null` for unknown ids.
 * Shared so every editor maps the ids the same way.
 */
export function effectOpOf(id: string): EffectOp | null {
  const rest = (p: string) => (id.startsWith(p) ? id.slice(p.length) : null);
  let r: string | null;
  if ((r = rest('shape.effect.reflection.')) !== null) {
    const p = REFLECTION_PRESETS.find((x) => x.id === r);
    if (!p) return null;
    return p.size === null
      ? { kind: 'reflection' }
      : { kind: 'reflection', size: p.size, distance: p.distance };
  }
  if ((r = rest('shape.effect.softEdge.')) !== null) {
    const v = Number(r);
    if (!Number.isFinite(v)) return null;
    return v > 0 ? { kind: 'softEdge', radius: v } : { kind: 'softEdge' };
  }
  if ((r = rest('shape.effect.rotation.')) !== null) {
    return r === 'none' ? { kind: 'rotation3d' } : { kind: 'rotation3d', preset: r };
  }
  if ((r = rest('text.effect.warp.')) !== null) {
    return r === 'none' ? { kind: 'textWarp' } : { kind: 'textWarp', preset: r };
  }
  if ((r = rest('text.effect.outline.')) !== null) {
    return r === 'none' ? { kind: 'textOutline' } : { kind: 'textOutline', color: r, width: 1 };
  }
  if ((r = rest('text.effect.fill.')) !== null) {
    if (r === 'auto') return { kind: 'textFill', fill: { type: 'auto' } };
    if (r === 'none') return { kind: 'textFill', fill: { type: 'none' } };
    const g = rest('text.effect.fill.gradient.');
    if (g !== null) {
      const p = TEXT_GRADIENTS.find((x) => x.id === g);
      return p
        ? { kind: 'textFill', fill: { type: 'gradient', from: p.from, to: p.to, angle: p.angle } }
        : null;
    }
    return /^[0-9A-Fa-f]{6}$/.test(r)
      ? { kind: 'textFill', fill: { type: 'solid', color: r } }
      : null;
  }
  return null;
}

/** Every gallery command id (for registering them), warps given by name. */
export function allEffectIds(warps: readonly string[]): string[] {
  return [
    ...REFLECTION_PRESETS.map((p) => effectIds.reflection(p.id)),
    ...SOFT_EDGE_PRESETS.map((r) => effectIds.softEdge(r)),
    ...ROTATION_PRESETS.map((p) => effectIds.rotation(p)),
    effectIds.warp('none'),
    ...warps.map((w) => effectIds.warp(w)),
    effectIds.outline('none'),
    ...TEXT_COLORS.map((c) => effectIds.outline(c.hex)),
    effectIds.fill('auto'),
    effectIds.fill('none'),
    ...TEXT_COLORS.map((c) => effectIds.fill(c.hex)),
    ...TEXT_GRADIENTS.map((g) => effectIds.fill(`gradient.${g.id}`)),
  ];
}

/** §20.1.10.76 ST_TextShapeType values with a preset definition (annex order: sorted). */
export const WARP_PRESETS: readonly string[] = [
  'textArchDown',
  'textArchDownPour',
  'textArchUp',
  'textArchUpPour',
  'textButton',
  'textButtonPour',
  'textCanDown',
  'textCanUp',
  'textCascadeDown',
  'textCascadeUp',
  'textChevron',
  'textChevronInverted',
  'textCircle',
  'textCirclePour',
  'textCurveDown',
  'textCurveUp',
  'textDeflate',
  'textDeflateBottom',
  'textDeflateInflate',
  'textDeflateInflateDeflate',
  'textDeflateTop',
  'textDoubleWave1',
  'textFadeDown',
  'textFadeLeft',
  'textFadeRight',
  'textFadeUp',
  'textInflate',
  'textInflateBottom',
  'textInflateTop',
  'textPlain',
  'textRingInside',
  'textRingOutside',
  'textSlantDown',
  'textSlantUp',
  'textStop',
  'textTriangle',
  'textTriangleInverted',
  'textWave1',
  'textWave2',
  'textWave4',
];
