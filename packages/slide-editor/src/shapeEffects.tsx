/**
 * Shape effects and WordArt in BlueSlide (CORE-SHAPE-002): the "Effects" group of the
 * shape tab with the shared galleries of `@blueoffice/ui` (`ShapeEffectsMenu`,
 * `TextEffectsMenu`). Every gallery entry is a registry command mapped onto the engine's
 * `shapeEffects` command.
 */
import type { CommandRegistry } from '@blueoffice/editor';
import type { useTranslation } from '@blueoffice/i18n';
import type { SlideCommand } from '@blueoffice/platform';
import {
  Popover,
  ShapeEffectsMenu,
  TextEffectsMenu,
  WARP_PRESETS,
  allEffectIds,
  effectOpOf,
  type EffectLabel,
  type RibbonGroup,
  type WarpPreview,
} from '@blueoffice/ui';
import { Sparkles, Type } from 'lucide-react';
import type { ShapeToolState } from './shapeFormat';
import styles from './shapeFormat.module.css';

type T = ReturnType<typeof useTranslation>['t'];

/** Effects apply to every selected shape. */
const anyShape = (s: ShapeToolState) => !!s.isShape || !!s.mergeable;

/** Register the effect gallery commands; returns the unregister function. */
export function registerShapeEffects<S extends ShapeToolState>(
  reg: CommandRegistry<S>,
  run: (cmd: SlideCommand) => void,
): () => void {
  const off = allEffectIds(WARP_PRESETS).map((id) =>
    reg.register({
      id,
      isEnabled: anyShape,
      run: () => {
        const op = effectOpOf(id);
        if (op) run({ cmd: 'shapeEffects', op });
      },
    }),
  );
  return () => off.forEach((f) => f());
}

/** Warp thumbnails from the engine (`warpPreviews`). */
export function warpPreviewLoader(
  run: (cmd: SlideCommand) => Promise<{ warpPreviews?: WarpPreview[] } | null>,
): () => Promise<WarpPreview[] | null> {
  let cached: Promise<WarpPreview[] | null> | null = null;
  return () => {
    cached ??= run({ cmd: 'shapeEffects', op: { kind: 'warpPreviews' } }).then(
      (r) => r?.warpPreviews ?? null,
    );
    return cached;
  };
}

/** The "Effects" group of the shape tab. */
export function shapeEffectsGroup<S extends ShapeToolState>({
  t,
  state,
  exec,
  loadPreviews,
}: {
  t: T;
  state: S;
  exec: (id: string) => void;
  loadPreviews: () => Promise<WarpPreview[] | null>;
}): RibbonGroup {
  const label: EffectLabel = (key, vars) => t(`shapeEffects.${key}`, vars);
  const on = anyShape(state);
  const none = () => {};
  return {
    id: 'shapeEffects',
    label: t('shapeEffects.group'),
    items: [
      {
        id: 'shape.effects',
        label: label('shapeEffects'),
        onSelect: none,
        enabled: on,
        render: (
          <Popover
            trigger={
              <span className={styles.trigger}>
                <Sparkles size={18} aria-hidden="true" />
                <span>{label('shapeEffects')}</span>
              </span>
            }
            label={label('shapeEffects')}
            disabled={!on}
            testId="shape-effects-menu"
          >
            {(close) => <ShapeEffectsMenu label={label} exec={exec} close={close} />}
          </Popover>
        ),
      },
      {
        id: 'text.effects',
        label: label('textEffects'),
        onSelect: none,
        enabled: on,
        render: (
          <Popover
            trigger={
              <span className={styles.trigger}>
                <Type size={18} aria-hidden="true" />
                <span>{label('textEffects')}</span>
              </span>
            }
            label={label('textEffects')}
            disabled={!on}
            testId="text-effects-menu"
          >
            {(close) => (
              <TextEffectsMenu
                label={label}
                exec={exec}
                close={close}
                loadPreviews={loadPreviews}
              />
            )}
          </Popover>
        ),
      },
    ],
  };
}
