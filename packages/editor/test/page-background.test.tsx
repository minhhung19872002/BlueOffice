// @vitest-environment happy-dom
import { createI18n, I18nextProvider } from '@blueoffice/i18n';
import type { DesignState, EditCommand, EditResult } from '@blueoffice/platform';
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import type { ReactNode } from 'react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import {
  BorderPreview,
  bordersCommand,
  PageBordersDialog,
  parseHex,
  StyleSetGallery,
  ThemeGallery,
  WatermarkDialog,
  WatermarkMenu,
  watermarkCommand,
} from '../src/PageBackgroundDialogs';

afterEach(cleanup);

const i18n = createI18n('en-US');
const wrap = (n: ReactNode) => render(<I18nextProvider i18n={i18n}>{n}</I18nextProvider>);

const DESIGN: DesignState = {
  pageColor: null,
  borders: null,
  watermark: null,
  theme: null,
  styleSet: null,
  themes: [
    {
      id: 'ocean',
      colors: [
        '000000',
        'FFFFFF',
        '1B3A5C',
        'E7EEF5',
        '1F6FB2',
        '2BA3A0',
        'F2A541',
        'D9534F',
        '6C5CA8',
        '7A8B99',
        '1F6FB2',
        '6C5CA8',
      ],
      major: 'Cambria',
      minor: 'Calibri',
    },
  ],
  styleSets: ['compact', 'balanced', 'airy', 'double'],
  artStyles: ['basicBlackDots', 'diamondsGray'],
};

/** A `run` that answers `design` with `state` and records the other commands. */
function engine(state: DesignState = DESIGN) {
  const sent: EditCommand[] = [];
  const run = vi.fn(async (c: EditCommand) => {
    if (c.cmd === 'design') return { design: state } as EditResult;
    sent.push(c);
    return {} as EditResult;
  });
  return { run, sent };
}

describe('page borders command', () => {
  const spec = {
    style: 'double',
    size: 6,
    color: '1F6FB2',
    space: 12,
    sides: [true, false, true, false] as [boolean, boolean, boolean, boolean],
  };

  it('box draws every edge without shadow, whole document', () => {
    expect(bordersCommand('box', spec, 'document')).toEqual({
      cmd: 'setPageBorders',
      scope: 'document',
      borders: { ...spec, sides: [true, true, true, true], shadow: false, display: 'allPages' },
    });
  });

  it('custom keeps the chosen edges; first page only applies to the section', () => {
    const c = bordersCommand('custom', spec, 'sectionFirst');
    expect(c).toMatchObject({
      scope: 'section',
      borders: { sides: [true, false, true, false], display: 'firstPage', shadow: false },
    });
    expect(bordersCommand('shadow', spec, 'sectionNotFirst')).toMatchObject({
      borders: { shadow: true, display: 'notFirstPage' },
    });
  });

  it('none removes the borders', () => {
    expect(bordersCommand('none', spec, 'section')).toEqual({
      cmd: 'setPageBorders',
      scope: 'section',
    });
  });

  it('parses colours', () => {
    expect(parseHex('#1f6fb2')).toBe('1F6FB2');
    expect(parseHex('1F6FB2')).toBe('1F6FB2');
    expect(parseHex('blue')).toBeNull();
  });
});

describe('watermark command', () => {
  it('trims the text and refuses an empty one', () => {
    expect(watermarkCommand('text', { text: '  MẪU ', diagonal: false }, null, null, true)).toEqual(
      {
        cmd: 'setWatermark',
        watermark: { text: 'MẪU', diagonal: false },
      },
    );
    expect(watermarkCommand('text', { text: '   ' }, null, null, true)).toBeNull();
  });

  it('needs a picture for a picture watermark; none removes it', () => {
    expect(watermarkCommand('picture', {}, null, null, true)).toBeNull();
    expect(watermarkCommand('picture', {}, 'AAAA', 150, false)).toEqual({
      cmd: 'setWatermark',
      watermark: { picture: 'AAAA', scale: 150, washout: false },
    });
    expect(watermarkCommand('none', {}, null, null, true)).toEqual({ cmd: 'setWatermark' });
  });
});

