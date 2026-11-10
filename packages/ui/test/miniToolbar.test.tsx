// @vitest-environment happy-dom
import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import {
  CommandSearch,
  MINI_CLOSE_AT,
  MINI_FADE_FROM,
  MiniToolbar,
  miniOpacity,
  pushRecentCommand,
  rectDistance,
  relatedCommands,
  flattenRibbon,
  type RibbonTab,
} from '../src';

afterEach(cleanup);

describe('mini toolbar (CORE-UI-003)', () => {
  it('fades with the pointer distance and closes far away', () => {
    expect(miniOpacity(0)).toBe(1);
    expect(miniOpacity(MINI_FADE_FROM)).toBe(1);
    const mid = miniOpacity((MINI_FADE_FROM + MINI_CLOSE_AT) / 2);
    expect(mid).toBeLessThan(1);
    expect(mid).toBeGreaterThan(0);
    expect(miniOpacity(MINI_CLOSE_AT + 1)).toBe(0);
    expect(rectDistance({ left: 0, top: 0, right: 10, bottom: 10 }, 5, 5)).toBe(0);
    expect(rectDistance({ left: 0, top: 0, right: 10, bottom: 10 }, 13, 14)).toBe(5);
  });

  it('runs its commands and closes on typing', () => {
    const bold = vi.fn();
    const onClose = vi.fn();
    render(
      <MiniToolbar
        x={100}
        y={100}
        label="Mini"
        onClose={onClose}
        items={[
          { id: 'format.bold', label: 'Đậm', onSelect: bold },
          { id: 'format.italic', label: 'Nghiêng', enabled: false, onSelect: vi.fn() },
        ]}
      />,
    );
    expect(screen.getByRole('toolbar', { name: 'Mini' })).toBeTruthy();
    fireEvent.click(screen.getByRole('button', { name: 'Đậm' }));
    expect(bold).toHaveBeenCalled();
    fireEvent.keyDown(window, { key: 'Shift' });
    expect(onClose).not.toHaveBeenCalled();
    fireEvent.keyDown(window, { key: 'a' });
    expect(onClose).toHaveBeenCalled();
  });
});

describe('command search suggestions', () => {
  const tabs: RibbonTab[] = [
    {
      id: 'home',
      label: 'Trang đầu',
      groups: [
        {
          id: 'font',
          label: 'Phông chữ',
          items: [
            { id: 'format.bold', label: 'Đậm', onSelect: vi.fn() },
            { id: 'format.italic', label: 'Nghiêng', onSelect: vi.fn() },
            { id: 'format.underline', label: 'Gạch chân', onSelect: vi.fn() },
          ],
        },
      ],
    },
  ];
  const labels = {
    title: 'Tìm',
    placeholder: 'Tìm',
    noResults: '—',
    recent: 'Gần đây',
    related: 'Liên quan',
  };

  it('related commands are the other commands of the best match group', () => {
    const e = flattenRibbon(tabs);
    expect(relatedCommands(e, [e[0]!]).map((x) => x.id)).toEqual([
      'format.italic',
      'format.underline',
    ]);
    expect(relatedCommands(e, [])).toEqual([]);
  });

  it('shows recent commands before typing and related ones after the matches', () => {
    localStorage.clear();
    pushRecentCommand('format.underline');
    render(<CommandSearch tabs={tabs} open onClose={vi.fn()} labels={labels} />);
    const recent = document.querySelectorAll('[data-section="recent"][data-command-result]');
    expect([...recent].map((r) => r.getAttribute('data-command-result'))).toEqual([
      'format.underline',
    ]);
    fireEvent.change(screen.getByTestId('command-search-input'), { target: { value: 'đậm' } });
    const related = document.querySelectorAll('[data-section="related"][data-command-result]');
    expect(related.length).toBe(2);
    fireEvent.keyDown(screen.getByTestId('command-search-input'), { key: 'Enter' });
    expect(JSON.parse(localStorage.getItem('bo.commandSearch.recent') ?? '[]')[0]).toBe(
      'format.bold',
    );
  });
});
