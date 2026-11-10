// @vitest-environment happy-dom
import { createI18n, I18nextProvider } from '@blueoffice/i18n';
import type {
  SheetChange,
  SheetCommand,
  SheetCompareResult,
  VersionHistory,
} from '@blueoffice/platform';
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import type { ReactNode } from 'react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { SheetShowChangesPane, changeTarget, sheetChangeRows } from '../src/showChanges';

afterEach(cleanup);

const i18n = createI18n('en-US');
const t = i18n.t.bind(i18n) as (k: string, o?: Record<string, unknown>) => string;
const wrap = (n: ReactNode) => render(<I18nextProvider i18n={i18n}>{n}</I18nextProvider>);

const changes: SheetChange[] = [
  { kind: 'rowsInserted', sheet: 'Data', sheetIndex: 0, address: '3', row: 2, count: 1 },
  {
    kind: 'valueChanged',
    sheet: 'Data',
    sheetIndex: 0,
    address: 'B2',
    row: 1,
    col: 1,
    old: '3',
    new: '6',
  },
  {
    kind: 'formulaChanged',
    sheet: 'Sum',
    sheetIndex: 1,
    address: 'A1',
    oldAddress: 'A2',
    row: 0,
    col: 0,
    old: '=Data!B2',
    new: '=Data!B2*2',
  },
  { kind: 'sheetRemoved', sheet: 'Old', sheetIndex: null, address: '' },
];

function history(): VersionHistory & { restore: ReturnType<typeof vi.fn> } {
  return {
    fileName: 'Bảng.xlsx',
    canRestore: () => true,
    list: async () => [
      {
        version: '3',
        modifiedAt: '2026-11-10T08:30:00Z',
        modifiedBy: { id: 'u1', name: 'An' },
        note: null,
      },
      { version: '2', modifiedAt: '2026-11-09T10:00:00Z', modifiedBy: { id: 'u2', name: 'Bình' } },
    ],
    contents: async (v) => new Uint8Array([Number(v)]),
    restore: vi.fn(async () => ({ version: '4' })),
  };
}

describe('BlueSheet Show Changes', () => {
  it('turns engine changes into rows and cell targets', () => {
    const rows = sheetChangeRows(changes, t);
    expect(rows.map((r) => [r.group, r.kind, r.where])).toEqual([
      ['Data', 'Rows inserted (1)', '3'],
      ['Data', 'Value changed', 'B2'],
      ['Sum', 'Formula changed', 'A1 (was A2)'],
      ['Old', 'Sheet removed', ''],
    ]);
    expect(changeTarget(changes[0]!)).toEqual({ sheet: 0, r0: 2, c0: 0, r1: 2, c1: 0 });
    expect(changeTarget(changes[1]!)).toEqual({ sheet: 0, r0: 1, c0: 1, r1: 1, c1: 1 });
    expect(changeTarget(changes[3]!)).toBeNull();
  });

  it('compares with a chosen version, lists changes by sheet, selects a cell, restores', async () => {
    const sent: SheetCommand[] = [];
    const exec = async <T,>(c: SheetCommand): Promise<T | null> => {
      sent.push(c);
      const r: SheetCompareResult = { changes, total: changes.length, truncated: false };
      return r as T;
    };
    const goTo = vi.fn();
    const versions = history();
    wrap(<SheetShowChangesPane exec={exec} versions={versions} goTo={goTo} onClose={() => {}} />);
    await waitFor(() => expect(screen.getAllByTestId('version-item')).toHaveLength(2));
    expect(screen.getByText('Version 3 (current)')).toBeTruthy();
    expect(screen.getByText(/^Bình · /)).toBeTruthy();

    fireEvent.click(screen.getAllByTestId('version-compare')[1]!);
    await waitFor(() => expect(screen.getAllByTestId('change-item')).toHaveLength(4));
    expect(sent[0]).toEqual({ cmd: 'compareWorkbook', name: 'Bảng.xlsx', base64: 'Ag==' });
    expect(screen.getByText('Changes since version 2')).toBeTruthy();
    expect(screen.getByText('4 change(s)')).toBeTruthy();
    expect(screen.getByText('Data')).toBeTruthy();
    expect(screen.getByText('Sum')).toBeTruthy();

    fireEvent.click(screen.getAllByTestId('change-item')[1]!);
    expect(goTo).toHaveBeenCalledWith({ sheet: 0, r0: 1, c0: 1, r1: 1, c1: 1 });

    fireEvent.change(screen.getByLabelText('Filter changes'), { target: { value: 'formula' } });
    expect(screen.getAllByTestId('change-item')).toHaveLength(1);

    // Only older versions can be restored, after a confirmation.
    expect(screen.getAllByTestId('version-restore')).toHaveLength(1);
    fireEvent.click(screen.getByTestId('version-restore'));
    fireEvent.click(screen.getByTestId('version-restore-yes'));
    await waitFor(() => expect(versions.restore).toHaveBeenCalledWith('2'));
  });

  it('without a host history it compares with a file and reports no changes', async () => {
    const exec = async <T,>(): Promise<T | null> =>
      ({ changes: [], total: 0, truncated: false }) as T;
    const openFile = vi.fn(async () => ({ name: 'old.xlsx', bytes: new Uint8Array([1]) }));
    wrap(
      <SheetShowChangesPane exec={exec} openFile={openFile} goTo={() => {}} onClose={() => {}} />,
    );
    expect(screen.getByText(/Version history is kept for files opened/)).toBeTruthy();
    fireEvent.click(screen.getByTestId('versions-compare-file'));
    await waitFor(() => expect(screen.getByText('No changes.')).toBeTruthy());
    expect(screen.getByText('Changes compared with old.xlsx')).toBeTruthy();
  });

  it('shows an error when the comparison fails', async () => {
    const exec = async <T,>(): Promise<T | null> => null;
    wrap(
      <SheetShowChangesPane exec={exec} versions={history()} goTo={() => {}} onClose={() => {}} />,
    );
    await waitFor(() => expect(screen.getAllByTestId('version-compare')).toHaveLength(2));
    const spy = vi.spyOn(console, 'error').mockImplementation(() => {});
    fireEvent.click(screen.getAllByTestId('version-compare')[0]!);
    await waitFor(() => expect(screen.getByRole('alert')).toBeTruthy());
    spy.mockRestore();
  });
});
