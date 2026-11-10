// @vitest-environment happy-dom
import { act, cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { assignKeyTips, keyTipHit, keyTipMatches, Ribbon, type RibbonTab } from '../src';

afterEach(cleanup);

const prefixFree = (codes: string[]) =>
  codes.every((a, i) => codes.every((b, j) => i === j || !b.startsWith(a)));

describe('KeyTip assignment (CORE-UI-004)', () => {
  it('derives letters from the labels (Vietnamese diacritics folded)', () => {
    const m = assignKeyTips(
      [
        { id: 'tab:home', label: 'Trang đầu' },
        { id: 'tab:insert', label: 'Chèn' },
        { id: 'tab:draw', label: 'Vẽ' },
        { id: 'tab:design', label: 'Thiết kế' },
        { id: 'tab:layout', label: 'Bố cục' },
      ],
      {},
    );
    expect(Object.fromEntries(m)).toEqual({
      'tab:home': 'T',
      'tab:insert': 'C',
      'tab:draw': 'V',
      'tab:design': 'K',
      'tab:layout': 'B',
    });
  });

  it('is unique, prefix-free and stable when entries are added at the end', () => {
    const labels = ['Đậm', 'Nghiêng', 'Gạch chân', 'Gạch ngang', 'Chỉ số dưới', 'Chỉ số trên'];
    const base = labels.map((l, i) => ({ id: `c${i}`, label: l }));
    const a = assignKeyTips(base, {});
    const codes = [...a.values()];
    expect(new Set(codes).size).toBe(codes.length);
    expect(prefixFree(codes)).toBe(true);
    const more = assignKeyTips([...base, { id: 'x', label: 'Đậm hơn' }], {});
    for (const [id, c] of a) expect(more.get(id)).toBe(c);
    // Same labels, same codes.
    expect(assignKeyTips(base, {})).toEqual(a);
  });

  it('uses two-letter codes when letters run out, still prefix-free', () => {
    const entries = Array.from({ length: 40 }, (_, i) => ({ id: `c${i}`, label: `Lệnh ${i}` }));
    const m = assignKeyTips(entries, {});
    const codes = [...m.values()];
    expect(m.size).toBe(40);
    expect(new Set(codes).size).toBe(40);
    expect(prefixFree(codes)).toBe(true);
    expect(codes.some((c) => c.length === 2)).toBe(true);
  });

  it('applies overrides first and never hands out reserved codes', () => {
    const m = assignKeyTips(
      [
        { id: 'format.bold', label: 'Đậm' },
        { id: 'other', label: 'Đổi' },
        { id: 'file', label: 'Tệp' },
      ],
      { 'format.bold': '1', file: 'F' },
      ['D'],
    );
    expect(m.get('format.bold')).toBe('1');
    expect(m.get('file')).toBe('F');
    expect(m.get('other')).not.toBe('D');
    expect(keyTipHit(m, '1')).toBe('format.bold');
    expect(keyTipMatches(m, '')).toHaveLength(3);
  });
});

describe('KeyTips on the ribbon', () => {
  const bold = vi.fn();
  const table = vi.fn();
  const tabs: RibbonTab[] = [
    {
      id: 'home',
      label: 'Trang đầu',
      groups: [
        {
          id: 'font',
          label: 'Phông chữ',
          items: [
            { id: 'format.bold', label: 'Đậm', onSelect: bold },
            { id: 'format.italic', label: 'Nghiêng', onSelect: vi.fn() },
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
          items: [{ id: 'insert.table', label: 'Bảng', onSelect: table }],
        },
      ],
    },
  ];
  const alt = () => {
    fireEvent.keyDown(window, { key: 'Alt', code: 'AltLeft', altKey: true });
    fireEvent.keyUp(window, { key: 'Alt', code: 'AltLeft' });
  };
  const key = (k: string) => fireEvent.keyDown(window, { key: k.toLowerCase(), code: `Key${k}` });
  const tips = () =>
    Object.fromEntries(
      [...document.querySelectorAll('[data-keytip-for]')].map((e) => [
        e.getAttribute('data-keytip-for'),
        e.getAttribute('data-keytip'),
      ]),
    );

  it('Alt shows tab letters, a letter opens the tab, its letters run commands', async () => {
    render(<Ribbon tabs={tabs} fileMenu={{ label: 'Tệp', onOpen: vi.fn() }} />);
    expect(screen.queryByTestId('keytips')).toBeNull();
    await act(async () => alt());
    expect(tips()).toEqual({ file: 'F', 'tab:home': 'T', 'tab:insert': 'C' });
    await act(async () => key('C'));
    expect(screen.getByTestId('keytips').getAttribute('data-level')).toBe('items');
    expect(tips()).toEqual({ 'insert.table': 'B' });
    // Escape steps back to the tab strip, then off.
    await act(async () => fireEvent.keyDown(window, { key: 'Escape' }));
    expect(screen.getByTestId('keytips').getAttribute('data-level')).toBe('tabs');
    await act(async () => key('T'));
    expect(tips()['format.bold']).toBe('1');
    await act(async () => fireEvent.keyDown(window, { key: '1', code: 'Digit1' }));
    expect(bold).toHaveBeenCalledTimes(1);
    expect(screen.queryByTestId('keytips')).toBeNull();
  });

  it('Alt with another key (Alt+Q) does not show KeyTips; Escape turns them off', async () => {
    render(<Ribbon tabs={tabs} />);
    await act(async () => {
      fireEvent.keyDown(window, { key: 'Alt', altKey: true });
      fireEvent.keyDown(window, { key: 'q', code: 'KeyQ', altKey: true });
      fireEvent.keyUp(window, { key: 'Alt' });
    });
    expect(screen.queryByTestId('keytips')).toBeNull();
    await act(async () => alt());
    expect(screen.getByTestId('keytips')).toBeTruthy();
    await act(async () => fireEvent.keyDown(window, { key: 'Escape' }));
    expect(screen.queryByTestId('keytips')).toBeNull();
  });
});
