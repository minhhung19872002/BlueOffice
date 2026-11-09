// @vitest-environment happy-dom
import { act, cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import {
  REFLECTION_PRESETS,
  ROTATION_PRESETS,
  SOFT_EDGE_PRESETS,
  ShapeEffectsMenu,
  TEXT_COLORS,
  TEXT_GRADIENTS,
  TextEffectsMenu,
  WARP_PRESETS,
  allEffectIds,
  effectIds,
  effectOpOf,
} from '../src/ShapeEffects';

afterEach(cleanup);

const label = (k: string, v?: Record<string, string | number>) =>
  v ? `${k}:${Object.values(v).join(',')}` : k;

describe('effect command ids', () => {
  it('map onto engine edits', () => {
    expect(effectOpOf(effectIds.reflection('none'))).toEqual({ kind: 'reflection' });
    expect(effectOpOf(effectIds.reflection('halfGap'))).toEqual({
      kind: 'reflection',
      size: 0.55,
      distance: 4,
    });
    expect(effectOpOf(effectIds.softEdge(0))).toEqual({ kind: 'softEdge' });
    expect(effectOpOf(effectIds.softEdge(2.5))).toEqual({ kind: 'softEdge', radius: 2.5 });
    expect(effectOpOf(effectIds.rotation('none'))).toEqual({ kind: 'rotation3d' });
    expect(effectOpOf(effectIds.rotation('perspectiveLeft'))).toEqual({
      kind: 'rotation3d',
      preset: 'perspectiveLeft',
    });
    expect(effectOpOf(effectIds.warp('textWave1'))).toEqual({
      kind: 'textWarp',
      preset: 'textWave1',
    });
    expect(effectOpOf(effectIds.warp('none'))).toEqual({ kind: 'textWarp' });
    expect(effectOpOf(effectIds.outline('C00000'))).toEqual({
      kind: 'textOutline',
      color: 'C00000',
      width: 1,
    });
    expect(effectOpOf(effectIds.outline('none'))).toEqual({ kind: 'textOutline' });
    expect(effectOpOf(effectIds.fill('none'))).toEqual({
      kind: 'textFill',
      fill: { type: 'none' },
    });
    expect(effectOpOf(effectIds.fill('0070C0'))).toEqual({
      kind: 'textFill',
      fill: { type: 'solid', color: '0070C0' },
    });
    expect(effectOpOf(effectIds.fill('gradient.ocean'))).toEqual({
      kind: 'textFill',
      fill: { type: 'gradient', from: '00B0F0', to: '002060', angle: 90 },
    });
    expect(effectOpOf('text.effect.fill.nothex')).toBeNull();
    expect(effectOpOf('shape.effect.reflection.bogus')).toBeNull();
    expect(effectOpOf('file.open')).toBeNull();
  });

  it('lists every gallery entry once, all mappable', () => {
    const ids = allEffectIds(WARP_PRESETS);
    expect(new Set(ids).size).toBe(ids.length);
    expect(ids.length).toBe(
      REFLECTION_PRESETS.length +
        SOFT_EDGE_PRESETS.length +
        ROTATION_PRESETS.length +
        1 +
        WARP_PRESETS.length +
        1 +
        TEXT_COLORS.length * 2 +
        2 +
        TEXT_GRADIENTS.length,
    );
    for (const id of ids) expect(effectOpOf(id), id).not.toBeNull();
    // ST_TextShapeType: 41 values, all but textNoShape have a preset path.
    expect(WARP_PRESETS).toHaveLength(40);
  });
});

describe('galleries', () => {
  it('shape effects run the entry and close', () => {
    const exec = vi.fn();
    const close = vi.fn();
    render(<ShapeEffectsMenu label={label} exec={exec} close={close} />);
    const half = screen.getByTestId(`fx-reflection-${effectIds.reflection('half')}`);
    expect(half.getAttribute('aria-label')).toBe('reflectionPreset.half');
    fireEvent.click(half);
    expect(close).toHaveBeenCalled();
    expect(exec).toHaveBeenCalledWith('shape.effect.reflection.half');
    expect(screen.getByTestId(`fx-soft-${effectIds.softEdge(5)}`).getAttribute('title')).toBe(
      'points:5',
    );
    fireEvent.click(screen.getByTestId(`fx-rotation-${effectIds.rotation('isometricTopUp')}`));
    expect(exec).toHaveBeenLastCalledWith('shape.effect.rotation.isometricTopUp');
    // Every reflection / rotation entry has a drawn thumbnail.
    for (const p of ROTATION_PRESETS) {
      expect(
        screen
          .getByTestId(`fx-rotation-${effectIds.rotation(p)}`)
          .querySelector('svg path, svg rect'),
      ).not.toBeNull();
    }
  });

  it('text effects show the engine thumbnails of the warps', async () => {
    const exec = vi.fn();
    const previews = [
      { name: 'textArchUp', d: 'M4 40L60 4L116 40Z' },
      { name: 'textWave1', d: 'M4 10L116 10L116 60L4 60Z' },
    ];
    const load = vi.fn(() => Promise.resolve(previews));
    render(<TextEffectsMenu label={label} exec={exec} close={() => {}} loadPreviews={load} />);
    expect(screen.getByText('loading')).toBeTruthy();
    await act(async () => {
      await Promise.resolve();
    });
    expect(load).toHaveBeenCalledTimes(1);
    expect(screen.queryByText('loading')).toBeNull();
    const arch = screen.getByTestId(`fx-warp-${effectIds.warp('textArchUp')}`);
    expect(arch.getAttribute('title')).toBe('warp.textArchUp');
    expect(arch.querySelector('path')?.getAttribute('d')).toBe(previews[0]?.d);
    fireEvent.click(arch);
    expect(exec).toHaveBeenCalledWith('text.effect.warp.textArchUp');
    fireEvent.click(screen.getByTestId(`fx-outline-${effectIds.outline('C00000')}`));
    expect(exec).toHaveBeenLastCalledWith('text.effect.outline.C00000');
    fireEvent.click(screen.getByTestId(`fx-fill-${effectIds.fill('gradient.sunrise')}`));
    expect(exec).toHaveBeenLastCalledWith('text.effect.fill.gradient.sunrise');
  });

  it('a failed preview load still offers the other entries', async () => {
    render(
      <TextEffectsMenu
        label={label}
        exec={() => {}}
        close={() => {}}
        loadPreviews={() => Promise.reject(new Error('x'))}
      />,
    );
    await act(async () => {
      await Promise.resolve();
      await Promise.resolve();
    });
    expect(screen.getByTestId(`fx-warp-${effectIds.warp('none')}`)).toBeTruthy();
    expect(screen.queryByText('loading')).toBeNull();
  });
});
