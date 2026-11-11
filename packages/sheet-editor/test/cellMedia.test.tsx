// @vitest-environment happy-dom
import { createI18n, I18nextProvider } from '@blueoffice/i18n';
import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import type { RemoteImages, SheetCommand, SheetHost } from '@blueoffice/platform';
import {
  CellPictureAltDialog,
  checkboxBox,
  checkboxHit,
  clickCheckbox,
  deleteCheckboxes,
  extOf,
  fromBase64,
  loadRequestedImages,
  registerCellMediaCommands,
  sheetMediaSource,
  toBase64,
  type CellMediaEditorState,
} from '../src/cellMedia';

afterEach(cleanup);

const i18n = createI18n('en-US');

function fakeHost(results: Record<string, unknown> = {}) {
  const calls: SheetCommand[] = [];
  const exec = vi.fn(async (cmd: SheetCommand) => {
    calls.push(cmd);
    return results[cmd.cmd] ?? null;
  });
  const host = { exec } as unknown as Pick<SheetHost, 'exec'>;
  return { host, exec, calls };
}

describe('checkbox geometry (same rule as bo_sheet::cell_image)', () => {
  it('centres a 12 pt box in a 60 × 20 pt cell', () => {
    // side = min(20 − 4, 12) = 12; x = 100 + (60 − 12)/2 = 124; y = 50 + (20 − 12)/2 = 54.
    expect(checkboxBox({ x: 100, y: 50, w: 60, h: 20 })).toEqual({ x: 124, y: 54, w: 12, h: 12 });
    // A 10 pt high cell: side = 10 − 4 = 6.
    expect(checkboxBox({ x: 0, y: 0, w: 30, h: 10 })).toEqual({ x: 12, y: 2, w: 6, h: 6 });
  });

  it('hits within 2 pt of the box', () => {
    const cell = { x: 100, y: 50, w: 60, h: 20 };
    expect(checkboxHit(cell, 130, 60)).toBe(true);
    expect(checkboxHit(cell, 122.5, 60)).toBe(true);
    expect(checkboxHit(cell, 105, 60)).toBe(false);
  });
});

describe('helpers', () => {
  it('round-trips base64 and reads extensions', () => {
    const bytes = new Uint8Array([0, 1, 2, 250, 255]);
    expect(toBase64(bytes)).toBe('AAEC+v8=');
    expect([...fromBase64('AAEC+v8=')]).toEqual([...bytes]);
    expect(extOf('Ảnh.JPEG')).toBe('jpeg');
    expect(extOf('noext')).toBe('png');
  });

  it('resolves display-list ids through cellMedia', async () => {
    const { host, calls } = fakeHost({ cellMedia: 'AAEC' });
    const bytes = await sheetMediaSource(host).getMedia('cellimg:1');
    expect([...bytes]).toEqual([0, 1, 2]);
    expect(calls[0]).toEqual({ cmd: 'cellMedia', id: 'cellimg:1' });
  });
});

describe('IMAGE pictures', () => {
  it('fetches the requested URLs through Platform.images and reports failures', async () => {
    const { host, calls } = fakeHost({
      imageRequests: ['https://a.vn/x.png', 'https://a.vn/y.png'],
    });
    const images: RemoteImages = {
      fetchImage: vi.fn(async (url: string) =>
        url.endsWith('x.png')
          ? { ok: true as const, bytes: new Uint8Array([1, 2, 3]) }
          : { ok: false as const, error: 'connect' as const },
      ),
    };
    expect(await loadRequestedImages(host, images)).toBe(true);
    expect(calls).toContainEqual({ cmd: 'imageLoaded', url: 'https://a.vn/x.png', data: 'AQID' });
    expect(calls).toContainEqual({
      cmd: 'imageLoaded',
      url: 'https://a.vn/y.png',
      error: 'connect',
    });
  });

  it('reports every URL blocked without the service, and nothing to do when none', async () => {
    const { host, calls } = fakeHost({ imageRequests: ['https://a.vn/x.png'] });
    expect(await loadRequestedImages(host, undefined)).toBe(true);
    expect(calls[1]).toEqual({ cmd: 'imageLoaded', url: 'https://a.vn/x.png', error: 'blocked' });
    const idle = fakeHost({ imageRequests: [] });
    expect(await loadRequestedImages(idle.host, undefined)).toBe(false);
  });
});

describe('checkbox clicks and Delete', () => {
  it('toggles only when the click is on the box of a checkbox cell', async () => {
    const { host } = fakeHost({ cellMediaState: { checkbox: true, value: false, image: null } });
    const exec = vi.fn(async () => null);
    const cell = { x: 0, y: 0, w: 60, h: 20 };
    expect(await clickCheckbox(host, exec, 0, 3, 1, cell, 5, 10)).toBe(false);
    expect(exec).not.toHaveBeenCalled();
    expect(await clickCheckbox(host, exec, 0, 3, 1, cell, 30, 10)).toBe(true);
    expect(exec).toHaveBeenCalledWith({
      cmd: 'toggleCheckbox',
      sheet: 0,
      range: { r0: 3, c0: 1, r1: 3, c1: 1 },
      row: 3,
      col: 1,
    });
    const plain = fakeHost({ cellMediaState: { checkbox: false, value: null, image: null } });
    expect(await clickCheckbox(plain.host, exec, 0, 3, 1, cell, 30, 10)).toBe(false);
  });

  it('Delete is handled by the engine only for checkbox ranges', async () => {
    const range = { r0: 0, c0: 0, r1: 1, c1: 0 };
    expect(await deleteCheckboxes(async () => ({ handled: true }), 0, range)).toBe(true);
    expect(await deleteCheckboxes(async () => ({ handled: false }), 0, range)).toBe(false);
  });
});

