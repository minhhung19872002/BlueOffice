// @vitest-environment happy-dom
import { createI18n, I18nextProvider } from '@blueoffice/i18n';
import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import type { ChartMapInfo, ChartMapOptions, SheetCommand } from '@blueoffice/platform';
import { MapChartOptions, mapOptionsOf } from '../src/mapChart';
import { registerMapDatasets } from '../src/mapDatasets';
import { CHART_TYPES } from '../src/dialogs';

afterEach(cleanup);

const i18n = createI18n('vi-VN');

const info = (over: Partial<ChartMapInfo> = {}): ChartMapInfo => ({
  projection: null,
  level: 'countryRegion',
  labels: 'bestFitOnly',
  scale: 'sequential',
  categorical: false,
  minColor: null,
  midColor: null,
  maxColor: null,
  status: 'ok',
  ...over,
});

function show(i: ChartMapInfo, onChange: (v: ChartMapOptions) => void) {
  render(
    <I18nextProvider i18n={i18n}>
      <MapChartOptions value={mapOptionsOf(i)} info={i} onChange={onChange} />
    </I18nextProvider>,
  );
}

describe('map chart options (CORE-CHART-006)', () => {
  it('is offered as a chart type', () => {
    expect(CHART_TYPES).toContain('regionMap');
    expect(i18n.t('sheetEditor.chart.types.regionMap')).toBe('Biểu đồ bản đồ');
  });

  it('edits projection, level, labels and the colour scale', () => {
    const onChange = vi.fn();
    show(info(), onChange);
    fireEvent.change(screen.getByTestId('map-projection'), { target: { value: 'robinson' } });
    expect(onChange).toHaveBeenLastCalledWith(expect.objectContaining({ projection: 'robinson' }));
    fireEvent.change(screen.getByTestId('map-level'), { target: { value: 'world' } });
    expect(onChange).toHaveBeenLastCalledWith(expect.objectContaining({ level: 'world' }));
    fireEvent.change(screen.getByTestId('map-labels'), { target: { value: 'showAll' } });
    expect(onChange).toHaveBeenLastCalledWith(expect.objectContaining({ labels: 'showAll' }));
    fireEvent.change(screen.getByTestId('map-scale'), { target: { value: 'diverging' } });
    expect(onChange).toHaveBeenLastCalledWith(expect.objectContaining({ scale: 'diverging' }));
    fireEvent.change(screen.getByTestId('map-max-color'), { target: { value: '#123456' } });
    expect(onChange).toHaveBeenLastCalledWith(expect.objectContaining({ maxColor: '123456' }));
    expect(screen.queryByTestId('map-mid-color')).toBeNull();
    expect(screen.queryByTestId('map-needs-dataset')).toBeNull();
  });

  it('says when a province map needs its dataset; categories have no scale', () => {
    show(info({ level: 'state', status: 'needsDataset', categorical: true }), vi.fn());
    expect(screen.getByTestId('map-needs-dataset').textContent).toBe(
      'Bản đồ cấp tỉnh cần bộ dữ liệu ranh giới tỉnh/thành được phê duyệt (assets/maps/vn-provinces).',
    );
    expect(screen.queryByTestId('map-scale')).toBeNull();
  });

  it('registers the datasets a source has, once per engine', async () => {
    const sent: SheetCommand[] = [];
    const host = {
      exec: vi.fn(async (c: SheetCommand) => {
        sent.push(c);
        return { id: 'vn-provinces' } as never;
      }),
    };
    const source = {
      load: vi.fn(async (id: string) =>
        id === 'vn-provinces' ? { manifest: '{}', boundaries: '{}' } : null,
      ),
    };
    expect(await registerMapDatasets(host, source, ['vn-provinces', 'other'])).toEqual([
      'vn-provinces',
    ]);
    await registerMapDatasets(host, source, ['vn-provinces', 'other']);
    expect(sent).toEqual([{ cmd: 'registerMapDataset', manifest: '{}', boundaries: '{}' }]);
    expect(await registerMapDatasets({ exec: vi.fn() }, undefined)).toEqual([]);
  });
});
