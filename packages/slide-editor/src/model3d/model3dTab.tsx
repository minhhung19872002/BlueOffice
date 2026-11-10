/**
 * The 3D model contextual tab (CORE-3DMODEL-001): preset views, reset, zoom, pan & zoom mode
 * and alternative text of the selected model, plus the alt text dialog.
 */
import type { CommandRegistry } from '@blueoffice/editor';
import {
  MODEL3D_PRESETS,
  type Model3dPreset,
  type SlideCommand,
  type SlideModel3dInfo,
} from '@blueoffice/platform';
import { Dialog, type RibbonTab } from '@blueoffice/ui';
import { Move, RotateCcw, Rotate3d, Text, ZoomIn, ZoomOut } from 'lucide-react';
import { useState, type ReactNode } from 'react';
import styles from '../SlideEditor.module.css';

type T = (key: string, vars?: Record<string, string | number>) => string;

type Item = (
  id: string,
  label: string,
  icon: ReactNode,
  size?: 'large' | 'small',
  checked?: boolean,
) => RibbonTab['groups'][number]['items'][number];

/** Zoom factor of one Zoom in / Zoom out press. */
export const ZOOM_PRESS = 1.2;

/** State the 3D model commands look at. */
export interface Model3dState {
  isModel3d?: boolean;
  /** Id of the selected model. */
  model3dId?: number;
  panZoom3d?: boolean;
}

/** Register the 3D model commands; returns the unregister function. */
export function registerModel3dCommands<S extends Model3dState>(
  reg: CommandRegistry<S>,
  run: (cmd: SlideCommand) => void,
  opts: { togglePanZoom: (state: S) => void; openAltText: () => void },
): () => void {
  const on = (s: S) => !!s.isModel3d;
  const off: (() => void)[] = [];
  const add = (id: string, exec: (s: S) => void, isChecked?: (s: S) => boolean) =>
    off.push(reg.register({ id, isEnabled: on, run: exec, isChecked }));
  for (const p of MODEL3D_PRESETS) {
    add(`model3d.preset.${p}`, () => run({ cmd: 'model3dPreset', preset: p }));
  }
  add('model3d.reset', () => run({ cmd: 'model3dReset' }));
  add('model3d.zoomIn', () => run({ cmd: 'model3dZoom', factor: ZOOM_PRESS }));
  add('model3d.zoomOut', () => run({ cmd: 'model3dZoom', factor: 1 / ZOOM_PRESS }));
  add('model3d.panZoom', opts.togglePanZoom, (s) => !!s.panZoom3d);
  add('model3d.altText', opts.openAltText);
  return () => {
    for (const f of off) f();
  };
}

const presetIcon = (p: Model3dPreset) => {
  // A small cube turned like the preset (own drawing).
  const turn: Record<Model3dPreset, string> = {
    front: 'rotate(0)',
    back: 'scale(-1,1)',
    left: 'skewY(-15)',
    right: 'skewY(15)',
    top: 'scale(1,0.6)',
    bottom: 'scale(1,-0.6)',
    aboveFrontLeft: 'rotate(-12)',
    aboveFrontRight: 'rotate(12)',
    belowFrontLeft: 'rotate(-12) scale(1,-1)',
    belowFrontRight: 'rotate(12) scale(1,-1)',
  };
  return (
    <svg width="16" height="16" viewBox="-8 -8 16 16" aria-hidden>
      <g transform={turn[p]}>
        <path d="M-5 -2 L0 -5 L5 -2 L5 4 L0 7 L-5 4 Z" fill="none" stroke="currentColor" />
        <path d="M-5 -2 L0 1 L5 -2 M0 1 L0 7" fill="none" stroke="currentColor" />
      </g>
    </svg>
  );
};

/** The contextual tab of a selected model. */
export function model3dTab(t: T, item: Item): RibbonTab {
  const sm = 'small' as const;
  return {
    id: 'model3d',
    context: 'model3d',
    label: t('slideEditor.model3d.tab'),
    groups: [
      {
        id: 'model3dViews',
        label: t('slideEditor.model3d.groupViews'),
        items: MODEL3D_PRESETS.map((p) =>
          item(`model3d.preset.${p}`, t(`slideEditor.model3d.presets.${p}`), presetIcon(p), sm),
        ),
      },
      {
        id: 'model3dAdjust',
        label: t('slideEditor.model3d.groupAdjust'),
        items: [
          item('model3d.reset', t('slideEditor.model3d.reset'), <RotateCcw size={18} />),
          item('model3d.panZoom', t('slideEditor.model3d.panZoom'), <Move size={18} />),
          item('model3d.zoomIn', t('slideEditor.model3d.zoomIn'), <ZoomIn size={16} />, sm),
          item('model3d.zoomOut', t('slideEditor.model3d.zoomOut'), <ZoomOut size={16} />, sm),
        ],
      },
      {
        id: 'model3dAccess',
        label: t('slideEditor.model3d.groupAccess'),
        items: [item('model3d.altText', t('slideEditor.model3d.altText'), <Text size={18} />)],
      },
    ],
  };
}

/** Icon of the Insert › 3D model button. */
export const Model3dIcon = () => <Rotate3d size={18} />;

/** Alternative text of the selected model. */
export function Model3dAltTextDialog({
  open,
  model,
  t,
  onClose,
  onSave,
}: {
  open: boolean;
  model: SlideModel3dInfo | undefined;
  t: T;
  onClose: () => void;
  onSave: (text: string) => void;
}) {
  // Mounted when opened: starts from the model's text.
  const [text, setText] = useState(model?.descr ?? '');
  return (
    <Dialog
      open={open}
      onOpenChange={(o) => !o && onClose()}
      title={t('slideEditor.model3d.altTextTitle')}
      testId="model3d-alt-dialog"
      footer={
        <>
          <button type="button" onClick={onClose}>
            {t('slideEditor.cancel')}
          </button>
          <button
            type="button"
            data-testid="model3d-alt-save"
            onClick={() => {
              onSave(text);
              onClose();
            }}
          >
            {t('slideEditor.ok')}
          </button>
        </>
      }
    >
      <label className={styles.model3dAltLabel}>
        {t('slideEditor.model3d.altTextLabel')}
        <textarea
          data-testid="model3d-alt-text"
          rows={4}
          value={text}
          onChange={(e) => setText(e.target.value)}
        />
      </label>
    </Dialog>
  );
}
