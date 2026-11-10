/**
 * Interaction with a 3D model in its frame (CORE-3DMODEL-001): dragging the rotation handle
 * turns it, dragging in pan & zoom mode moves it inside the frame, the wheel zooms. The
 * controller only computes views (no DOM); the layer draws them live and commits the last one
 * as one `model3dView` command.
 */
import type { Model3dView } from '@blueoffice/platform';
import { EMU_PT, orbit, pan, zoom } from './view';

export type Model3dDragMode = 'rotate' | 'pan';

/** Degrees turned per CSS pixel dragged. */
export const DEGREES_PER_PX = 0.5;
/** Zoom per wheel notch (100 delta units). */
export const ZOOM_STEP = 1.1;

export class Model3dViewController {
  private start: { x: number; y: number; view: Model3dView; mode: Model3dDragMode } | null = null;
  private current: Model3dView;
  private changed = false;

  /**
   * @param view the stored view
   * @param frame frame size in EMU
   * @param pxPerPt CSS pixels per point on screen
   */
  constructor(
    view: Model3dView,
    private readonly frame: [number, number],
    private readonly pxPerPt: number,
  ) {
    this.current = view;
  }

  /** The view shown now. */
  get view(): Model3dView {
    return this.current;
  }

  /** Has the view changed since construction / the last {@link commit}? */
  get dirty(): boolean {
    return this.changed;
  }

  get dragging(): boolean {
    return this.start !== null;
  }

  begin(mode: Model3dDragMode, x: number, y: number): void {
    this.start = { x, y, view: this.current, mode };
  }

  /** Pointer at (x, y) CSS px; returns the view to show. */
  move(x: number, y: number): Model3dView {
    const s = this.start;
    if (!s) return this.current;
    const dx = x - s.x;
    const dy = y - s.y;
    if (s.mode === 'rotate') {
      // Right turns the model to its right, down tips its top towards the viewer.
      this.current = orbit(s.view, dx * DEGREES_PER_PX, dy * DEGREES_PER_PX);
    } else {
      const k = EMU_PT / Math.max(1e-6, this.pxPerPt);
      this.current = pan(s.view, dx * k, dy * k, this.frame);
    }
    if (dx !== 0 || dy !== 0) this.changed = true;
    return this.current;
  }

  /** End of the drag; the view to commit, or null when nothing moved. */
  end(): Model3dView | null {
    this.start = null;
    return this.changed ? this.current : null;
  }

  /** Abandon the drag (Escape): back to the view at its start. */
  cancel(): Model3dView {
    if (this.start) this.current = this.start.view;
    this.start = null;
    return this.current;
  }

  /** Wheel `deltaY` (negative = away from the user = larger). */
  wheel(deltaY: number): Model3dView {
    if (!Number.isFinite(deltaY) || deltaY === 0) return this.current;
    const factor = ZOOM_STEP ** (-deltaY / 100);
    this.current = zoom(this.current, factor, this.frame);
    this.changed = true;
    return this.current;
  }

  /** The view was committed (or replaced by the engine's). */
  commit(view?: Model3dView): void {
    if (view) this.current = view;
    this.changed = false;
  }
}

/** Arrow keys on the focused rotation handle: degrees per press (Shift: 15°). */
export function keyRotation(key: string, shift: boolean): [number, number] | null {
  const d = shift ? 15 : 5;
  switch (key) {
    case 'ArrowLeft':
      return [-d, 0];
    case 'ArrowRight':
      return [d, 0];
    case 'ArrowUp':
      return [0, -d];
    case 'ArrowDown':
      return [0, d];
    default:
      return null;
  }
}

/** Decode the engine's base64 little-endian arrays. */
export function decodeMesh(m: {
  positions: string;
  normals: string;
  colors: string;
  indices: string;
}): { positions: Float32Array; normals: Float32Array; colors: Uint8Array; indices: Uint32Array } {
  const bytes = (s: string) => {
    const bin = atob(s);
    const out = new Uint8Array(bin.length);
    for (let i = 0; i < bin.length; i++) out[i] = bin.charCodeAt(i);
    return out;
  };
  const f32 = (s: string) => {
    const b = bytes(s);
    const dv = new DataView(b.buffer);
    const out = new Float32Array(b.length / 4);
    for (let i = 0; i < out.length; i++) out[i] = dv.getFloat32(i * 4, true);
    return out;
  };
  const u32 = (s: string) => {
    const b = bytes(s);
    const dv = new DataView(b.buffer);
    const out = new Uint32Array(b.length / 4);
    for (let i = 0; i < out.length; i++) out[i] = dv.getUint32(i * 4, true);
    return out;
  };
  return {
    positions: f32(m.positions),
    normals: f32(m.normals),
    colors: bytes(m.colors),
    indices: u32(m.indices),
  };
}
