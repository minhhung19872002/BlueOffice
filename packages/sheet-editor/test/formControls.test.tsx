// @vitest-environment happy-dom
import { createI18n, I18nextProvider } from '@blueoffice/i18n';
import { cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import type {
  FormControlInfo,
  FormControlKind,
  SheetCommand,
  SheetHost,
  SheetView,
} from '@blueoffice/platform';
import {
  controlBox,
  FormatControlDialog,
  FormControlLayer,
  type FormControlLayerProps,
  moveSel,
  placeControls,
  specOf,
} from '../src/formControls';

afterEach(cleanup);

const i18n = createI18n('en-US');
const view = { sheet: 0 } as unknown as SheetView;

function ctl(kind: FormControlKind, index: number, extra: Partial<FormControlInfo> = {}) {
  const info: FormControlInfo = {
    index,
    kind,
    name: `${kind} ${index}`,
    text: `Caption ${index}`,
    link: null,
    inputRange: null,
    items: null,
    checked: 'unchecked',
    sel: 0,
    val: 0,
    min: 0,
    max: 100,
    inc: 1,
    page: 10,
    dropLines: 8,
    horiz: false,
    noThreeD: false,
    macro: null,
    group: null,
    from: [index * 2, 1],
    fromOff: [0, 0],
    to: [index * 2 + 1, 3],
    toOff: [0, 0],
    image: null,
    isNew: true,
    ...extra,
  };
  return info;
}

/** Cells are 50 × 20 pt; rangeRect spans whole cells. */
function fakeHost(list: FormControlInfo[], results: Record<string, unknown> = {}) {
  const calls: SheetCommand[] = [];
  const host = {
    rangeRect: vi.fn(async (_v: SheetView, r0: number, c0: number, r1: number, c1: number) => ({
      x: c0 * 50,
      y: r0 * 20,
      w: (c1 - c0 + 1) * 50,
      h: (r1 - r0 + 1) * 20,
    })),
    hitTest: vi.fn(async () => ({ kind: 'cell', row: 4, col: 2 })),
    exec: vi.fn(async (cmd: SheetCommand) => {
      if (cmd.cmd === 'formControls') return list;
      return results[cmd.cmd] ?? { ok: true };
    }),
  } as unknown as SheetHost;
  const exec = vi.fn(async <T,>(cmd: SheetCommand): Promise<T | null> => {
    calls.push(cmd);
    return (results[cmd.cmd] ?? { ok: true }) as T;
  });
  return { host, exec, calls };
}

function layer(
  list: FormControlInfo[],
  results: Record<string, unknown> = {},
  selected: number | null = null,
) {
  const f = fakeHost(list, results);
  const onSelect = vi.fn();
  const onFormat = vi.fn();
  const onNotice = vi.fn();
  render(
    <I18nextProvider i18n={i18n}>
      <FormControlLayer
        host={f.host}
        sheet={0}
        view={view}
        scale={2}
        revision={0}
        exec={f.exec as unknown as FormControlLayerProps['exec']}
        selected={selected}
        onSelect={onSelect}
        onFormat={onFormat}
        onNotice={onNotice}
      />
    </I18nextProvider>,
  );
  return { ...f, onSelect, onFormat, onNotice };
}

describe('placement', () => {
  it('adds the anchor offsets and scales points to pixels', () => {
    const b = controlBox(
      { fromOff: [5, 2], toOff: [10, 4] },
      { x: 50, y: 20, w: 150, h: 40 },
      { x: 150, y: 40, w: 50, h: 20 },
      2,
    );
    // x = 50 + 5 = 55 pt, right = 150 + 10 = 160 pt; y = 22 pt, bottom = 44 pt.
    expect(b).toEqual({ x: 110, y: 44, w: 210, h: 44 });
  });

  it('falls back to the whole range when the end cell is off screen', async () => {
    const info = ctl('label', 1);
    const b = controlBox(info, { x: 0, y: 0, w: 100, h: 30 }, null, 1);
    expect(b).toEqual({ x: 0, y: 0, w: 100, h: 30 });
    const { host } = fakeHost([info]);
    const placed = await placeControls(host, view, 1, [info]);
    // from (2,1) → x 50, y 40; to (3,3) top-left → x 150, y 60.
    expect(placed[0]!.box).toEqual({ x: 50, y: 40, w: 100, h: 20 });
  });

  it('moves a list selection within its items', () => {
    expect(moveSel(0, 1, 3)).toBe(1);
    expect(moveSel(0, -1, 3)).toBe(3);
    expect(moveSel(3, 1, 3)).toBe(3);
    expect(moveSel(2, -1, 3)).toBe(1);
    expect(moveSel(0, 1, 0)).toBe(0);
  });
});

describe('FormControlLayer', () => {
  it('draws each kind and sends its command', async () => {
    const list = [
      ctl('checkBox', 0, { checked: 'checked', link: '$D$1' }),
      ctl('comboBox', 1, { items: ['Hà Nội', 'Huế'], sel: 1 }),
      ctl('scrollBar', 2, { val: 30 }),
      ctl('spinner', 3, { val: 2, from: [6, 1], to: [9, 1], toOff: [20, 0] }),
      ctl('listBox', 4, { items: ['A', 'B', 'C'], sel: 2 }),
      ctl('optionButton', 5),
      ctl('groupBox', 6, { text: 'Frame' }),
    ];
    const { calls } = layer(list);
    const box = await screen.findByRole('checkbox', { name: 'Caption 0' });
    expect(box.getAttribute('aria-checked')).toBe('true');
    fireEvent.click(box);
    await waitFor(() =>
      expect(calls).toContainEqual({ cmd: 'formControlClick', sheet: 0, index: 0 }),
    );

    fireEvent.change(screen.getByRole('combobox', { name: 'comboBox 1' }), {
      target: { value: '2' },
    });
    expect(calls).toContainEqual({ cmd: 'formControlSelect', sheet: 0, index: 1, sel: 2 });

    const sb = screen.getByRole('scrollbar');
    fireEvent.click(sb.querySelectorAll('button')[1]!);
    expect(calls).toContainEqual({
      cmd: 'formControlValue',
      sheet: 0,
      index: 2,
      step: 1,
      page: false,
    });
    // The upper arrow of a vertical up-down button increases.
    const spin = screen.getByRole('spinbutton');
    fireEvent.click(spin.querySelectorAll('button')[0]!);
    expect(calls.at(-1)).toEqual({
      cmd: 'formControlValue',
      sheet: 0,
      index: 3,
      step: 1,
      page: false,
    });

    const lb = screen.getByRole('listbox');
    const options = within(lb).getAllByRole('option');
    expect(options[1]!.getAttribute('aria-selected')).toBe('true');
    fireEvent.click(options[2]!);
    expect(calls.at(-1)).toEqual({ cmd: 'formControlSelect', sheet: 0, index: 4, sel: 3 });
    fireEvent.keyDown(lb, { key: 'ArrowUp' });
    expect(calls.at(-1)).toEqual({ cmd: 'formControlSelect', sheet: 0, index: 4, sel: 1 });

    fireEvent.click(screen.getByRole('radio', { name: 'Caption 5' }));
    await waitFor(() =>
      expect(calls.at(-1)).toEqual({ cmd: 'formControlClick', sheet: 0, index: 5 }),
    );
    expect(screen.getByText('Frame')).toBeTruthy();
  });

  it('reports a button macro instead of running it', async () => {
    const { onNotice, calls } = layer(
      [ctl('button', 0, { text: 'Báo cáo', macro: 'ChayBaoCao' })],
      {
        formControlClick: { changed: false, macro: 'ChayBaoCao', button: true },
      },
    );
    fireEvent.click(await screen.findByRole('button', { name: 'Báo cáo' }));
    await waitFor(() => expect(onNotice).toHaveBeenCalled());
    expect(calls).toContainEqual({ cmd: 'formControlClick', sheet: 0, index: 0 });
    expect(onNotice.mock.calls[0]![0]).toContain('ChayBaoCao');
    expect(onNotice.mock.calls[0]![0]).toContain('does not run');
  });

  it('shows an ActiveX fallback picture and keeps the control', async () => {
    const { onNotice } = layer([
      ctl('activeX', 0, { name: 'CommandButton1', image: 'data:image/png;base64,AA==' }),
    ]);
    const img = await screen.findByAltText('CommandButton1');
    expect(img.getAttribute('src')).toBe('data:image/png;base64,AA==');
    fireEvent.click(img);
    expect(onNotice.mock.calls[0]![0]).toContain('ActiveX');
  });

  it('selects with a right-click; the selected frame sets up, deletes and moves', async () => {
    const list = [ctl('checkBox', 0)];
    const first = layer(list);
    const el = await screen.findByTestId('form-control-checkBox');
    fireEvent.contextMenu(el);
    expect(first.onSelect).toHaveBeenCalledWith(0);
    cleanup();

    const { calls, onFormat, onSelect } = layer(list, {}, 0);
    const frame = await screen.findByTestId('form-control-selected');
    fireEvent.keyDown(frame, { key: 'Enter' });
    expect(onFormat).toHaveBeenCalledWith(list[0]);
    fireEvent.keyDown(frame, { key: 'Delete' });
    expect(onSelect).toHaveBeenCalledWith(null);
    expect(calls).toContainEqual({ cmd: 'deleteFormControl', sheet: 0, index: 0 });
    // Drag by 40 px: the top-left lands in the hit cell (row 4, col 2).
    fireEvent.mouseDown(frame, { button: 0, clientX: 100, clientY: 100 });
    fireEvent.mouseMove(window, { clientX: 140, clientY: 100 });
    fireEvent.mouseUp(window, { clientX: 140, clientY: 100 });
    await waitFor(() =>
      expect(calls.some((c) => c.cmd === 'moveFormControl' && c.row === 4 && c.col === 2)).toBe(
        true,
      ),
    );
  });
});

describe('FormatControlDialog', () => {
  it('validates limits and sends the settings', async () => {
    const onApply = vi.fn();
    render(
      <I18nextProvider i18n={i18n}>
        <FormatControlDialog
          info={ctl('scrollBar', 0, { link: '$A$1', val: 5 })}
          onClose={() => {}}
          onApply={onApply}
        />
      </I18nextProvider>,
    );
    fireEvent.change(screen.getByTestId('fc-min'), { target: { value: '50' } });
    fireEvent.change(screen.getByTestId('fc-max'), { target: { value: '10' } });
    fireEvent.click(screen.getByTestId('format-control-ok'));
    expect((await screen.findByRole('alert')).textContent).toContain('lowest');
    expect(onApply).not.toHaveBeenCalled();
    fireEvent.change(screen.getByTestId('fc-max'), { target: { value: '90' } });
    fireEvent.change(screen.getByTestId('fc-val'), { target: { value: '60' } });
    fireEvent.change(screen.getByTestId('fc-link'), { target: { value: ' $B$2 ' } });
    fireEvent.click(screen.getByTestId('fc-horiz'));
    fireEvent.click(screen.getByTestId('format-control-ok'));
    await waitFor(() => expect(onApply).toHaveBeenCalled());
    expect(onApply.mock.calls[0]![0]).toEqual({
      name: 'scrollBar 0',
      link: '$B$2',
      val: 60,
      min: 50,
      max: 90,
      inc: 1,
      page: 10,
      horiz: true,
      noThreeD: false,
    });
  });

  it('offers states for a check box and the source range for a list', () => {
    expect(
      specOf('checkBox', {
        name: 'n',
        text: 't',
        link: '',
        inputRange: '',
        checked: 'mixed',
        val: '0',
        min: '0',
        max: '1',
        inc: '1',
        page: '1',
        dropLines: '8',
        noThreeD: true,
        horiz: false,
        macroName: '',
      }),
    ).toEqual({ name: 'n', text: 't', link: '', checked: 'mixed', noThreeD: true });
    expect(
      specOf('comboBox', {
        name: 'n',
        text: '',
        link: '$C$1',
        inputRange: '$A$1:$A$5',
        checked: 'unchecked',
        val: '0',
        min: '0',
        max: '1',
        inc: '1',
        page: '1',
        dropLines: '0',
        noThreeD: false,
        horiz: false,
        macroName: '',
      }),
    ).toBe('sheetEditor.formControl.badNumber');
  });
});
