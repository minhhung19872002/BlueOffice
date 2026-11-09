// @vitest-environment happy-dom
import { CommandRegistry } from '@blueoffice/editor';
import { createI18n, I18nextProvider } from '@blueoffice/i18n';
import type { SlideCommand, SlideDesignInfo, SlideEditResult } from '@blueoffice/platform';
import { Ribbon } from '@blueoffice/ui';
import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import {
  RenameDialog,
  SlideSizeDialog,
  designTab,
  masterState,
  masterTab,
  registerDesignChoices,
  registerMasterCommands,
  sizeFromCm,
  toBase64,
  type MasterState,
} from '../src/masterView';

afterEach(cleanup);

const i18n = createI18n('vi-VN');
const t = i18n.t.bind(i18n) as (key: string, o?: Record<string, unknown>) => string;

const design: SlideDesignInfo = {
  cx: 12_192_000,
  cy: 6_858_000,
  sizeKind: null,
  theme: 'BlueSlide',
  colors: 'BlueSlide',
  fonts: 'BlueSlide',
  masters: [
    {
      index: 0,
      name: '',
      theme: 'BlueSlide',
      layouts: [
        { index: 0, name: 'Title Slide' },
        { index: 1, name: 'Title and Content' },
      ],
    },
  ],
  themes: [{ id: 'harbor', colors: ['14324F', 'E8EEF4', '1B6CA8'], fonts: ['Carlito', 'Carlito'] }],
  colorVariants: [{ id: 'bamboo', colors: ['1E3B2A'], fonts: null }],
  fontVariants: [{ id: 'tinos', colors: [], fonts: ['Tinos', 'Liberation Sans'] }],
};

function viewResult(slide: number): SlideEditResult {
  return {
    slide,
    masterView: {
      items: [
        {
          kind: 'master',
          name: 'Chính',
          master: 0,
          uses: 2,
          preserved: true,
          hideGraphics: false,
          title: true,
          footers: false,
        },
        {
          kind: 'layout',
          name: 'Trống',
          master: 0,
          uses: 0,
          preserved: false,
          hideGraphics: true,
          title: false,
          footers: true,
        },
      ],
    },
  } as unknown as SlideEditResult;
}

describe('BlueSlide master view helpers', () => {
  it('converts custom sizes from cm to EMU within §19.2.1.39 limits', () => {
    // 25.4 cm · 360 000 = 9 144 000 EMU; 19.05 cm · 360 000 = 6 858 000 EMU.
    expect(sizeFromCm(25.4, 19.05)).toEqual({ cx: 9_144_000, cy: 6_858_000 });
    // 2.54 cm = 914 400 EMU (the minimum), 142.24 cm = 51 206 400 EMU (the maximum).
    expect(sizeFromCm(2.54, 142.24)).toEqual({ cx: 914_400, cy: 51_206_400 });
    expect(sizeFromCm(2.5, 10)).toBeNull();
    expect(sizeFromCm(150, 10)).toBeNull();
    expect(sizeFromCm(Number.NaN, 10)).toBeNull();
  });

  it('encodes theme files as base64', () => {
    // RFC 4648 §4: 00 01 02 FF → "AAEC/w==".
    expect(toBase64(new Uint8Array([0, 1, 2, 255]))).toBe('AAEC/w==');
  });

  it('reads the current view slide', () => {
    expect(masterState(null)).toEqual({ masterView: false });
    expect(masterState(viewResult(0))).toMatchObject({
      masterView: true,
      masterItem: 'master',
      preserved: true,
      layoutTitle: true,
    });
    expect(masterState(viewResult(1))).toMatchObject({
      masterItem: 'layout',
      hideGraphics: true,
      preserved: true,
      layoutFooters: true,
    });
  });
});

