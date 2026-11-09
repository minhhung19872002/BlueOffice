// @vitest-environment happy-dom
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import {
  CHART_TEMPLATES_KEY,
  ChartColorsMenu,
  ChartLayoutMenu,
  ChartStyleGallery,
  ChartTemplateList,
  deleteChartTemplate,
  loadChartTemplates,
  saveChartTemplate,
  type ChartDesignGalleryData,
  type ChartDesignLabels,
} from '../src';

afterEach(cleanup);
beforeEach(() => localStorage.clear());

const svg = '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 10 10"></svg>';
const gallery: ChartDesignGalleryData = {
  styles: ['clean', 'night'].map((id) => ({ id, svg })),
  colors: [
    { id: 'colorful', colors: ['#112233', '#445566'] },
    { id: 'lotus', colors: ['#B23A6F'] },
  ],
  layouts: ['layout1', 'layout2'].map((id) => ({ id, svg })),
};
const labels: ChartDesignLabels = {
  styles: 'Kiểu biểu đồ',
  colors: 'Đổi màu',
  layouts: 'Bố cục nhanh',
  loading: 'Đang tải',
  styleName: (id) => `style ${id}`,
  colorName: (id) => `màu ${id}`,
  layoutName: (id) => `bố cục ${id}`,
};

describe('Chart design controls', () => {
  it('style gallery loads previews when opened and picks one', async () => {
    const load = vi.fn(async () => gallery);
    const pick = vi.fn();
    render(<ChartStyleGallery load={load} onPick={pick} labels={labels} />);
    expect(load).not.toHaveBeenCalled();
    fireEvent.click(screen.getByTestId('chart-styles'));
    const night = await screen.findByTestId('chart-styles-grid-night');
    expect(load).toHaveBeenCalledTimes(1);
    expect(night.getAttribute('aria-label')).toBe('style night');
    expect(night.querySelector('img')?.getAttribute('src')).toMatch(/^data:image\/svg\+xml/);
    fireEvent.click(night);
    expect(pick).toHaveBeenCalledWith('night');
    // The panel closes after a pick.
    await waitFor(() => expect(screen.queryByTestId('chart-styles-grid')).toBeNull());
  });

  it('colour and layout menus', async () => {
    const load = vi.fn(async () => gallery);
    const colors = vi.fn();
    const layout = vi.fn();
    render(
      <>
        <ChartColorsMenu load={load} onPick={colors} labels={labels} />
        <ChartLayoutMenu load={load} onPick={layout} labels={labels} />
      </>,
    );
    fireEvent.click(screen.getByTestId('chart-colors'));
    const lotus = await screen.findByTestId('chart-colors-lotus');
    expect(lotus.textContent).toContain('màu lotus');
    expect(screen.getByTestId('chart-colors-colorful').querySelectorAll('span span')).toHaveLength(
      2,
    );
    fireEvent.click(lotus);
    expect(colors).toHaveBeenCalledWith('lotus');
    fireEvent.click(screen.getByTestId('chart-layouts'));
    fireEvent.click(await screen.findByTestId('chart-layouts-grid-layout2'));
    expect(layout).toHaveBeenCalledWith('layout2');
  });

  it('a disabled dropdown does not load', () => {
    const load = vi.fn(async () => gallery);
    render(<ChartStyleGallery load={load} onPick={() => {}} labels={labels} disabled />);
    fireEvent.click(screen.getByTestId('chart-styles'));
    expect(load).not.toHaveBeenCalled();
  });

  it('templates are saved, replaced, listed and deleted', () => {
    expect(loadChartTemplates()).toEqual([]);
    saveChartTemplate({ name: 'Báo cáo', kind: 'column' });
    saveChartTemplate({ name: 'An', kind: 'line' });
    saveChartTemplate({ name: 'Báo cáo', kind: 'bar' });
    const list = loadChartTemplates<{ name: string; kind: string }>();
    expect(list.map((t) => [t.name, t.kind])).toEqual([
      ['An', 'line'],
      ['Báo cáo', 'bar'],
    ]);
    expect(deleteChartTemplate('An')).toHaveLength(1);
    localStorage.setItem(CHART_TEMPLATES_KEY, '{not json');
    expect(loadChartTemplates()).toEqual([]);
  });

  it('template list picks and deletes', () => {
    const change = vi.fn();
    const del = vi.fn();
    const lbl = { title: 'Mẫu', none: 'Chưa có', noTemplate: 'Không', remove: 'Xoá' };
    const { rerender } = render(
      <ChartTemplateList templates={[]} value={null} onChange={change} labels={lbl} />,
    );
    expect(screen.getByText('Chưa có')).toBeTruthy();
    rerender(
      <ChartTemplateList
        templates={[{ name: 'A' }, { name: 'B' }]}
        value={null}
        onChange={change}
        onDelete={del}
        labels={lbl}
      />,
    );
    fireEvent.click(screen.getByTestId('chart-template-B'));
    expect(change).toHaveBeenCalledWith('B');
    fireEvent.click(screen.getByRole('button', { name: 'Xoá: A' }));
    expect(del).toHaveBeenCalledWith('A');
  });
});
