// @vitest-environment happy-dom
import { createI18n, I18nextProvider } from '@blueoffice/i18n';
import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import type { ReactNode } from 'react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import type { SheetCommand, SparklineGroupInfo } from '@blueoffice/platform';
import {
  EditSparklineDataDialog,
  InsertSparklinesDialog,
  dataBlock,
  groupLocation,
  SparklineOptionsDialog,
  groupAt,
  optionsPatch,
  parseRef,
  proposeLocation,
  registerSparklineCommands,
} from '../src/sparklines';

afterEach(cleanup);

const i18n = createI18n('en-US');

const group = (over: Partial<SparklineGroupInfo> = {}): SparklineGroupInfo => ({
  index: 0,
  type: 'line',
  sparklines: [
    { formula: 'Sheet1!A1:D1', location: { row: 0, col: 5 } },
    { formula: 'Sheet1!A2:D2', location: { row: 1, col: 5 } },
  ],
  emptyAs: 'gap',
  markers: false,
  high: true,
  low: false,
  first: false,
  last: false,
  negative: false,
  displayXAxis: false,
  displayHidden: false,
  rightToLeft: false,
  minAxis: 'individual',
  maxAxis: 'individual',
  manualMin: null,
  manualMax: null,
  lineWeight: 0.75,
  dateAxis: false,
  dateRange: null,
  colorsHex: {
    series: '#2E6DB4',
    negative: '#D64545',
    axis: '#000000',
    markers: '#1F4E8C',
    first: '#F28C28',
    last: '#F28C28',
    high: '#2FA36B',
    low: '#D64545',
  },
  ...over,
});

describe('helpers', () => {
  it('parses plain and sheet-qualified ranges', () => {
    expect(parseRef('b2:e5')).toEqual({ sheet: null, range: { r0: 1, c0: 1, r1: 4, c1: 4 } });
    expect(parseRef("'Bảng 1'!$A$1:$A$3")).toEqual({
      sheet: 'Bảng 1',
      range: { r0: 0, c0: 0, r1: 2, c1: 0 },
    });
    expect(parseRef('Sheet2!C3')?.range).toEqual({ r0: 2, c0: 2, r1: 2, c1: 2 });
    expect(parseRef('!A1')).toBeNull();
    expect(parseRef('nope')).toBeNull();
  });

  it('proposes the column right of the data', () => {
    // B2:E5 → F2:F5.
    expect(proposeLocation({ r0: 1, c0: 1, r1: 4, c1: 4 })).toEqual({ r0: 1, c0: 5, r1: 4, c1: 5 });
  });

  it('finds the group of a cell', () => {
    const gs = [
      group(),
      group({ index: 1, sparklines: [{ formula: null, location: { row: 3, col: 2 } }] }),
    ];
    expect(groupAt(gs, 1, 5)?.index).toBe(0);
    expect(groupAt(gs, 3, 2)?.index).toBe(1);
    expect(groupAt(gs, 0, 0)).toBeNull();
  });

  it('sends custom bounds only for custom axes', () => {
    const p = optionsPatch(group({ minAxis: 'custom', manualMin: -1, manualMax: 9 }));
    expect(p.manualMin).toBe(-1);
    expect(p.manualMax).toBeUndefined();
    expect(p.colorHigh).toBe('#2FA36B');
    expect(p.emptyAs).toBe('gap');
  });
});