describe('BlueSlide master commands', () => {
  function setup() {
    const sent: SlideCommand[] = [];
    const reg = new CommandRegistry<MasterState>();
    const openSize = vi.fn();
    const openRename = vi.fn();
    const bytes = new Uint8Array([1, 2, 3]);
    registerMasterCommands(reg, {
      run: (c) => sent.push(c),
      openSize,
      openRename,
      pickThemeFile: async () => bytes,
      names: () => ({ layout: 'Bố cục của tôi', master: 'Trang cái mới' }),
    });
    registerDesignChoices(reg, (c) => sent.push(c), design);
    return { sent, reg, openSize, openRename };
  }

  it('maps registry commands onto engine commands', async () => {
    const { sent, reg, openSize, openRename } = setup();
    const normal: MasterState = { hasDeck: true, masterView: false };
    const layout: MasterState = { hasDeck: true, masterView: true, masterItem: 'layout' };
    const master: MasterState = { hasDeck: true, masterView: true, masterItem: 'master' };
    expect(reg.isEnabled('view.slideMaster', normal)).toBe(true);
    expect(reg.isEnabled('master.insertLayout', normal)).toBe(false);
    expect(reg.isEnabled('master.placeholder.picture', master)).toBe(false);
    expect(reg.isEnabled('master.placeholder.picture', layout)).toBe(true);
    expect(reg.isEnabled('design.slideSize', layout)).toBe(false);
    await reg.execute('view.slideMaster', normal);
    await reg.execute('master.insertLayout', layout);
    await reg.execute('master.insertMaster', master);
    await reg.execute('master.placeholder.picture', layout);
    await reg.execute('master.footers.off', layout);
    await reg.execute('design.hideGraphics.on', normal);
    await reg.execute('design.theme.harbor', normal);
    await reg.execute('design.colors.bamboo', normal);
    await reg.execute('design.fonts.tinos', normal);
    await reg.execute('design.themeFile', normal);
    await reg.execute('master.close', layout);
    await reg.execute('design.slideSize', normal);
    await reg.execute('master.rename', layout);
    expect(openSize).toHaveBeenCalledOnce();
    expect(openRename).toHaveBeenCalledOnce();
    expect(sent).toEqual([
      { cmd: 'masterView', on: true },
      { cmd: 'insertLayout', name: 'Bố cục của tôi' },
      { cmd: 'addMaster', name: 'Trang cái mới' },
      { cmd: 'insertPlaceholder', kind: 'picture' },
      { cmd: 'layoutElements', footers: false },
      { cmd: 'hideBackgroundGraphics', hide: true },
      { cmd: 'applyTheme', builtin: 'harbor' },
      { cmd: 'themeVariant', colors: 'bamboo' },
      { cmd: 'themeVariant', fonts: 'tinos' },
      { cmd: 'applyTheme', file: 'AQID' },
      { cmd: 'masterView', on: false },
    ]);
  });

  it('builds the Slide master and Design tabs', () => {
    const { reg, sent } = setup();
    const state: MasterState = { hasDeck: true, masterView: true, masterItem: 'layout' };
    const item = (
      id: string,
      label: string,
      icon: React.ReactNode,
      size: 'large' | 'small' = 'large',
      checked?: boolean,
    ) => ({
      id,
      label,
      icon,
      size,
      enabled: reg.isEnabled(id, state),
      checked,
      onSelect: () => void reg.execute(id, state),
    });
    const run = (c: SlideCommand) => sent.push(c);
    const tabs = [
      masterTab(t, item, state, run),
      designTab(t, item, design, { hasDeck: true, masterView: false }, 3, run),
    ];
    render(
      <I18nextProvider i18n={i18n}>
        <Ribbon tabs={tabs} value="design" />
      </I18nextProvider>,
    );
    expect(tabs[0]!.label).toBe('Trang chiếu cái');
    expect(tabs[1]!.groups.map((g) => g.id)).toEqual([
      'designThemes',
      'designVariants',
      'designCustomize',
      'designLayouts',
    ]);
    expect(tabs[1]!.groups[0]!.items[0]!.label).toBe('Bến cảng');
    // Applying a layout to the current slide.
    tabs[1]!.groups[3]!.items[1]!.onSelect();
    fireEvent.input(screen.getByTestId('design-background'), { target: { value: '#ff8800' } });
    fireEvent.change(screen.getByTestId('design-background'), { target: { value: '#ff8800' } });
    expect(sent[0]).toEqual({ cmd: 'applyLayout', layout: 1, slides: [3] });
    expect(sent).toContainEqual({ cmd: 'setBackground', color: 'FF8800' });
  });
});

describe('BlueSlide slide size and rename dialogs', () => {
  it('sends a preset size with "fit"', () => {
    const onApply = vi.fn();
    render(
      <I18nextProvider i18n={i18n}>
        <SlideSizeDialog open cx={12_192_000} cy={6_858_000} onClose={() => {}} onApply={onApply} />
      </I18nextProvider>,
    );
    fireEvent.click(screen.getByTestId('slide-size-standard'));
    fireEvent.click(screen.getByTestId('slide-size-fit'));
    fireEvent.click(screen.getByTestId('slide-size-ok'));
    expect(onApply).toHaveBeenCalledWith({
      cmd: 'setSlideSize',
      cx: 9_144_000,
      cy: 6_858_000,
      kind: 'screen4x3',
      mode: 'fit',
    });
  });

  it('sends a custom size with "scale" and refuses sizes out of range', () => {
    const onApply = vi.fn();
    render(
      <I18nextProvider i18n={i18n}>
        <SlideSizeDialog open cx={12_192_000} cy={6_858_000} onClose={() => {}} onApply={onApply} />
      </I18nextProvider>,
    );
    fireEvent.click(screen.getByTestId('slide-size-custom'));
    fireEvent.change(screen.getByTestId('slide-size-width'), { target: { value: '1' } });
    expect((screen.getByTestId('slide-size-ok') as HTMLButtonElement).disabled).toBe(true);
    expect(screen.getByRole('alert').textContent).toContain('2,54');
    // 20 cm = 7 200 000 EMU, 10 cm = 3 600 000 EMU.
    fireEvent.change(screen.getByTestId('slide-size-width'), { target: { value: '20' } });
    fireEvent.change(screen.getByTestId('slide-size-height'), { target: { value: '10' } });
    fireEvent.click(screen.getByTestId('slide-size-scale'));
    fireEvent.click(screen.getByTestId('slide-size-ok'));
    expect(onApply).toHaveBeenCalledWith({
      cmd: 'setSlideSize',
      cx: 7_200_000,
      cy: 3_600_000,
      kind: 'custom',
      mode: 'scale',
    });
  });

  it('renames the current master or layout', () => {
    const onRename = vi.fn();
    render(
      <I18nextProvider i18n={i18n}>
        <RenameDialog open initial="Trống" onClose={() => {}} onRename={onRename} />
      </I18nextProvider>,
    );
    const input = screen.getByTestId('master-rename-input') as HTMLInputElement;
    expect(input.value).toBe('Trống');
    fireEvent.change(input, { target: { value: '  ' } });
    expect((screen.getByTestId('master-rename-ok') as HTMLButtonElement).disabled).toBe(true);
    fireEvent.change(input, { target: { value: ' Ảnh lớn ' } });
    fireEvent.click(screen.getByTestId('master-rename-ok'));
    expect(onRename).toHaveBeenCalledWith('Ảnh lớn');
  });
});
