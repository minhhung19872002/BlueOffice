// @vitest-environment happy-dom
import { createI18n, I18nextProvider } from '@blueoffice/i18n';
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import type { PageSetupInfo, SheetCommand, SheetHost } from '@blueoffice/platform';
import {
  areaText,
  colSpanText,
  insertCode,
  joinSections,
  parseColSpan,
  parseRowSpan,
  splitSections,
} from '../src/hfCodes';
import { PageSetupDialog, pictureSection } from '../src/pageSetup';

afterEach(cleanup);

const i18n = createI18n('en-US');

describe('header / footer section codes', () => {
  it('splits and joins &L &C &R sections', () => {
    expect(splitSections('&LPage &P&CTitle&R&D')).toEqual({
      left: 'Page &P',
      center: 'Title',
      right: '&D',
    });
    // Text before a section code is the centre section; && stays literal.
    expect(splitSections('A&&B&RX')).toEqual({ left: '', center: 'A&&B', right: 'X' });
    expect(joinSections({ left: '&A', center: '', right: '&P / &N' })).toBe('&L&A&R&P / &N');
    expect(joinSections(splitSections('&L&"Arial,Bold"x&Cy'))).toBe('&L&"Arial,Bold"x&Cy');
  });

  it('inserts codes at the caret', () => {
    expect(insertCode('Page  of', '&P', 5, 5)).toEqual({ text: 'Page &P of', caret: 7 });
    expect(insertCode('abc', '&D', 1, 3)).toEqual({ text: 'a&D', caret: 3 });
  });

  it('parses print titles and areas', () => {
    expect(parseRowSpan('1:3')).toEqual([0, 2]);
    expect(parseRowSpan('$4')).toEqual([3, 3]);
    expect(parseRowSpan('x')).toBeNull();
    expect(parseColSpan('b:a')).toEqual([0, 1]);
    expect(parseColSpan('AA')).toEqual([26, 26]);
    expect(colSpanText([0, 27])).toBe('A:AB');
    expect(areaText("Sheet1!$A$1:$B$2,'My sheet'!$D$4:$E$5")).toBe('A1:B2,D4:E5');
  });
});

function info(): PageSetupInfo {
  return {
    orientation: 'portrait',
    paperSize: 9,
    scale: 100,
    fit: null,
    margins: [0.7, 0.7, 0.75, 0.75],
    printArea: 'Sheet1!$A$1:$C$9',
    titleRows: [0, 0],
    gridLines: false,
    extra: {
      titleCols: null,
      headings: false,
      centerH: false,
      centerV: false,
      pageOrder: 'downThenOver',
      firstPageNumber: null,
      blackAndWhite: false,
      draft: false,
      errors: 'displayed',
      comments: 'none',
      hfMargins: [0.3, 0.3],
      headerFooter: {
        oddHeader: '&CReport',
        oddFooter: '',
        evenHeader: '',
        evenFooter: '',
        firstHeader: '',
        firstFooter: '',
        differentOddEven: false,
        differentFirst: false,
        scaleWithDoc: true,
        alignWithMargins: true,
      },
      rowBreaks: [],
      colBreaks: [],
    },
  };
}

