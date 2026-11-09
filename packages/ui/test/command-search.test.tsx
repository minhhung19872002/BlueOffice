// @vitest-environment happy-dom
import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import {
  CommandSearch,
  flattenRibbon,
  isCommandSearchKey,
  normalize,
  searchCommands,
  type RibbonTab,
} from '../src';

afterEach(cleanup);

const bold = vi.fn();
const tabs: RibbonTab[] = [
  {
    id: 'home',
    label: 'Trang đầu',
    groups: [
      {
        id: 'font',
        label: 'Phông chữ',
        items: [
          { id: 'format.bold', label: 'Đậm', shortcut: 'Ctrl+B', onSelect: bold },
          { id: 'format.italic', label: 'Nghiêng', onSelect: vi.fn() },
          { id: 'format.clear', label: 'Xoá định dạng', onSelect: vi.fn(), enabled: false },
          { id: 'font.family', label: 'font.family', onSelect: vi.fn(), render: 'x' },
        ],
      },
    ],
  },
  {
    id: 'insert',
    label: 'Chèn',
    groups: [
      {
        id: 'tables',
        label: 'Bảng',
        items: [
          { id: 'insert.table', label: 'Bảng', onSelect: vi.fn() },
          { id: 'format.bold', label: 'Đậm (trùng)', onSelect: vi.fn() },
          { id: 'format.painter', label: 'Chổi định dạng', onSelect: vi.fn() },
        ],
      },
    ],
  },
];

describe('command search ranking', () => {
  it('normalizes Vietnamese diacritics and đ', () => {
    expect(normalize('  Định Dạng  ')).toBe('dinh dang');
    expect(normalize('Xoá')).toBe('xoa');
  });

  it('flattens the ribbon once per command, without custom controls', () => {
    const e = flattenRibbon(tabs);
    expect(e.map((x) => x.id)).toEqual([
      'format.bold',
      'format.italic',
      'format.clear',
      'insert.table',
      'format.painter',
    ]);
    expect(e[0]!.path).toBe('Trang đầu › Phông chữ');
  });

  it('ranks prefix > word prefix > substring > words anywhere, enabled first', () => {
    const e = flattenRibbon(tabs);
    // "dinh" starts a word of "Xoá định dạng" (disabled) and of "Chổi định dạng".
    expect(searchCommands(e, 'dinh').map((x) => x.id)).toEqual(['format.painter', 'format.clear']);
    expect(searchCommands(e, 'dam').map((x) => x.id)).toEqual(['format.bold']);
    // Words found across label and path ("Chèn › Bảng"): both commands of that group, in ribbon order.
    expect(searchCommands(e, 'chen bang').map((x) => x.id)).toEqual([
      'insert.table',
      'format.painter',
    ]);
    expect(searchCommands(e, '   ')).toEqual([]);
  });

  it('recognizes Ctrl+Q and Alt+Q only', () => {
    const k = (o: Partial<KeyboardEvent>) =>
      isCommandSearchKey({
        ctrlKey: false,
        altKey: false,
        shiftKey: false,
        metaKey: false,
        key: 'q',
        ...o,
      });
    expect(k({ ctrlKey: true })).toBe(true);
    expect(k({ altKey: true, key: 'Q' })).toBe(true);
    expect(k({})).toBe(false);
    expect(k({ ctrlKey: true, shiftKey: true })).toBe(false);
  });
});

describe('CommandSearch', () => {
  const labels = { title: 'Tìm lệnh', placeholder: 'Tìm lệnh (Alt+Q)', noResults: 'Không có' };

  it('runs the chosen command with the keyboard and closes', () => {
    const onClose = vi.fn();
    render(<CommandSearch tabs={tabs} open onClose={onClose} labels={labels} />);
    const input = screen.getByTestId('command-search-input');
    fireEvent.change(input, { target: { value: 'nghieng' } });
    fireEvent.change(input, { target: { value: 'dam' } });
    expect(screen.getAllByRole('option')).toHaveLength(1);
    fireEvent.keyDown(input, { key: 'Enter' });
    expect(bold).toHaveBeenCalledOnce();
    expect(onClose).toHaveBeenCalledOnce();
  });

  it('does not run disabled commands and shows an empty state', () => {
    const onClose = vi.fn();
    render(<CommandSearch tabs={tabs} open onClose={onClose} labels={labels} />);
    const input = screen.getByTestId('command-search-input');
    fireEvent.change(input, { target: { value: 'xoa' } });
    const opt = screen.getByRole('option');
    expect(opt.getAttribute('aria-disabled')).toBe('true');
    fireEvent.click(opt);
    expect(onClose).not.toHaveBeenCalled();
    fireEvent.change(input, { target: { value: 'zzz' } });
    expect(screen.getByText('Không có')).toBeTruthy();
    fireEvent.keyDown(input, { key: 'Escape' });
    expect(onClose).toHaveBeenCalledOnce();
  });
});
