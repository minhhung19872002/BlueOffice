// @vitest-environment happy-dom
import { createI18n, I18nextProvider } from '@blueoffice/i18n';
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import type { ReactElement } from 'react';
import type { FilterSpec, SheetCommand, SheetHost } from '@blueoffice/platform';
import {
  AdvancedFilterDialog,
  FilterExtras,
  PERIOD_FILTERS,
  SortDialog,
  levelKeys,
  parseRange,
} from '../src/sortFilter';

afterEach(cleanup);

const i18n = createI18n('en-US');

function fakeHost() {
  const calls: SheetCommand[] = [];
  const host = {
    cell: vi.fn(async (_s: number, _row: number, col: number) => ({
      address: '',
      input: '',
      display: ['Region', 'Name', 'Sales'][col] ?? '',
    })),
    exec: vi.fn(async (cmd: SheetCommand) => {
      calls.push(cmd);
      switch (cmd.cmd) {
        case 'region':
          return { r0: 0, c0: 0, r1: 8, c1: 2 };
        case 'sortLists':
          return [['Low', 'Mid', 'High']];
        case 'sortColors':
          return { cell: [], font: ['000000'], icons: [{ set: '3Arrows', id: 2 }] };
        case 'advancedFilter':
          return { matched: 1, hidden: 7, list: cmd.list, extract: null };
        default:
          return { ok: true };
      }
    }),
  } as unknown as SheetHost;
  return { host, calls };
}

const wrap = (ui: ReactElement) => render(<I18nextProvider i18n={i18n}>{ui}</I18nextProvider>);
const common = (host: SheetHost) => ({
  host,
  sheet: 0,
  onClose: vi.fn(),
  onDone: vi.fn(),
  fail: (e: unknown) => {
    throw e;
  },
});

describe('levelKeys', () => {
  it('maps levels to engine sort keys', () => {
    expect(
      levelKeys([
        { col: 0, on: 'value', order: 'custom', list: 'Low\n Mid \n\nHigh' },
        { col: 1, on: 'value', order: 'desc' },
        { col: 2, on: 'icon', order: 'asc', icon: '3Arrows:2' },
        { col: 2, on: 'cellColor', order: 'desc', color: 'FF0000' },
        // Incomplete colour / icon levels are skipped.
        { col: 2, on: 'fontColor', order: 'asc' },
        { col: 2, on: 'icon', order: 'asc' },
      ]),
    ).toEqual([
      { col: 0, customList: ['Low', 'Mid', 'High'] },
      { col: 1, descending: true },
      { col: 2, descending: false, by: 'icon', iconSet: '3Arrows', iconId: 2 },
      { col: 2, descending: true, by: 'cellColor', color: 'FF0000' },
    ]);
    // An empty custom list falls back to ascending.
    expect(levelKeys([{ col: 3, on: 'value', order: 'custom', list: ' ' }])).toEqual([
      { col: 3, descending: false },
    ]);
  });

  it('parses range addresses', () => {
    expect(parseRange('$a$1:c9')).toEqual({ r0: 0, c0: 0, r1: 8, c1: 2 });
    expect(parseRange('F1')).toEqual({ r0: 0, c0: 5, r1: 0, c1: 5 });
    expect(parseRange('nope')).toBeNull();
  });
});

