import type { SlideCommand, SlideEditResult, SlideModel3dInfo } from '@blueoffice/platform';
import { Rotate3d } from 'lucide-react';
import { useEffect, useRef, useState } from 'react';
import styles from '../SlideEditor.module.css';
import { decodeMesh, keyRotation, Model3dViewController, type Model3dDragMode } from './controller';
import { GlModelView, type Mesh } from './glView';

export interface Model3dLayerProps {
  model: SlideModel3dInfo;
  /** CSS px per point. */
  scale: number;
  /** Pan & zoom mode: drags pan, the wheel zooms. */
  panZoom: boolean;
  run: (cmd: SlideCommand) => Promise<SlideEditResult | null>;
  labels: { rotate: string; panZoom: string };
}

/** Meshes fetched for the viewer, by GLB part. */
const meshes = new Map<string, Mesh>();

/**
 * The 3D rotation handle of the selected model and, while it is dragged (or the model is
 * panned / zoomed), a live WebGL view over the frame. The view is committed as one
 * `model3dView` when the drag ends; the engine renders the saved picture again.
 */
export function Model3dLayer({ model, scale, panZoom, run, labels }: Model3dLayerProps) {
  const [x, y, w, h] = model.rect;
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const ctl = useRef<Model3dViewController | null>(null);
  const gl = useRef<GlModelView | null>(null);
  const [live, setLive] = useState(false);
  const wheelTimer = useRef<ReturnType<typeof setTimeout> | null>(null);
  const viewKey = JSON.stringify(model.view);

  // A new stored view (undo, another command) restarts from it.
  useEffect(() => {
    ctl.current = new Model3dViewController(model.view, model.frameEmu, scale);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [viewKey, model.id, scale, model.frameEmu[0], model.frameEmu[1]]);

  useEffect(
    () => () => {
      if (wheelTimer.current) clearTimeout(wheelTimer.current);
    },
    [],
  );

  const ensureGl = async (): Promise<void> => {
    const canvas = canvasRef.current;
    if (!canvas || gl.current) return;
    const key = model.part ?? `#${model.id}`;
    let mesh = meshes.get(key);
    if (!mesh) {
      const r = await run({ cmd: 'model3dMesh' });
      if (!r?.model3dMesh) return;
      mesh = decodeMesh(r.model3dMesh);
      meshes.set(key, mesh);
    }
    const dpr = typeof window !== 'undefined' ? window.devicePixelRatio || 1 : 1;
    canvas.width = Math.max(1, Math.round(w * scale * dpr));
    canvas.height = Math.max(1, Math.round(h * scale * dpr));
    gl.current = GlModelView.create(canvas, mesh);
  };

  const draw = () => {
    const c = ctl.current;
    if (c && gl.current) gl.current.draw(c.view, model.frameEmu);
  };

  const commit = async () => {
    const c = ctl.current;
    if (!c?.dirty) {
      setLive(false);
      return;
    }
    const view = c.view;
    c.commit();
    await run({ cmd: 'model3dView', view });
    setLive(false);
  };

  const begin = (mode: Model3dDragMode, e: React.PointerEvent<HTMLElement>) => {
    e.stopPropagation();
    e.preventDefault();
    const c = ctl.current;
    if (!c) return;
    e.currentTarget.setPointerCapture?.(e.pointerId);
    c.begin(mode, e.clientX, e.clientY);
    setLive(true);
    void ensureGl().then(draw);
  };

  const move = (e: React.PointerEvent<HTMLElement>) => {
    const c = ctl.current;
    if (!c?.dragging) return;
    e.stopPropagation();
    c.move(e.clientX, e.clientY);
    draw();
  };

  const end = (e: React.PointerEvent<HTMLElement>) => {
    const c = ctl.current;
    if (!c?.dragging) return;
    e.stopPropagation();
    c.move(e.clientX, e.clientY);
    c.end();
    void commit();
  };

  const stop = (e: { stopPropagation: () => void }) => e.stopPropagation();

  return (
    <div
      className={styles.model3dLayer}
      data-testid="model3d-layer"
      data-pan-zoom={panZoom}
      data-rot={model.view.rot.join(',')}
      data-viewport={model.view.viewport ?? ''}
      style={{
        left: x * scale,
        top: y * scale,
        width: w * scale,
        height: h * scale,
        pointerEvents: panZoom ? 'auto' : 'none',
      }}
      onPointerDown={panZoom ? (e) => begin('pan', e) : undefined}
      onPointerMove={panZoom ? move : undefined}
      onPointerUp={panZoom ? end : undefined}
      onMouseDown={panZoom ? stop : undefined}
      onWheel={
        panZoom
          ? (e) => {
              e.stopPropagation();
              const c = ctl.current;
              if (!c) return;
              c.wheel(e.deltaY);
              setLive(true);
              void ensureGl().then(draw);
              if (wheelTimer.current) clearTimeout(wheelTimer.current);
              wheelTimer.current = setTimeout(() => void commit(), 400);
            }
          : undefined
      }
    >
      <canvas
        ref={canvasRef}
        className={styles.model3dCanvas}
        style={{ visibility: live ? 'visible' : 'hidden' }}
        aria-hidden
      />
      <button
        type="button"
        className={styles.model3dHandle}
        data-testid="model3d-rotate"
        aria-label={labels.rotate}
        title={labels.rotate}
        onPointerDown={(e) => begin('rotate', e)}
        onPointerMove={move}
        onPointerUp={end}
        onMouseDown={stop}
        onClick={stop}
        onDoubleClick={stop}
        onKeyDown={(e) => {
          const d = keyRotation(e.key, e.shiftKey);
          if (!d) return;
          e.preventDefault();
          e.stopPropagation();
          void run({ cmd: 'model3dRotate', dx: d[0], dy: d[1] });
        }}
      >
        <Rotate3d size={18} />
      </button>
      {panZoom && <span className={styles.model3dHint}>{labels.panZoom}</span>}
    </div>
  );
}
