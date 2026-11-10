// @vitest-environment happy-dom
import { act, cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import {
  ObjectActionsHub,
  areaOrigin,
  columnName,
  resizeGrid,
  useObjectTools,
  type ObjectToolsCommand,
  type ObjectToolsHost,
  type ObjectToolsLabels,
  type ObjectToolsResult,
} from '../src';

afterEach(cleanup);

const labels: ObjectToolsLabels = {
  insertTitle: 'Chèn đối tượng',
  editTitle: 'Sửa sổ làm việc nhúng',
  newTable: 'Mới',
  fromFile: 'Từ tệp',
  browse: 'Chọn tệp',
  noFile: 'Chưa chọn',
  link: 'Liên kết',
  linkHint: 'Chỉ trên máy tính',
  autoUpdate: 'Tự cập nhật',
  range: 'Vùng',
  rangeHint: 'Sheet1!A1:D10',
  addRow: 'Thêm hàng',
  addColumn: 'Thêm cột',
  removeRow: 'Bớt hàng',
  removeColumn: 'Bớt cột',
  ok: 'Đồng ý',
  cancel: 'Hủy',
  cell: ({ address }) => `Ô ${address}`,
};

function Harness(props: { host: ObjectToolsHost }) {
  const tools = useObjectTools(props.host);
  return <>{tools.dialogs}</>;
}

function setup(
  over: Partial<ObjectToolsHost> = {},
  reply: (c: ObjectToolsCommand) => ObjectToolsResult = () => ({}),
) {
  const sent: ObjectToolsCommand[] = [];
  const hub = new ObjectActionsHub();
  const host: ObjectToolsHost = {
    labels,
    run: (c) => {
      sent.push(c);
      return Promise.resolve(reply(c));
    },
    pickFile: () =>
      Promise.resolve({ bytes: new Uint8Array([1, 2, 3]), name: 'b.xlsx', path: 'C:/b.xlsx' }),
    onError: vi.fn(),
    onUpdated: vi.fn(),
    hub,
    ...over,
  };
  render(<Harness host={host} />);
  return { sent, hub, host };
}

describe('object tools helpers', () => {
  it('names columns and reads an area origin', () => {
    expect([0, 25, 26, 27, 701, 702].map(columnName)).toEqual(['A', 'Z', 'AA', 'AB', 'ZZ', 'AAA']);
    expect(areaOrigin('B3:D9')).toEqual({ row: 2, col: 1 });
    expect(areaOrigin("'Sheet 1'!$C$2:$D$4")).toEqual({ row: 1, col: 2 });
    expect(areaOrigin(undefined)).toEqual({ row: 0, col: 0 });
  });

  it('resizes a grid', () => {
    expect(resizeGrid([['a', 'b'], ['c']], 3, 1)).toEqual([['a'], ['c'], ['']]);
    expect(resizeGrid([], 0, 0)).toEqual([['']]);
  });
});

describe('insert object dialog', () => {
  it('inserts a new empty workbook', async () => {
    const { sent, hub } = setup();
    act(() => hub.get()?.['object.insert']());
    fireEvent.click(await screen.findByTestId('object-insert-ok'));
    await waitFor(() => expect(sent).toEqual([{ cmd: 'oleInsert' }]));
  });

  it('embeds or links a chosen file with a range', async () => {
    const { sent, hub } = setup({ readPath: () => Promise.resolve(new Uint8Array()) });
    act(() => hub.get()?.['object.insert']());
    fireEvent.click(await screen.findByTestId('object-from-file'));
    // No file yet: OK is disabled.
    expect((screen.getByTestId('object-insert-ok') as HTMLButtonElement).disabled).toBe(true);
    fireEvent.click(screen.getByTestId('object-browse'));
    await waitFor(() => expect(screen.getByTestId('object-file-name').textContent).toBe('b.xlsx'));
    fireEvent.change(screen.getByTestId('object-range'), { target: { value: ' Sheet1!A1:B2 ' } });
    fireEvent.click(screen.getByTestId('object-link'));
    fireEvent.click(screen.getByTestId('object-insert-ok'));
    await waitFor(() =>
      expect(sent).toEqual([
        {
          cmd: 'oleInsert',
          data: 'AQID',
          range: 'Sheet1!A1:B2',
          link: 'C:/b.xlsx',
          autoUpdate: false,
        },
      ]),
    );
  });

  it('cannot link without file access by path (web)', async () => {
    const { hub } = setup();
    act(() => hub.get()?.['object.insert']());
    fireEvent.click(await screen.findByTestId('object-from-file'));
    fireEvent.click(screen.getByTestId('object-browse'));
    await waitFor(() => expect(screen.getByTestId('object-file-name').textContent).toBe('b.xlsx'));
    expect((screen.getByTestId('object-link') as HTMLInputElement).disabled).toBe(true);
    expect(screen.getByText('Chỉ trên máy tính')).toBeTruthy();
  });
});

describe('edit embedded workbook', () => {
  const info = {
    workbook: true,
    linked: false,
    range: 'B2:C3',
    cells: [
      ['Năm', 'Doanh thu'],
      ['2026', '=10*12'],
    ],
  };

  it('opens the grid on activate and sends the edited cells', async () => {
    const { sent, hub } = setup({}, (c) => (c.cmd === 'oleInfo' ? { ole: info } : {}));
    let found = false;
    await act(async () => {
      found = await hub.activate();
    });
    expect(found).toBe(true);
    expect(sent[0]).toEqual({ cmd: 'oleInfo', cells: true });
    // Cell names follow the area's top-left (B2).
    expect(screen.getByLabelText('Ô C3')).toBeTruthy();
    fireEvent.change(screen.getByTestId('object-cell-1-0'), { target: { value: '2027' } });
    fireEvent.click(screen.getByTestId('object-add-column'));
    fireEvent.change(screen.getByTestId('object-cell-0-2'), { target: { value: 'Lãi' } });
    fireEvent.click(screen.getByTestId('object-edit-ok'));
    await waitFor(() =>
      expect(sent[1]).toEqual({
        cmd: 'oleEdit',
        cells: [
          ['Năm', 'Doanh thu', 'Lãi'],
          ['2027', '=10*12', ''],
        ],
      }),
    );
  });

  it('activate without an object does nothing; a linked one reports why', async () => {
    const onError = vi.fn();
    const { hub } = setup({ onError }, () => ({}));
    let found = true;
    await act(async () => {
      found = await hub.activate();
    });
    expect(found).toBe(false);
    expect(onError).not.toHaveBeenCalled();
    cleanup();
    const second = setup({ onError }, () => ({ ole: { workbook: true, linked: true } }));
    await act(async () => {
      await second.hub.activate();
    });
    expect(onError).toHaveBeenCalledWith('linkedObject');
    expect(screen.queryByTestId('object-edit-dialog')).toBeNull();
  });
});

describe('update links', () => {
  const links = [{ source: 'file:///C:/a.xlsx', path: 'C:/a.xlsx', item: '', count: 2 }];

  it('reports that the web cannot read linked files', async () => {
    const onError = vi.fn();
    const { hub } = setup({ onError }, () => ({ links }));
    act(() => hub.get()?.['object.updateLinks']());
    await waitFor(() => expect(onError).toHaveBeenCalledWith('linksUnavailable'));
  });

  it('reads each source by path and updates', async () => {
    const onUpdated = vi.fn();
    const readPath = vi.fn(() => Promise.resolve(new Uint8Array([255])));
    const { sent, hub } = setup({ onUpdated, readPath }, (c) =>
      c.cmd === 'oleLinks' ? { links } : { count: 2 },
    );
    act(() => hub.get()?.['object.updateLinks']());
    await waitFor(() => expect(onUpdated).toHaveBeenCalledWith(2));
    expect(readPath).toHaveBeenCalledWith('C:/a.xlsx');
    expect(sent[1]).toEqual({
      cmd: 'oleUpdateLinks',
      sources: [{ source: 'file:///C:/a.xlsx', data: '/w==' }],
    });
  });

  it('reports a document without links', async () => {
    const onError = vi.fn();
    const { hub } = setup({ onError }, () => ({ links: [] }));
    act(() => hub.get()?.['object.updateLinks']());
    await waitFor(() => expect(onError).toHaveBeenCalledWith('noLinks'));
  });
});