describe('SortDialog', () => {
  it('sorts by a custom list then by an icon', async () => {
    const { host, calls } = fakeHost();
    wrap(<SortDialog {...common(host)} range={{ r0: 0, c0: 0, r1: 0, c1: 0 }} />);
    await waitFor(() => expect(screen.getByText('Region')).toBeTruthy());
    await waitFor(() => expect(calls.some((c) => c.cmd === 'sortLists')).toBe(true));
    fireEvent.change(screen.getByTestId('sort-order-0'), { target: { value: 'custom' } });
    fireEvent.change(screen.getByTestId('sort-list-preset-0'), { target: { value: '0' } });
    expect((screen.getByTestId('sort-list-0') as HTMLTextAreaElement).value).toBe('Low\nMid\nHigh');
    fireEvent.click(screen.getByTestId('sort-add-level'));
    fireEvent.change(screen.getByTestId('sort-col-1'), { target: { value: '2' } });
    fireEvent.change(screen.getByTestId('sort-by-1'), { target: { value: 'icon' } });
    await waitFor(() => expect(screen.getByText('3Arrows – icon 3')).toBeTruthy());
    fireEvent.change(screen.getByTestId('sort-icon-1'), { target: { value: '3Arrows:2' } });
    fireEvent.change(screen.getByTestId('sort-order-1'), { target: { value: 'desc' } });
    fireEvent.click(screen.getByTestId('sort-ok'));
    await waitFor(() => expect(calls.some((c) => c.cmd === 'sort')).toBe(true));
    expect(calls.find((c) => c.cmd === 'sort')).toEqual({
      cmd: 'sort',
      sheet: 0,
      range: { r0: 0, c0: 0, r1: 8, c1: 2 },
      keys: [
        { col: 0, customList: ['Low', 'Mid', 'High'] },
        { col: 2, descending: true, by: 'icon', iconSet: '3Arrows', iconId: 2 },
      ],
      hasHeader: true,
      leftToRight: false,
    });
  });
});

describe('FilterExtras', () => {
  it('builds top N, period and icon filters', () => {
    const got: FilterSpec[] = [];
    wrap(
      <FilterExtras
        icons={[
          { set: '3Arrows', id: 0 },
          { set: '3Arrows', id: 2 },
        ]}
        top10={null}
        onFilter={(f) => got.push(f)}
      />,
    );
    fireEvent.change(screen.getByTestId('filter-top-dir'), { target: { value: 'bottom' } });
    fireEvent.change(screen.getByTestId('filter-top-n'), { target: { value: '25' } });
    fireEvent.change(screen.getByTestId('filter-top-unit'), { target: { value: 'percent' } });
    fireEvent.click(screen.getByTestId('filter-top-apply'));
    fireEvent.change(screen.getByTestId('filter-period'), { target: { value: 'Q3' } });
    fireEvent.click(screen.getByTestId('filter-icon-3Arrows-2'));
    fireEvent.click(screen.getByTestId('filter-icon-3Arrows-none'));
    expect(got).toEqual([
      { top10: { top: false, percent: true, val: 25 } },
      { dynamic: 'Q3' },
      { icon: { set: '3Arrows', id: 2 } },
      { icon: { set: '3Arrows' } },
    ]);
    expect(PERIOD_FILTERS).toContain('M12');
    expect(screen.getByText('Quarter 3 (any year)')).toBeTruthy();
  });
});

describe('AdvancedFilterDialog', () => {
  it('copies unique records and rejects bad addresses', async () => {
    const { host, calls } = fakeHost();
    const c = common(host);
    wrap(<AdvancedFilterDialog {...c} range={{ r0: 0, c0: 0, r1: 0, c1: 0 }} />);
    await waitFor(() =>
      expect((screen.getByTestId('adv-list') as HTMLInputElement).value).toBe('A1:C9'),
    );
    fireEvent.click(screen.getByTestId('adv-ok'));
    expect(screen.getByTestId('adv-error').textContent).toBe('The range address is not valid.');
    fireEvent.change(screen.getByTestId('adv-criteria'), { target: { value: 'F1:G3' } });
    fireEvent.click(screen.getByTestId('adv-copy'));
    fireEvent.change(screen.getByTestId('adv-copy-to'), { target: { value: 'I1' } });
    fireEvent.click(screen.getByTestId('adv-unique'));
    fireEvent.click(screen.getByTestId('adv-ok'));
    await waitFor(() => expect(c.onClose).toHaveBeenCalled());
    expect(calls.find((x) => x.cmd === 'advancedFilter')).toEqual({
      cmd: 'advancedFilter',
      sheet: 0,
      list: { r0: 0, c0: 0, r1: 8, c1: 2 },
      criteria: { r0: 0, c0: 5, r1: 2, c1: 6 },
      copyTo: { r0: 0, c0: 8, r1: 0, c1: 8 },
      unique: true,
    });
    expect(c.onDone).toHaveBeenCalled();
  });
});