describe('commands', () => {
  function setup(results: Record<string, unknown> = {}) {
    const reg = new Map<string, (s: CellMediaEditorState) => Promise<unknown>>();
    const { host } = fakeHost(results);
    const exec = vi.fn(async (_c: SheetCommand) => null as unknown);
    const openAltText = vi.fn();
    const openBackground = vi.fn();
    const pickFile = vi.fn(async () => ({ name: 'logo.PNG', bytes: new Uint8Array([7, 8]) }));
    registerCellMediaCommands<CellMediaEditorState>(
      (id, run) => reg.set(id, run),
      (s) => s,
      { exec, pickFile, openAltText, openBackground, fail: vi.fn() },
    );
    const state: CellMediaEditorState = {
      host,
      hasWorkbook: true,
      sheet: 1,
      active: { row: 2, col: 3 },
      range: { r0: 2, c0: 3, r1: 4, c1: 3 },
    };
    return { reg, exec, openAltText, openBackground, pickFile, state };
  }

  it('inserts checkboxes on the selection and toggles from the active cell', async () => {
    const { reg, exec, state } = setup();
    await reg.get('insert.checkbox')!(state);
    expect(exec).toHaveBeenLastCalledWith({ cmd: 'insertCheckbox', sheet: 1, range: state.range });
    await reg.get('cell.toggleCheckbox')!(state);
    expect(exec).toHaveBeenLastCalledWith({
      cmd: 'toggleCheckbox',
      sheet: 1,
      range: state.range,
      row: 2,
      col: 3,
    });
  });

  it('places a picture from a file in the active cell', async () => {
    const { reg, exec, pickFile, state } = setup();
    await reg.get('insert.pictureInCell')!(state);
    expect(pickFile).toHaveBeenCalledWith(['.png', '.jpg', '.jpeg', '.gif', '.bmp']);
    expect(exec).toHaveBeenLastCalledWith({
      cmd: 'placeImageInCell',
      sheet: 1,
      row: 2,
      col: 3,
      data: 'Bwg=',
      ext: 'png',
      alt: null,
    });
  });

  it('opens the alt-text dialog only for stored pictures', async () => {
    const local = setup({
      cellMediaState: {
        checkbox: false,
        value: null,
        image: { kind: 'local', alt: 'Logo', decorative: false },
      },
    });
    await local.reg.get('cell.pictureAltText')!(local.state);
    expect(local.openAltText).toHaveBeenCalledWith({
      sheet: 1,
      row: 2,
      col: 3,
      alt: 'Logo',
      decorative: false,
    });
    const web = setup({
      cellMediaState: {
        checkbox: false,
        value: null,
        image: {
          kind: 'web',
          alt: null,
          url: 'https://a.vn',
          sizing: 0,
          height: null,
          width: null,
          state: 'ready',
        },
      },
    });
    await web.reg.get('cell.pictureAltText')!(web.state);
    expect(web.openAltText).not.toHaveBeenCalled();
  });

  it('opens the background removal mode only for stored pictures (CORE-PIC-002)', async () => {
    const local = setup({
      cellMediaState: {
        checkbox: false,
        value: null,
        image: { kind: 'local', alt: null, decorative: false },
      },
    });
    await local.reg.get('cell.pictureRemoveBackground')!(local.state);
    expect(local.openBackground).toHaveBeenCalledWith({ sheet: 1, row: 2, col: 3 });
    const none = setup({ cellMediaState: { checkbox: false, value: null, image: null } });
    await none.reg.get('cell.pictureRemoveBackground')!(none.state);
    expect(none.openBackground).not.toHaveBeenCalled();
  });
});

describe('alt-text dialog', () => {
  it('applies the text, or none when decorative', () => {
    const onApply = vi.fn();
    render(
      <I18nextProvider i18n={i18n}>
        <CellPictureAltDialog
          initial="Logo"
          decorative={false}
          onClose={() => {}}
          onApply={onApply}
        />
      </I18nextProvider>,
    );
    fireEvent.change(screen.getByTestId('cell-picture-alt-text'), {
      target: { value: '  Biểu trưng  ' },
    });
    fireEvent.click(screen.getByTestId('cell-picture-alt-ok'));
    expect(onApply).toHaveBeenLastCalledWith('Biểu trưng', false);
    fireEvent.click(screen.getByTestId('cell-picture-decorative'));
    fireEvent.click(screen.getByTestId('cell-picture-alt-ok'));
    expect(onApply).toHaveBeenLastCalledWith(null, true);
  });
});