describe('EditSparklineDataDialog', () => {
  const wrap = (n: ReactNode) => render(<I18nextProvider i18n={i18n}>{n}</I18nextProvider>);

  it('proposes the block of data and the cells of the group', async () => {
    expect(dataBlock(['Sheet1!A1:D1', 'Sheet1!A2:D2'])).toBe('Sheet1!A1:D2');
    expect(dataBlock(["'Bảng 1'!A1:A3", "'Bảng 1'!B1:B3"])).toBe("'Bảng 1'!A1:B3");
    expect(dataBlock(['Sheet1!A1:D1', 'Sheet2!A2:D2'])).toBe('');
    expect(groupLocation(group())).toEqual({ r0: 0, c0: 5, r1: 1, c1: 5 });
    const exec = vi.fn(async (_c: SheetCommand) => ({ ok: true }));
    const onClose = vi.fn();
    wrap(<EditSparklineDataDialog sheet={0} group={group()} exec={exec} onClose={onClose} />);
    expect((screen.getByTestId('sparkline-edit-data-range') as HTMLInputElement).value).toBe(
      'Sheet1!A1:D2',
    );
    expect((screen.getByTestId('sparkline-edit-data-location') as HTMLInputElement).value).toBe(
      'F1:F2',
    );
    fireEvent.change(screen.getByTestId('sparkline-edit-data-location'), {
      target: { value: 'G1:G2' },
    });
    fireEvent.click(screen.getByTestId('sparkline-edit-data-ok'));
    await vi.waitFor(() => expect(onClose).toHaveBeenCalled());
    expect(exec).toHaveBeenCalledWith({
      cmd: 'sparklineData',
      sheet: 0,
      group: 0,
      data: 'Sheet1!A1:D2',
      location: { r0: 0, c0: 6, r1: 1, c1: 6 },
    });
  });

  it('edits the data of one sparkline', async () => {
    const exec = vi.fn(async (_c: SheetCommand) => ({ ok: true }));
    const onClose = vi.fn();
    wrap(
      <EditSparklineDataDialog
        sheet={1}
        group={group()}
        single={{ row: 1, col: 5 }}
        exec={exec}
        onClose={onClose}
      />,
    );
    expect(screen.queryByTestId('sparkline-edit-data-location')).toBeNull();
    const input = screen.getByTestId('sparkline-edit-data-range') as HTMLInputElement;
    expect(input.value).toBe('Sheet1!A2:D2');
    fireEvent.change(input, { target: { value: '???' } });
    fireEvent.click(screen.getByTestId('sparkline-edit-data-ok'));
    expect(screen.getByRole('alert').textContent).toBe('Enter a valid cell range.');
    fireEvent.change(input, { target: { value: 'B2:E2' } });
    fireEvent.click(screen.getByTestId('sparkline-edit-data-ok'));
    await vi.waitFor(() => expect(onClose).toHaveBeenCalled());
    expect(exec).toHaveBeenCalledWith({
      cmd: 'sparklineSingleData',
      sheet: 1,
      row: 1,
      col: 5,
      data: 'B2:E2',
    });
  });
});

describe('commands', () => {
  type St = { sheet: number; hasWorkbook: boolean; sparkline: SparklineGroupInfo | null };
  function setup(current: SparklineGroupInfo | null) {
    const cmds = new Map<
      string,
      {
        run: (s: St) => Promise<unknown>;
        isEnabled?: (s: St) => boolean;
        isChecked?: (s: St) => boolean;
      }
    >();
    const exec = vi.fn(async (_c: SheetCommand) => ({ ok: true }));
    const openInsert = vi.fn();
    const openEditData = vi.fn();
    registerSparklineCommands<St>((id, run, extra) => cmds.set(id, { run, ...extra }), {
      range: () => ({ r0: 0, c0: 5, r1: 1, c1: 5 }),
      exec,
      openInsert,
      openOptions: vi.fn(),
      openEditData,
    });
    const st: St = { sheet: 2, hasWorkbook: true, sparkline: current };
    return { cmds, exec, openInsert, openEditData, st };
  }

  it('opens Edit data and switches rows and columns', async () => {
    const { cmds, exec, openEditData, st } = setup(group());
    await cmds.get('sparkline.editData')?.run(st);
    expect(openEditData).toHaveBeenLastCalledWith('group');
    await cmds.get('sparkline.editSingle')?.run(st);
    expect(openEditData).toHaveBeenLastCalledWith('single');
    await cmds.get('sparkline.switch')?.run(st);
    expect(exec).toHaveBeenLastCalledWith({ cmd: 'sparklineSwitch', sheet: 2, group: 0 });
    // One sparkline: nothing to switch.
    const one = group({ sparklines: [{ formula: 'Sheet1!A1:D1', location: { row: 0, col: 5 } }] });
    expect(cmds.get('sparkline.switch')?.isEnabled?.({ ...st, sparkline: one })).toBe(false);
    expect(cmds.get('sparkline.editData')?.isEnabled?.({ ...st, sparkline: null })).toBe(false);
  });

  it('toggles flags and switches the type of the current group', async () => {
    const { cmds, exec, st } = setup(group());
    expect(cmds.get('sparkline.high')?.isChecked?.(st)).toBe(true);
    expect(cmds.get('sparkline.typeLine')?.isChecked?.(st)).toBe(true);
    await cmds.get('sparkline.high')?.run(st);
    expect(exec).toHaveBeenLastCalledWith({
      cmd: 'sparklineEdit',
      sheet: 2,
      group: 0,
      high: false,
    });
    await cmds.get('sparkline.typeWinLoss')?.run(st);
    expect(exec).toHaveBeenLastCalledWith({
      cmd: 'sparklineEdit',
      sheet: 2,
      group: 0,
      type: 'stacked',
    });
    await cmds.get('sparkline.clearGroup')?.run(st);
    expect(exec).toHaveBeenLastCalledWith({
      cmd: 'sparklineClear',
      sheet: 2,
      range: { r0: 0, c0: 5, r1: 1, c1: 5 },
      wholeGroups: true,
    });
  });

  it('disables the tab commands outside a sparkline', () => {
    const { cmds, st } = setup(null);
    expect(cmds.get('sparkline.high')?.isEnabled?.(st)).toBe(false);
    expect(cmds.get('sparkline.ungroup')?.isEnabled?.(st)).toBe(false);
    expect(cmds.get('insert.sparklines')?.isEnabled?.(st)).toBe(true);
  });

  it('allows markers only on line sparklines', () => {
    const { cmds, st } = setup(group({ type: 'column' }));
    expect(cmds.get('sparkline.markers')?.isEnabled?.(st)).toBe(false);
    expect(cmds.get('sparkline.negative')?.isEnabled?.(st)).toBe(true);
  });
});

