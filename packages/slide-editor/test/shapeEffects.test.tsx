// @vitest-environment happy-dom
import { CommandRegistry } from '@blueoffice/editor';
import { createI18n, I18nextProvider } from '@blueoffice/i18n';
import type { SlideCommand } from '@blueoffice/platform';
import { Ribbon } from '@blueoffice/ui';
import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { registerShapeEffects, shapeEffectsGroup, warpPreviewLoader } from '../src/shapeEffects';
import { shapeFormatTab, type ShapeToolState } from '../src/shapeFormat';

afterEach(cleanup);

const i18n = createI18n('vi-VN');
const t = i18n.t.bind(i18n) as unknown as Parameters<typeof shapeFormatTab>[0]['t'];

describe('shape effect commands', () => {
  it('send shapeEffects edits for the selected shapes', async () => {
    const sent: SlideCommand[] = [];
    const reg = new CommandRegistry<ShapeToolState>();
    const off = registerShapeEffects(reg, (c) => sent.push(c));
    const one: ShapeToolState = { isShape: true };
    expect(reg.isEnabled('shape.effect.reflection.half', {})).toBe(false);
    expect(reg.isEnabled('shape.effect.reflection.half', { mergeable: true })).toBe(true);
    await reg.execute('shape.effect.softEdge.10', one);
    expect(sent.at(-1)).toEqual({ cmd: 'shapeEffects', op: { kind: 'softEdge', radius: 10 } });
    await reg.execute('text.effect.warp.textCurveUp', one);
    expect(sent.at(-1)).toEqual({
      cmd: 'shapeEffects',
      op: { kind: 'textWarp', preset: 'textCurveUp' },
    });
    await reg.execute('shape.effect.rotation.none', one);
    expect(sent.at(-1)).toEqual({ cmd: 'shapeEffects', op: { kind: 'rotation3d' } });
    off();
    expect(reg.isEnabled('shape.effect.softEdge.10', one)).toBe(false);
  });

  it('asks the engine for the warp thumbnails once', async () => {
    const run = vi.fn(async () => ({ warpPreviews: [{ name: 'textPlain', d: 'M0 0Z' }] }));
    const load = warpPreviewLoader(run);
    expect(await load()).toEqual([{ name: 'textPlain', d: 'M0 0Z' }]);
    await load();
    expect(run).toHaveBeenCalledTimes(1);
    expect(run).toHaveBeenCalledWith({ cmd: 'shapeEffects', op: { kind: 'warpPreviews' } });
  });

  it('adds the Effects group to the shape tab', () => {
    const state: ShapeToolState = { isShape: true };
    const exec = vi.fn();
    const tab = shapeFormatTab({
      t,
      state,
      enabled: () => true,
      exec,
      extraGroups: [shapeEffectsGroup({ t, state, exec, loadPreviews: () => Promise.resolve([]) })],
    });
    render(
      <I18nextProvider i18n={i18n}>
        <Ribbon tabs={[tab]} value="shapeFormat" />
      </I18nextProvider>,
    );
    fireEvent.click(screen.getByTestId('shape-effects-menu'));
    const entry = screen.getByTestId('fx-reflection-shape.effect.reflection.short');
    expect(entry.getAttribute('title')).toBe('Ngắn, sát hình');
    fireEvent.click(entry);
    expect(exec).toHaveBeenCalledWith('shape.effect.reflection.short');
    fireEvent.click(screen.getByTestId('text-effects-menu'));
    expect(screen.getByTestId('text-effects-gallery')).toBeTruthy();
    expect(screen.getByTestId('fx-outline-text.effect.outline.none').getAttribute('title')).toBe(
      'Không viền',
    );
  });
});
