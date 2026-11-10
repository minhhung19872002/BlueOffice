import { describe, expect, it } from 'vitest';
import type { Model3dView } from '@blueoffice/platform';
import {
  DEGREES_PER_PX,
  Model3dViewController,
  ZOOM_STEP,
  decodeMesh,
  keyRotation,
} from '../src/model3d/controller';
import {
  ANGLE,
  EMU_PER_METER,
  EMU_PT,
  camera,
  clipMatrix,
  ndc,
  orbit,
  pan,
  rotation,
  toCamera,
  toFrame,
  toScene,
  zoom,
  type V3,
} from '../src/model3d/view';

/** `bo_model3d::view::View3d::fit([-1; 3], [1; 3])`: 2 units across → 1 m, 30° camera. */
function fitView(): Model3dView {
  const mpu = 0.5;
  const diag = Math.sqrt(3) * 2 * mpu * EMU_PER_METER;
  const dist = Math.round(diag / 2 / Math.sin((15 * Math.PI) / 180));
  return {
    cameraPos: [0, 0, dist],
    cameraUp: [0, EMU_PER_METER, 0],
    lookAt: [0, 0, 0],
    fov: 30 * ANGLE,
    orthoSize: EMU_PER_METER,
    meterPerUnit: mpu,
    preTrans: [0, 0, 0],
    scale: [1, 1, 1],
    rot: [0, 0, 0],
    postTrans: [0, 0, 0],
    viewport: null,
  };
}

/** Frame position of a model vertex through the view formulas. */
function framePos(v: Model3dView, p: V3, w: number, h: number): [number, number] {
  const c = camera(v);
  const q = ndc(c, toCamera(c, toScene(v, rotation(v), p)));
  if (!q) throw new Error('behind the camera');
  return toFrame(v, q, w, h);
}

describe('3D model view math (mirror of bo_model3d::view)', () => {
  it('turns about y then x and wraps the angles', () => {
    const v = orbit(fitView(), 90, 0);
    expect(v.rot).toEqual([0, 90 * ANGLE, 0]);
    // +x turns to −z about y.
    const p = toScene({ ...v, meterPerUnit: 1 / EMU_PER_METER }, rotation(v), [1, 0, 0]);
    expect(p[2]).toBeCloseTo(-1, 9);
    const w = orbit(v, -450, -30);
    expect(w.rot[1]).toBe(0);
    expect(w.rot[0]).toBe(330 * ANGLE);
  });

  it('pans so the model follows the pointer', () => {
    const frame: [number, number] = [1_000_000, 1_000_000];
    const v = pan({ ...fitView(), viewport: 1_000_000 }, 100_000, 50_000, frame);
    const [fx, fy] = framePos(v, [0, 0, 0], frame[0], frame[1]);
    expect(Math.abs(fx - 600_000)).toBeLessThan(50);
    expect(Math.abs(fy - 550_000)).toBeLessThan(50);
  });

  it('zooms the viewport square', () => {
    const v = zoom({ ...fitView(), viewport: 2_000_000 }, 1.5, [1_000_000, 1_000_000]);
    expect(v.viewport).toBe(3_000_000);
    // A window viewport starts from the frame's longer side.
    expect(zoom(fitView(), 2, [400_000, 300_000]).viewport).toBe(800_000);
  });

  it('the WebGL matrix lands where the formulas put the vertex', () => {
    const v = { ...orbit(fitView(), 30, 20), viewport: 1_500_000 };
    const [w, h] = [1_200_000, 900_000];
    const m = clipMatrix(v, w, h);
    for (const p of [
      [0.5, 0.5, 0.5],
      [-1, 0.2, 0.7],
      [1, -1, -1],
    ] as V3[]) {
      const clip = [0, 1, 2, 3].map(
        (r) => m[r]! * p[0] + m[4 + r]! * p[1] + m[8 + r]! * p[2] + m[12 + r]!,
      );
      const cx = clip[0]! / clip[3]!;
      const cy = clip[1]! / clip[3]!;
      const cz = clip[2]! / clip[3]!;
      const [fx, fy] = framePos(v, p, w, h);
      expect((cx + 1) * 0.5 * w).toBeCloseTo(fx, 0);
      expect((1 - cy) * 0.5 * h).toBeCloseTo(fy, 0);
      expect(cz).toBeGreaterThan(-1);
      expect(cz).toBeLessThan(1);
    }
  });
});

describe('Model3dViewController', () => {
  const frame: [number, number] = [2_000_000, 2_000_000];

  it('a drag on the handle turns the model; release gives the view to commit', () => {
    const c = new Model3dViewController(fitView(), frame, 1);
    expect(c.end()).toBeNull();
    c.begin('rotate', 100, 100);
    expect(c.dragging).toBe(true);
    c.move(140, 100);
    const v = c.move(160, 80);
    expect(v.rot[1]).toBe(60 * DEGREES_PER_PX * ANGLE);
    expect(v.rot[0]).toBe(360 * ANGLE - 20 * DEGREES_PER_PX * ANGLE);
    const done = c.end();
    expect(done).toEqual(v);
    expect(c.dirty).toBe(true);
    c.commit();
    expect(c.dirty).toBe(false);
  });

  it('Escape returns to the view at the start of the drag', () => {
    const start = fitView();
    const c = new Model3dViewController(start, frame, 2);
    c.begin('rotate', 0, 0);
    c.move(50, 50);
    expect(c.cancel()).toEqual(start);
    expect(c.dragging).toBe(false);
  });

  it('pan mode moves the camera by the dragged points; the wheel zooms', () => {
    const c = new Model3dViewController({ ...fitView(), viewport: 2_000_000 }, frame, 2);
    c.begin('pan', 10, 10);
    // 20 CSS px at 2 px/pt = 10 pt.
    const v = c.move(30, 10);
    const ref = pan({ ...fitView(), viewport: 2_000_000 }, 10 * EMU_PT, 0, frame);
    expect(v.cameraPos).toEqual(ref.cameraPos);
    c.end();
    const z = c.wheel(-100);
    expect(z.viewport).toBe(Math.round(2_000_000 * ZOOM_STEP));
  });

  it('arrow keys turn by 5° (15° with Shift)', () => {
    expect(keyRotation('ArrowLeft', false)).toEqual([-5, 0]);
    expect(keyRotation('ArrowDown', true)).toEqual([0, 15]);
    expect(keyRotation('a', false)).toBeNull();
  });

  it('decodes the engine mesh arrays', () => {
    const f = new Float32Array([1.5, -2, 0.25]);
    const i = new Uint32Array([0, 1, 70_000]);
    const b64 = (b: ArrayBufferLike) => btoa(String.fromCharCode(...new Uint8Array(b)));
    const m = decodeMesh({
      positions: b64(f.buffer),
      normals: b64(f.buffer),
      colors: btoa('ÿ\u0010 ÿ'),
      indices: b64(i.buffer),
    });
    expect([...m.positions]).toEqual([1.5, -2, 0.25]);
    expect([...m.indices]).toEqual([0, 1, 70_000]);
    expect([...m.colors]).toEqual([255, 16, 32, 255]);
  });
});