describe('PageSetupDialog', () => {
  it('sends every tab’s settings with setPageSetup', async () => {
    const calls: SheetCommand[] = [];
    const host = {
      exec: vi.fn(async (cmd: SheetCommand) => {
        calls.push(cmd);
        return cmd.cmd === 'pageSetup' ? info() : { ok: true };
      }),
    } as unknown as SheetHost;
    const onDone = vi.fn();
    render(
      <I18nextProvider i18n={i18n}>
        <PageSetupDialog
          host={host}
          sheet={0}
          range={{ r0: 0, c0: 0, r1: 0, c1: 0 }}
          onClose={() => {}}
          onDone={onDone}
          fail={(e) => {
            throw e;
          }}
        />
      </I18nextProvider>,
    );
    await screen.findByTestId('page-setup');
    // Page: fit 2 pages wide, any height; first page number 3.
    fireEvent.click(screen.getByTestId('page-fit-mode'));
    fireEvent.change(screen.getByTestId('page-fit-wide'), { target: { value: '2' } });
    fireEvent.change(screen.getByTestId('page-fit-tall'), { target: { value: '0' } });
    fireEvent.change(screen.getByTestId('page-first-number'), { target: { value: '3' } });
    // Margins: centre horizontally.
    fireEvent.click(screen.getByTestId('page-tab-margins'));
    fireEvent.click(screen.getByTestId('page-center-h'));
    // Header / footer: insert the page number code in the footer's right part.
    fireEvent.click(screen.getByTestId('page-tab-headerFooter'));
    screen.getByTestId('page-hf-oddFooter-right').focus();
    fireEvent.focusIn(screen.getByTestId('page-hf-oddFooter-right'));
    fireEvent.click(screen.getByTestId('page-hf-oddFooter-code-page'));
    fireEvent.click(screen.getByTestId('page-hf-first'));
    fireEvent.change(await screen.findByTestId('page-hf-firstHeader-center'), {
      target: { value: 'Cover' },
    });
    // Sheet: two areas, title columns, headings, page order, errors, comments.
    fireEvent.click(screen.getByTestId('page-tab-sheet'));
    fireEvent.change(screen.getByTestId('page-area'), { target: { value: 'A1:C9, E1:F4' } });
    fireEvent.change(screen.getByTestId('page-title-cols'), { target: { value: 'A:A' } });
    fireEvent.click(screen.getByTestId('page-headings'));
    fireEvent.click(screen.getByTestId('page-order-overThenDown'));
    fireEvent.change(screen.getByTestId('page-errors'), { target: { value: 'dash' } });
    fireEvent.change(screen.getByTestId('page-comments'), { target: { value: 'atEnd' } });
    fireEvent.click(screen.getByTestId('page-ok'));
    await waitFor(() => expect(onDone).toHaveBeenCalled());
    const set = calls.find((c) => c.cmd === 'setPageSetup');
    expect(set).toMatchObject({
      cmd: 'setPageSetup',
      sheet: 0,
      fit: [2, 0],
      firstPageNumber: 3,
      centerH: true,
      titleRows: [0, 0],
      titleCols: [0, 0],
      printAreas: [
        { r0: 0, c0: 0, r1: 8, c1: 2 },
        { r0: 0, c0: 4, r1: 3, c1: 5 },
      ],
      headings: true,
      pageOrder: 'overThenDown',
      errors: 'dash',
      comments: 'atEnd',
      headerFooter: {
        oddHeader: '&CReport',
        oddFooter: '&R&P',
        firstHeader: '&CCover',
        differentFirst: true,
      },
    });
  });

  it('refuses an invalid print area', async () => {
    const host = {
      exec: vi.fn(async (cmd: SheetCommand) => (cmd.cmd === 'pageSetup' ? info() : { ok: true })),
    } as unknown as SheetHost;
    render(
      <I18nextProvider i18n={i18n}>
        <PageSetupDialog
          host={host}
          sheet={0}
          range={{ r0: 0, c0: 0, r1: 0, c1: 0 }}
          onClose={() => {}}
          onDone={() => {}}
          fail={() => {}}
        />
      </I18nextProvider>,
    );
    await screen.findByTestId('page-setup');
    fireEvent.click(screen.getByTestId('page-tab-sheet'));
    fireEvent.change(screen.getByTestId('page-area'), { target: { value: 'A1:??' } });
    fireEvent.click(screen.getByTestId('page-ok'));
    await screen.findByText(/not a valid list of ranges/);
    expect(host.exec).toHaveBeenCalledTimes(1);
  });

  it('names picture sections like the VML shapes and sends chosen pictures on OK', async () => {
    expect(pictureSection('oddHeader', 'center')).toBe('CH');
    expect(pictureSection('evenFooter', 'left')).toBe('LFEVEN');
    expect(pictureSection('firstHeader', 'right')).toBe('RHFIRST');
    const calls: SheetCommand[] = [];
    const withPicture = info();
    withPicture.extra.hfPictures = [{ section: 'RF', width: 40, height: 20, name: 'logo' }];
    withPicture.extra.headerFooter.oddFooter = '&R&G';
    const host = {
      exec: vi.fn(async (cmd: SheetCommand) => {
        calls.push(cmd);
        return cmd.cmd === 'pageSetup' ? withPicture : { ok: true };
      }),
    } as unknown as SheetHost;
    const pickFile = vi.fn(async () => ({ name: 'mark.png', bytes: new Uint8Array([1, 2, 3]) }));
    const onDone = vi.fn();
    render(
      <I18nextProvider i18n={i18n}>
        <PageSetupDialog
          host={host}
          sheet={0}
          range={{ r0: 0, c0: 0, r1: 0, c1: 0 }}
          onClose={() => {}}
          onDone={onDone}
          fail={(e) => {
            throw e;
          }}
          pickFile={pickFile}
        />
      </I18nextProvider>,
    );
    await screen.findByTestId('page-setup');
    fireEvent.click(screen.getByTestId('page-tab-headerFooter'));
    expect(screen.getByText('Right part: picture')).toBeTruthy();
    // A picture in the header's centre: &G goes into the text.
    fireEvent.focus(screen.getByTestId('page-hf-oddHeader-center'));
    fireEvent.click(screen.getByTestId('page-hf-oddHeader-picture'));
    await waitFor(() => expect(pickFile).toHaveBeenCalled());
    await waitFor(() =>
      expect(
        (screen.getByTestId('page-hf-oddHeader-center') as HTMLTextAreaElement).value,
      ).toContain('&G'),
    );
    // The footer's picture is removed, with its code.
    fireEvent.focus(screen.getByTestId('page-hf-oddFooter-right'));
    fireEvent.click(screen.getByTestId('page-hf-oddFooter-remove-picture'));
    fireEvent.click(screen.getByTestId('page-ok'));
    await waitFor(() => expect(onDone).toHaveBeenCalled());
    expect(calls).toContainEqual({
      cmd: 'headerFooterPicture',
      sheet: 0,
      section: 'CH',
      image: 'AQID',
      ext: 'png',
      name: 'mark.png',
    });
    expect(calls).toContainEqual({ cmd: 'headerFooterPicture', sheet: 0, section: 'RF' });
    const set = calls.find((c) => c.cmd === 'setPageSetup');
    expect(set).toMatchObject({ headerFooter: { oddFooter: '', oddHeader: '&CReport&G' } });
  });
});