describe('page borders dialog', () => {
  it('starts from the section borders and sends an art border', async () => {
    const state: DesignState = {
      ...DESIGN,
      borders: {
        style: 'single',
        size: 8,
        color: 'FF0000',
        space: 20,
        sides: [true, true, true, true],
        offsetFromPage: false,
        display: 'allPages',
        behind: false,
        shadow: true,
        art: false,
        surroundHeader: true,
        surroundFooter: false,
      },
    };
    const { run, sent } = engine(state);
    const onClose = vi.fn();
    wrap(<PageBordersDialog run={run} onClose={onClose} />);
    await waitFor(() =>
      expect(screen.getByTestId('borders-setting-shadow').getAttribute('aria-checked')).toBe(
        'true',
      ),
    );
    expect((screen.getByTestId('borders-color') as HTMLInputElement).value).toBe('FF0000');
    // §17.15.1.18 / §17.15.1.19: measured from the text, the borders go around the header
    // (and here not the footer) — the footer is turned on too.
    expect((screen.getByTestId('borders-surroundHeader') as HTMLInputElement).checked).toBe(true);
    const footer = screen.getByTestId('borders-surroundFooter') as HTMLInputElement;
    expect(footer.checked).toBe(false);
    fireEvent.click(footer);
    fireEvent.change(screen.getByTestId('borders-art'), { target: { value: 'diamondsGray' } });
    fireEvent.change(screen.getByTestId('borders-width'), { target: { value: '16' } });
    fireEvent.change(screen.getByTestId('borders-measure'), { target: { value: 'page' } });
    fireEvent.click(screen.getByTestId('borders-setting-box'));
    // The preview draws the motif along the edges.
    expect(
      screen.getByTestId('borders-preview').querySelectorAll('polygon').length,
    ).toBeGreaterThan(8);
    fireEvent.click(screen.getByTestId('page-borders-ok'));
    expect(onClose).toHaveBeenCalled();
    expect(sent[0]).toMatchObject({
      cmd: 'setPageBorders',
      scope: 'document',
      borders: {
        style: 'diamondsGray',
        size: 16,
        color: 'FF0000',
        offsetFromPage: true,
        shadow: false,
        sides: [true, true, true, true],
        surroundFooter: true,
      },
    });
  });

  it('custom edges and removal', async () => {
    const { run, sent } = engine();
    wrap(<PageBordersDialog run={run} onClose={() => {}} />);
    fireEvent.click(screen.getByTestId('borders-setting-custom'));
    fireEvent.click(screen.getByTestId('borders-side-left'));
    fireEvent.click(screen.getByTestId('borders-side-right'));
    fireEvent.click(screen.getByTestId('page-borders-ok'));
    await waitFor(() => expect(sent).toHaveLength(1));
    expect(sent[0]).toMatchObject({
      borders: { style: 'single', sides: [true, false, true, false] },
    });
    cleanup();
    wrap(<PageBordersDialog run={run} onClose={() => {}} />);
    fireEvent.click(screen.getByTestId('borders-setting-none'));
    fireEvent.click(screen.getByTestId('page-borders-ok'));
    await waitFor(() => expect(sent).toHaveLength(2));
    expect(sent[1]).toEqual({ cmd: 'setPageBorders', scope: 'document' });
  });

  it('previews nothing for no borders', () => {
    wrap(<BorderPreview spec={null} />);
    expect(screen.getByTestId('borders-preview').querySelectorAll('line').length).toBe(0);
  });
});

