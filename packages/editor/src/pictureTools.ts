/**
 * BlueDoc picture tools (CORE-PIC-001): labels of the shared "Picture tools" tab and the
 * registry commands behind its buttons. The tab itself is `usePictureTools` of `@blueoffice/ui`;
 * edits are `picture*` engine commands (one undo step each).
 */
import { PICTURE_COMMANDS, type PictureActions, type PictureTabLabels } from '@blueoffice/ui';
import type { CommandRegistry } from './commands';

type T = (key: string, opts?: Record<string, unknown>) => string;

/** Name of a gallery preset (`lum.20.-40`, `sat.66`, `recolor.sepia`…). */
export function presetName(t: T, id: string): string {
  const [kind, a, b] = id.split('.');
  switch (kind) {
    case 'lum':
      return t('pictureTools.preset.lum', { b: a, c: b });
    case 'sat':
      return t('pictureTools.preset.sat', { n: a });
    case 'alpha':
      return t('pictureTools.preset.alpha', { n: a });
    default:
      return t(`pictureTools.preset.${id}`);
  }
}

/** Labels of the picture tools tab, galleries and dialogs. */
export function pictureTabLabels(t: T): PictureTabLabels {
  const k = (s: string) => t(`pictureTools.${s}`);
  return {
    tab: k('tab'),
    groupAdjust: k('groupAdjust'),
    groupSize: k('groupSize'),
    change: k('change'),
    resetPicture: k('resetPicture'),
    resetSize: k('resetSize'),
    compress: k('compress'),
    toShapes: k('toShapes'),
    adjust: k('adjust'),
    corrections: k('corrections'),
    sharpness: k('sharpness'),
    brightnessContrast: k('brightnessContrast'),
    color: k('color'),
    saturation: k('saturation'),
    recolor: k('recolor'),
    transparency: k('transparency'),
    crop: k('crop'),
    cropAspect: k('cropAspect'),
    cropShape: k('cropShape'),
    fill: k('fill'),
    fit: k('fit'),
    customCrop: k('customCrop'),
    options: k('options'),
    loading: k('loading'),
    preset: (id) => presetName(t, id),
    shape: (s) => t(`pictureTools.shape.${s}`),
    ok: k('ok'),
    cancel: k('cancel'),
    apply: k('apply'),
    adjustTitle: k('adjustTitle'),
    bright: k('bright'),
    contrast: k('contrast'),
    sharpen: k('sharpen'),
    alpha: k('alpha'),
    reset: k('reset'),
    cropTitle: k('cropTitle'),
    left: k('left'),
    top: k('top'),
    right: k('right'),
    bottom: k('bottom'),
    compressTitle: k('compressTitle'),
    resolution: k('resolution'),
    keepResolution: k('keepResolution'),
    ppi: (n) => t('pictureTools.ppi', { n }),
    dropCropped: k('dropCropped'),
    jpegQuality: k('jpegQuality'),
    scopeAll: k('scopeAll'),
    current: (i) => t('pictureTools.current', i),
    saved: (size) => t('pictureTools.saved', { size }),
    nothingSaved: k('nothingSaved'),
    // CORE-PIC-002 background removal mode.
    removeBackground: k('removeBackground'),
    markKeep: k('markKeep'),
    markRemove: k('markRemove'),
    undoMark: k('undoMark'),
    discard: k('discardChanges'),
    keepChanges: k('keepChanges'),
    computing: k('computing'),
    kept: (percent) => t('pictureTools.kept', { percent }),
    canvas: k('bgCanvas'),
  };
}

/**
 * Register the `picture.*` commands; they run the actions of the mounted picture tools
 * (`actions()` is read when a command runs).
 */
export function registerPictureCommands<S>(
  r: CommandRegistry<S>,
  isEnabled: (s: S) => boolean,
  actions: () => PictureActions | null,
): void {
  for (const id of PICTURE_COMMANDS) {
    r.register({ id, isEnabled, run: () => actions()?.[id]() });
  }
}
