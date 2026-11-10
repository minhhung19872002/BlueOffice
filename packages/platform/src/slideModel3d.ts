/**
 * BlueSlide 3D models (CORE-3DMODEL-001): the commands of `bo_engine::slide_edit::model3d`
 * and their results. A model is stored as a GLB part referenced by [MS-ODRAWXML]
 * `am3d:model3d`; its view is the `bo_model3d::view::View3d` numbers (EMU, 60 000ths of a
 * degree). Insert a model with `EngineHost.slideInsertPicture` (`.glb`, `.gltf`, `.obj`,
 * `.stl`).
 */

/** View of a model (mirrors `bo_model3d::view::View3d`). */
export interface Model3dView {
  /** `camera/pos` (EMU). */
  cameraPos: [number, number, number];
  /** `camera/up`. */
  cameraUp: [number, number, number];
  /** `camera/lookAt` (EMU). */
  lookAt: [number, number, number];
  /** `perspective@fov` (60 000ths of a degree); `null` = orthographic. */
  fov: number | null;
  /** `orthographic/sz` (EMU across). */
  orthoSize: number;
  /** `trans/meterPerModelUnit`. */
  meterPerUnit: number;
  /** `trans/preTrans` (EMU). */
  preTrans: [number, number, number];
  /** `trans/scale`. */
  scale: [number, number, number];
  /** `trans/rot` ax, ay, az (60 000ths of a degree). */
  rot: [number, number, number];
  /** `trans/postTrans` (EMU). */
  postTrans: [number, number, number];
  /** `objViewport@viewportSz` (EMU); `null` = window viewport. */
  viewport: number | null;
}

/** Preset views (`model3dPreset`). */
export const MODEL3D_PRESETS = [
  'front',
  'back',
  'left',
  'right',
  'top',
  'bottom',
  'aboveFrontLeft',
  'aboveFrontRight',
  'belowFrontLeft',
  'belowFrontRight',
] as const;
export type Model3dPreset = (typeof MODEL3D_PRESETS)[number];

export type SlideModel3dCommand =
  /** The selected model → `model3d`. */
  | { cmd: 'model3dInfo' }
  /** Geometry for the interactive viewer → `model3dMesh`. */
  | { cmd: 'model3dMesh' }
  /** Set the whole view (one undo step; the picture is rendered again). */
  | { cmd: 'model3dView'; view: Model3dView }
  /** Turn by `dx` degrees about the vertical axis, `dy` about the horizontal one. */
  | { cmd: 'model3dRotate'; dx: number; dy: number }
  | { cmd: 'model3dPreset'; preset: Model3dPreset }
  /** Front view, whole model in the frame. */
  | { cmd: 'model3dReset' }
  /** Zoom within the frame (> 1 larger). */
  | { cmd: 'model3dZoom'; factor: number }
  /** Pan within the frame (points). */
  | { cmd: 'model3dPan'; dx: number; dy: number }
  /** Render the picture again. */
  | { cmd: 'model3dRefresh' }
  /** Alternative text (empty removes it). */
  | { cmd: 'model3dAltText'; text: string };

/** The selected model (`model3d` of a slide result). */
export interface SlideModel3dInfo {
  id: number;
  name: string;
  descr: string | null;
  /** Frame `[x, y, w, h]` in slide points. */
  rect: [number, number, number, number];
  /** Frame size in EMU. */
  frameEmu: [number, number];
  /** GLB part (bytes via `EngineHost.getMedia`). */
  part: string | null;
  view: Model3dView;
  triangles: number;
}

/** Geometry for the viewer: little-endian arrays, base64 (`model3dMesh`). */
export interface SlideModel3dMesh {
  /** `f32 × 3` per vertex, model units. */
  positions: string;
  /** `f32 × 3` per vertex. */
  normals: string;
  /** `u8 × 4` per vertex (sRGB). */
  colors: string;
  /** `u32 × 3` per triangle. */
  indices: string;
  vertices: number;
  triangles: number;
}

/** File extensions of models BlueSlide inserts. */
export const MODEL3D_FILES = ['.glb', '.gltf', '.obj', '.stl'];
