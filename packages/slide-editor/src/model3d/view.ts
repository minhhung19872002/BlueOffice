/**
 * 3D model view math (CORE-3DMODEL-001), the TypeScript mirror of `bo_model3d::view`: the
 * scene transform (`am3d:trans`), the camera (`am3d:camera`) and the viewport square
 * (`am3d:objViewport@viewportSz`) of [MS-ODRAWXML] `am3d:model3d`. Units as stored: EMU,
 * 60 000ths of a degree. The engine renders the saved picture with the same formulas, so the
 * live WebGL view and the picture after a drag agree.
 */
import type { Model3dView } from '@blueoffice/platform';

export type V3 = [number, number, number];
export type M3 = [V3, V3, V3];

/** EMU per metre. */
export const EMU_PER_METER = 36_000_000;
/** `ST_Angle` units per degree. */
export const ANGLE = 60_000;
/** EMU per point. */
export const EMU_PT = 12_700;

const dot = (a: V3, b: V3) => a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
const sub = (a: V3, b: V3): V3 => [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
const add = (a: V3, b: V3): V3 => [a[0] + b[0], a[1] + b[1], a[2] + b[2]];
const mulS = (a: V3, s: number): V3 => [a[0] * s, a[1] * s, a[2] * s];
const cross = (a: V3, b: V3): V3 => [
  a[1] * b[2] - a[2] * b[1],
  a[2] * b[0] - a[0] * b[2],
  a[0] * b[1] - a[1] * b[0],
];

export function unit(a: V3, fallback: V3): V3 {
  const l = Math.sqrt(dot(a, a));
  return l > 1e-12 && Number.isFinite(l) ? mulS(a, 1 / l) : fallback;
}

function mul(a: M3, b: M3): M3 {
  const o: M3 = [
    [0, 0, 0],
    [0, 0, 0],
    [0, 0, 0],
  ];
  for (let i = 0; i < 3; i++)
    for (let j = 0; j < 3; j++) {
      let s = 0;
      for (let k = 0; k < 3; k++) s += a[i]![k]! * b[k]![j]!;
      o[i]![j] = s;
    }
  return o;
}

export function apply(m: M3, v: V3): V3 {
  return [dot(m[0], v), dot(m[1], v), dot(m[2], v)];
}

const rad = (a: number) => ((a / ANGLE) * Math.PI) / 180;

/** `R = Rx(ax) · Ry(ay) · Rz(az)`. */
export function rotation(v: Model3dView): M3 {
  const [a, b, c] = v.rot.map(rad) as V3;
  const rx: M3 = [
    [1, 0, 0],
    [0, Math.cos(a), -Math.sin(a)],
    [0, Math.sin(a), Math.cos(a)],
  ];
  const ry: M3 = [
    [Math.cos(b), 0, Math.sin(b)],
    [0, 1, 0],
    [-Math.sin(b), 0, Math.cos(b)],
  ];
  const rz: M3 = [
    [Math.cos(c), -Math.sin(c), 0],
    [Math.sin(c), Math.cos(c), 0],
    [0, 0, 1],
  ];
  return mul(rx, mul(ry, rz));
}

/** Scene position (EMU) of a model vertex. */
export function toScene(v: Model3dView, r: M3, p: V3): V3 {
  const k = v.meterPerUnit * EMU_PER_METER;
  const q: V3 = [
    (p[0] * k + v.preTrans[0]) * v.scale[0],
    (p[1] * k + v.preTrans[1]) * v.scale[1],
    (p[2] * k + v.preTrans[2]) * v.scale[2],
  ];
  return add(apply(r, q), v.postTrans);
}

export interface Camera {
  pos: V3;
  right: V3;
  up: V3;
  fwd: V3;
  /** `tan(fov / 2)`; null = orthographic. */
  tanHalf: number | null;
  half: number;
  dist: number;
}

export function camera(v: Model3dView): Camera {
  const fwd = unit(sub(v.lookAt, v.cameraPos), [0, 0, -1]);
  const up0 = unit(v.cameraUp, [0, 1, 0]);
  let right = cross(fwd, up0);
  if (dot(right, right) < 1e-12) right = cross(fwd, [0, 0, 1]);
  right = unit(right, [1, 0, 0]);
  const up = cross(right, fwd);
  const d = sub(v.lookAt, v.cameraPos);
  const fovDeg = v.fov === null ? null : Math.min(179, Math.max(1, v.fov / ANGLE));
  return {
    pos: v.cameraPos,
    right,
    up,
    fwd,
    tanHalf: fovDeg === null ? null : Math.tan((fovDeg * Math.PI) / 360),
    half: Math.max(1, v.orthoSize / 2),
    dist: Math.max(1, Math.sqrt(dot(d, d))),
  };
}

export function toCamera(c: Camera, p: V3): V3 {
  const d = sub(p, c.pos);
  return [dot(d, c.right), dot(d, c.up), dot(d, c.fwd)];
}

/** Camera-square position (−1..1, y up); null behind the eye. */
export function ndc(c: Camera, p: V3): [number, number] | null {
  if (c.tanHalf !== null) {
    if (p[2] <= c.dist * 1e-3) return null;
    return [p[0] / (p[2] * c.tanHalf), p[1] / (p[2] * c.tanHalf)];
  }
  return [p[0] / c.half, p[1] / c.half];
}

/** Side of the camera square on the page (EMU) for a frame `w × h` EMU. */
export function square(v: Model3dView, w: number, h: number): number {
  return v.viewport !== null && v.viewport > 0 ? v.viewport : Math.max(w, h);
}

/** Frame position (EMU from the top left) of camera-square coordinates. */
export function toFrame(
  v: Model3dView,
  p: [number, number],
  w: number,
  h: number,
): [number, number] {
  const s = square(v, w, h) / 2;
  return [w / 2 + p[0] * s, h / 2 - p[1] * s];
}

const wrap = (a: number) => Math.round(((a % (360 * ANGLE)) + 360 * ANGLE) % (360 * ANGLE));

/** Turn by `dx` degrees about the vertical axis and `dy` about the horizontal one. */
export function orbit(v: Model3dView, dx: number, dy: number): Model3dView {
  return {
    ...v,
    rot: [wrap(v.rot[0] + dy * ANGLE), wrap(v.rot[1] + dx * ANGLE), v.rot[2]],
  };
}

/** Zoom by `factor` (the viewport square grows). */
export function zoom(v: Model3dView, factor: number, frame: [number, number]): Model3dView {
  if (!Number.isFinite(factor) || factor <= 0) return v;
  const s = square(v, frame[0], frame[1]);
  const max = Math.max(frame[0], frame[1], 1) * 50;
  return { ...v, viewport: Math.round(Math.min(max, Math.max(1000, s * factor))) };
}

/** Pan by (`dx`, `dy`) EMU on the page (y down): the model follows the pointer. */
export function pan(v: Model3dView, dx: number, dy: number, frame: [number, number]): Model3dView {
  const c = camera(v);
  const sq = Math.max(1, square(v, frame[0], frame[1]));
  const across = c.tanHalf !== null ? 2 * c.dist * c.tanHalf : 2 * c.half;
  const k = across / sq;
  const shift = add(mulS(c.right, -dx * k), mulS(c.up, dy * k));
  const round = (p: V3): V3 => [Math.round(p[0]), Math.round(p[1]), Math.round(p[2])];
  return { ...v, cameraPos: round(add(v.cameraPos, shift)), lookAt: round(add(v.lookAt, shift)) };
}

/**
 * Column-major 4 × 4 matrix mapping a model vertex to WebGL clip space for a canvas covering
 * the frame (`w × h` EMU): x, y as {@link toFrame}, depth over [near, far] of the camera.
 */
export function clipMatrix(v: Model3dView, w: number, h: number): Float32Array {
  const r = rotation(v);
  const c = camera(v);
  const k = v.meterPerUnit * EMU_PER_METER;
  // Scene = A · vertex + b.
  const a: M3 = [
    [r[0][0] * v.scale[0] * k, r[0][1] * v.scale[1] * k, r[0][2] * v.scale[2] * k],
    [r[1][0] * v.scale[0] * k, r[1][1] * v.scale[1] * k, r[1][2] * v.scale[2] * k],
    [r[2][0] * v.scale[0] * k, r[2][1] * v.scale[1] * k, r[2][2] * v.scale[2] * k],
  ];
  const pre: V3 = [
    v.preTrans[0] * v.scale[0],
    v.preTrans[1] * v.scale[1],
    v.preTrans[2] * v.scale[2],
  ];
  const b = sub(add(apply(r, pre), v.postTrans), c.pos);
  // Camera = C · (A · vertex + b): rows right, up, fwd.
  const rows = [c.right, c.up, c.fwd];
  const cam = rows.map((row) => [
    row[0] * a[0][0] + row[1] * a[1][0] + row[2] * a[2][0],
    row[0] * a[0][1] + row[1] * a[1][1] + row[2] * a[2][1],
    row[0] * a[0][2] + row[1] * a[1][2] + row[2] * a[2][2],
    dot(row, b),
  ]) as [number[], number[], number[]];
  const sq = square(v, w, h);
  const sx = sq / Math.max(1, w);
  const sy = sq / Math.max(1, h);
  const near = c.dist * 0.01;
  const far = c.dist * 100;
  // Rows of the projection applied to (cx, cy, cz, 1).
  let p: number[][];
  if (c.tanHalf !== null) {
    const t = c.tanHalf;
    p = [
      [sx / t, 0, 0, 0],
      [0, sy / t, 0, 0],
      [0, 0, (far + near) / (far - near), (-2 * far * near) / (far - near)],
      [0, 0, 1, 0],
    ];
  } else {
    p = [
      [sx / c.half, 0, 0, 0],
      [0, sy / c.half, 0, 0],
      [0, 0, 2 / (far - near), -(far + near) / (far - near)],
      [0, 0, 0, 1],
    ];
  }
  // Full = P · [cam; 0 0 0 1].
  const camH = [...cam, [0, 0, 0, 1]];
  const out = new Float32Array(16);
  for (let row = 0; row < 4; row++)
    for (let col = 0; col < 4; col++) {
      let s = 0;
      for (let kk = 0; kk < 4; kk++) s += p[row]![kk]! * camH[kk]![col]!;
      out[col * 4 + row] = s;
    }
  return out;
}

/** Normal matrix (scene directions of model normals, before normalising): `R · S⁻¹`. */
export function normalMatrix(v: Model3dView): Float32Array {
  const r = rotation(v);
  const s = v.scale.map((x) => (Math.abs(x) > 1e-12 ? x : 1e-12));
  const out = new Float32Array(9);
  for (let col = 0; col < 3; col++)
    for (let row = 0; row < 3; row++) out[col * 3 + row] = r[row]![col]! / s[col]!;
  return out;
}

/** Light directions (towards the light) fixed to the camera, as the engine's renderer. */
export function lights(v: Model3dView): { key: V3; fill: V3; eye: V3 } {
  const c = camera(v);
  const mix = (r: number, u: number, f: number): V3 =>
    unit(add(add(mulS(c.right, r), mulS(c.up, u)), mulS(c.fwd, f)), [0, 0, 1]);
  return { key: mix(-0.45, 0.55, -0.7), fill: mix(0.6, -0.25, -0.75), eye: mulS(c.fwd, -1) };
}