describe('watermark dialog and menu', () => {
  it('sends a custom horizontal text watermark', async () => {
    const { run, sent } = engine();
    wrap(<WatermarkDialog run={run} onClose={() => {}} pickPicture={async () => null} />);
    fireEvent.change(screen.getByTestId('watermark-text'), { target: { value: 'NỘI BỘ' } });
    fireEvent.change(screen.getByTestId('watermark-layout'), { target: { value: 'horizontal' } });
    fireEvent.change(screen.getByTestId('watermark-size'), { target: { value: '72' } });
    fireEvent.change(screen.getByTestId('watermark-color'), { target: { value: '#c00000' } });
    fireEvent.click(screen.getByTestId('watermark-semitransparent'));
    fireEvent.click(screen.getByTestId('watermark-ok'));
    await waitFor(() => expect(sent).toHaveLength(1));
    expect(sent[0]).toMatchObject({
      cmd: 'setWatermark',
      watermark: {
        text: 'NỘI BỘ',
        diagonal: false,
        size: 72,
        color: 'C00000',
        semitransparent: false,
      },
    });
  });

  it('picks a picture and washes it out', async () => {
    const { run, sent } = engine();
    const pick = vi.fn(async () => ({ base64: 'iVBORw0K', name: 'logo.png' }));
    wrap(<WatermarkDialog run={run} onClose={() => {}} pickPicture={pick} />);
    fireEvent.click(screen.getByTestId('watermark-kind-picture'));
    expect((screen.getByTestId('watermark-ok') as HTMLButtonElement).disabled).toBe(true);
    fireEvent.click(screen.getByTestId('watermark-choose'));
    await screen.findByText('logo.png');
    fireEvent.click(screen.getByTestId('watermark-ok'));
    await waitFor(() => expect(sent).toHaveLength(1));
    expect(sent[0]).toEqual({
      cmd: 'setWatermark',
      watermark: { picture: 'iVBORw0K', scale: null, washout: true },
    });
  });

  it('shows the current watermark', async () => {
    const { run } = engine({
      ...DESIGN,
      watermark: {
        text: 'MẬT',
        font: 'Cambria',
        color: '808080',
        picture: false,
        washout: false,
        semitransparent: true,
        diagonal: true,
      },
    });
    wrap(<WatermarkDialog run={run} onClose={() => {}} pickPicture={async () => null} />);
    expect((await screen.findByTestId('watermark-current')).textContent).toContain('MẬT');
    expect((screen.getByTestId('watermark-text') as HTMLInputElement).value).toBe('MẬT');
  });

  it('menu presets, custom and removal', () => {
    const { run, sent } = engine();
    const custom = vi.fn();
    wrap(<WatermarkMenu run={run} disabled={false} onCustom={custom} />);
    fireEvent.click(screen.getByTestId('watermark-menu'));
    fireEvent.click(document.querySelector('[data-preset="confidential"]') as HTMLElement);
    expect(sent[0]).toEqual({ cmd: 'setWatermark', watermark: { text: 'CONFIDENTIAL' } });
    fireEvent.click(screen.getByTestId('watermark-menu'));
    fireEvent.click(
      document.querySelector('[data-command="design.customWatermark"]') as HTMLElement,
    );
    expect(custom).toHaveBeenCalled();
    fireEvent.click(screen.getByTestId('watermark-menu'));
    fireEvent.click(
      document.querySelector('[data-command="design.removeWatermark"]') as HTMLElement,
    );
    expect(sent[1]).toEqual({ cmd: 'setWatermark' });
  });
});

describe('theme and style set galleries', () => {
  it('applies a theme', async () => {
    const { run, sent } = engine();
    wrap(<ThemeGallery run={run} disabled={false} />);
    fireEvent.click(screen.getByTestId('theme-gallery'));
    const item = await screen.findByText('Ocean');
    expect(screen.getByText('Headings: Cambria · Body: Calibri')).toBeTruthy();
    fireEvent.click(item);
    expect(sent[0]).toEqual({ cmd: 'applyTheme', id: 'ocean' });
  });

  it('applies a style set', () => {
    const { run, sent } = engine();
    wrap(<StyleSetGallery run={run} disabled={false} />);
    fireEvent.click(screen.getByTestId('style-set-gallery'));
    fireEvent.click(document.querySelector('[data-style-set="airy"]') as HTMLElement);
    expect(sent[0]).toEqual({ cmd: 'applyStyleSet', id: 'airy' });
  });
});