describe('InsertSparklinesDialog', () => {
  it('proposes the location and inserts column sparklines', async () => {
    const exec = vi.fn(async (_c: SheetCommand) => ({ index: 0 }));
    const onClose = vi.fn();
    render(
      <I18nextProvider i18n={i18n}>
        <InsertSparklinesDialog
          sheet={0}
          selection={{ r0: 1, c0: 1, r1: 4, c1: 4 }}
          exec={exec}
          onClose={onClose}
        />
      </I18nextProvider>,
    );
    expect((screen.getByTestId('sparkline-data') as HTMLInputElement).value).toBe('B2:E5');
    expect((screen.getByTestId('sparkline-location') as HTMLInputElement).value).toBe('F2:F5');
    fireEvent.click(screen.getByTestId('sparkline-type-column'));
    fireEvent.click(screen.getByTestId('sparkline-insert-ok'));
    await vi.waitFor(() => expect(onClose).toHaveBeenCalled());
    expect(exec).toHaveBeenCalledWith({
      cmd: 'sparklineInsert',
      sheet: 0,
      type: 'column',
      data: 'B2:E5',
      location: { r0: 1, c0: 5, r1: 4, c1: 5 },
    });
  });

  it('refuses a two-dimensional location', () => {
    const exec = vi.fn(async (_c: SheetCommand) => ({ index: 0 }));
    render(
      <I18nextProvider i18n={i18n}>
        <InsertSparklinesDialog
          sheet={0}
          selection={{ r0: 0, c0: 0, r1: 0, c1: 0 }}
          exec={exec}
          onClose={vi.fn()}
        />
      </I18nextProvider>,
    );
    fireEvent.change(screen.getByTestId('sparkline-data'), { target: { value: 'A1:D1' } });
    fireEvent.change(screen.getByTestId('sparkline-location'), { target: { value: 'F1:G2' } });
    fireEvent.click(screen.getByTestId('sparkline-insert-ok'));
    expect(screen.getByRole('alert').textContent).toBe('Enter a valid cell range.');
    expect(exec).not.toHaveBeenCalled();
  });
});

describe('SparklineOptionsDialog', () => {
  it('sends the axis and empty-cell settings', async () => {
    const exec = vi.fn(async (_c: SheetCommand) => ({ ok: true }));
    const onClose = vi.fn();
    render(
      <I18nextProvider i18n={i18n}>
        <SparklineOptionsDialog
          sheet={1}
          group={group({ index: 3 })}
          exec={exec}
          onClose={onClose}
        />
      </I18nextProvider>,
    );
    fireEvent.change(screen.getByTestId('sparkline-minAxis'), { target: { value: 'custom' } });
    fireEvent.change(screen.getByTestId('sparkline-manualMin'), { target: { value: '-5' } });
    fireEvent.change(screen.getByTestId('sparkline-maxAxis'), { target: { value: 'group' } });
    fireEvent.change(screen.getByTestId('sparkline-emptyAs'), { target: { value: 'span' } });
    fireEvent.click(screen.getByTestId('sparkline-displayXAxis'));
    fireEvent.click(screen.getByTestId('sparkline-options-ok'));
    await vi.waitFor(() => expect(onClose).toHaveBeenCalled());
    const cmd = exec.mock.calls[0]![0] as Record<string, unknown>;
    expect(cmd).toMatchObject({
      cmd: 'sparklineEdit',
      sheet: 1,
      group: 3,
      minAxis: 'custom',
      manualMin: -5,
      maxAxis: 'group',
      emptyAs: 'span',
      displayXAxis: true,
      colorSeries: '#2E6DB4',
    });
    expect(cmd.manualMax).toBeUndefined();
  });

  it('refuses a custom minimum above the maximum', () => {
    const exec = vi.fn(async (_c: SheetCommand) => ({ ok: true }));
    render(
      <I18nextProvider i18n={i18n}>
        <SparklineOptionsDialog
          sheet={0}
          group={group({ minAxis: 'custom', maxAxis: 'custom', manualMin: 5, manualMax: 1 })}
          exec={exec}
          onClose={vi.fn()}
        />
      </I18nextProvider>,
    );
    fireEvent.click(screen.getByTestId('sparkline-options-ok'));
    expect(screen.getByRole('alert').textContent).toBe(
      'The minimum must be less than the maximum.',
    );
    expect(exec).not.toHaveBeenCalled();
  });
});
