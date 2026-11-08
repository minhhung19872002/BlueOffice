// @vitest-environment happy-dom
import { act, cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { planRibbonLayout, Ribbon, type GroupWidths, type RibbonTab } from '../src';

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

const g = (full: number, compact: number, collapsed: number): GroupWidths => ({
  full,
  compact,
  collapsed,
});

describe('planRibbonLayout', () => {
  const groups = [g(200, 120, 60), g(300, 150, 60), g(100, 100, 60)];

  it('keeps every group full when it fits', () => {
    expect(planRibbonLayout(groups, 600)).toEqual({
      levels: ['full', 'full', 'full'],
      overflowFrom: 3,
    });
  });

  it('hides labels from the right first, skipping groups it would not narrow', () => {
    // 600 needed; 550 available: the last group cannot get narrower by hiding
    // labels (100 → 100), so the middle one goes compact (300 → 150).
    expect(planRibbonLayout(groups, 550).levels).toEqual(['full', 'compact', 'full']);
    // 430 available: both narrowable groups go compact (120 + 150 + 100 = 370).
    expect(planRibbonLayout(groups, 430).levels).toEqual(['compact', 'compact', 'full']);
  });

  it('collapses whole groups into dropdowns once labels are all hidden', () => {
    // compact everywhere = 370; collapsing the last (100 → 60) gives 330.
    expect(planRibbonLayout(groups, 340).levels).toEqual(['compact', 'compact', 'collapsed']);
    // 120 + 60 + 60 = 240.
    expect(planRibbonLayout(groups, 250).levels).toEqual(['compact', 'collapsed', 'collapsed']);
    expect(planRibbonLayout(groups, 180)).toEqual({
      levels: ['collapsed', 'collapsed', 'collapsed'],
      overflowFrom: 3,
    });
  });

  it('moves trailing groups into the overflow menu as a last resort', () => {
    // All collapsed = 180 > 150: drop groups until shown + 48 (more button) fits.
    expect(planRibbonLayout(groups, 150).overflowFrom).toBe(1);
    expect(planRibbonLayout(groups, 100).overflowFrom).toBe(0);
  });

  it('never produces a total wider than the available width', () => {
    const many = Array.from({ length: 14 }, (_, i) => g(80 + i * 13, 50 + i * 5, 56));
    // From the width of the "more" button upwards (narrower cannot show anything).
    for (let w = 48; w <= 2000; w += 37) {
      const plan = planRibbonLayout(many, w);
      const shown = plan.levels
        .slice(0, plan.overflowFrom)
        .reduce((sum, level, i) => sum + (many[i]?.[level] ?? 0), 0);
      const more = plan.overflowFrom < many.length ? 48 : 0;
      expect(shown + more).toBeLessThanOrEqual(w);
    }
  });
});

// happy-dom has no layout: give elements widths that depend on their collapse level.
function fakeLayout(panelWidth: number) {
  const widths: Record<string, number> = { full: 300, compact: 200, collapsed: 70 };
  vi.spyOn(HTMLElement.prototype, 'getBoundingClientRect').mockImplementation(function (
    this: HTMLElement,
  ) {
    const w = widths[this.dataset.level ?? ''] ?? 0;
    return { width: w, height: 80, x: 0, y: 0, top: 0, left: 0, right: w, bottom: 80 } as DOMRect;
  });
  vi.spyOn(HTMLElement.prototype, 'clientWidth', 'get').mockImplementation(function (
    this: HTMLElement,
  ) {
    return this.getAttribute('role') === 'tabpanel' ? panelWidth : 0;
  });
}

function tabs(onBold: () => void): RibbonTab[] {
  const group = (id: string): RibbonTab['groups'][number] => ({
    id,
    label: `Nhóm ${id}`,
    items: [
      {
        id: `${id}.a`,
        label: `Lệnh ${id}`,
        icon: <svg />,
        onSelect: id === 'g3' ? onBold : () => {},
      },
      { id: `${id}.b`, label: `Phụ ${id}`, icon: <svg />, size: 'small', onSelect: () => {} },
    ],
  });
  return [{ id: 'home', label: 'Trang đầu', groups: [group('g1'), group('g2'), group('g3')] }];
}

describe('Ribbon collapse', () => {
  it('collapses the rightmost groups into dropdowns that still run their commands', async () => {
    // 3 × 300 = 900 full; 550 available → all compact (600, still too wide) → g3
    // collapsed (200 + 200 + 70 = 470).
    fakeLayout(550);
    const bold = vi.fn();
    await act(async () => {
      render(<Ribbon tabs={tabs(bold)} moreLabel="Thêm lệnh" />);
    });
    const level = (id: string) =>
      screen.getByRole('group', { name: `Nhóm ${id}` }).getAttribute('data-level');
    expect([level('g1'), level('g2'), level('g3')]).toEqual(['compact', 'compact', 'collapsed']);
    expect(screen.queryByTestId('ribbon-more')).toBeNull();
    // The collapsed group is one button; its commands appear in its dropdown.
    expect(screen.queryByText('Lệnh g3')).toBeNull();
    fireEvent.click(screen.getByTestId('ribbon-group-g3'));
    fireEvent.click(screen.getByText('Lệnh g3'));
    expect(bold).toHaveBeenCalledTimes(1);
    // Choosing a command closes the dropdown.
    expect(screen.queryByText('Lệnh g3')).toBeNull();
  });

  it('puts what still does not fit into the overflow menu', async () => {
    fakeLayout(150);
    await act(async () => {
      render(<Ribbon tabs={tabs(() => {})} moreLabel="Thêm lệnh" />);
    });
    expect(screen.getByRole('group', { name: 'Nhóm g1' }).getAttribute('data-level')).toBe(
      'collapsed',
    );
    expect(screen.queryByRole('group', { name: 'Nhóm g3' })).toBeNull();
    const more = screen.getByTestId('ribbon-more');
    expect(more.getAttribute('aria-label')).toBe('Thêm lệnh');
    fireEvent.click(more);
    expect(screen.getByText('Lệnh g3')).toBeTruthy();
    expect(screen.getByText('Lệnh g2')).toBeTruthy();
  });

  it('keeps everything expanded when the window is wide', async () => {
    fakeLayout(2000);
    await act(async () => {
      render(<Ribbon tabs={tabs(() => {})} />);
    });
    for (const id of ['g1', 'g2', 'g3']) {
      expect(screen.getByRole('group', { name: `Nhóm ${id}` }).getAttribute('data-level')).toBe(
        'full',
      );
    }
    expect(screen.getByText('Lệnh g3')).toBeTruthy();
  });
});
