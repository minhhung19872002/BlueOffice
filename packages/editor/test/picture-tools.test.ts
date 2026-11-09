import { createI18n } from '@blueoffice/i18n';
import { PICTURE_COMMANDS, PictureActionsHub, type PictureActions } from '@blueoffice/ui';
import { describe, expect, it, vi } from 'vitest';
import { CommandRegistry } from '../src/commands';
import { pictureTabLabels, presetName, registerPictureCommands } from '../src/pictureTools';

describe('picture tools labels (CORE-PIC-001)', () => {
  it('every label resolves in both languages', () => {
    for (const lang of ['vi-VN', 'en-US'] as const) {
      const i18n = createI18n(lang);
      const t = i18n.t.bind(i18n);
      const l = pictureTabLabels(t);
      for (const [k, v] of Object.entries(l)) {
        if (typeof v === 'string')
          expect(v.startsWith('pictureTools.'), `${lang} ${k}`).toBe(false);
      }
      for (const id of [
        'sharp.soften50',
        'sharp.none',
        'recolor.sepia',
        'recolor.washout',
        'recolor.bw50',
      ]) {
        expect(presetName(t, id).startsWith('pictureTools.'), id).toBe(false);
      }
      expect(l.shape('heart').startsWith('pictureTools.')).toBe(false);
    }
    const vi_ = createI18n('vi-VN');
    const t = vi_.t.bind(vi_);
    expect(presetName(t, 'lum.20.-40')).toBe('Độ sáng 20%, độ tương phản -40%');
    expect(presetName(t, 'alpha.30')).toBe('Trong suốt 30%');
    expect(pictureTabLabels(t).ppi(150)).toBe('150 điểm ảnh mỗi inch');
  });

  it('registers the picture commands and runs the mounted actions', async () => {
    const r = new CommandRegistry<{ on: boolean }>();
    const hub = new PictureActionsHub();
    registerPictureCommands(
      r,
      (s) => s.on,
      () => hub.get(),
    );
    for (const id of PICTURE_COMMANDS) expect(r.has(id)).toBe(true);
    const reset = vi.fn();
    const actions = Object.fromEntries(PICTURE_COMMANDS.map((id) => [id, vi.fn()])) as Record<
      string,
      () => void
    >;
    actions['picture.reset'] = reset;
    hub.set(actions as PictureActions);
    expect(await r.execute('picture.reset', { on: false })).toBe(false);
    expect(await r.execute('picture.reset', { on: true })).toBe(true);
    expect(reset).toHaveBeenCalledTimes(1);
  });
});
