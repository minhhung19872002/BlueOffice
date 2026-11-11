import { createI18n } from '@blueoffice/i18n';
import { describe, expect, it } from 'vitest';
import { CHART_TYPES } from '../src/dialogs';

// CORE-CHART-002: volume-high-low-close and volume-open-high-low-close stock charts.
describe('volume stock chart types', () => {
  it('are offered after the other stock charts, named in both languages', () => {
    const at = CHART_TYPES.indexOf('stockUpDown');
    expect(CHART_TYPES.slice(at + 1, at + 3)).toEqual(['stockVolume', 'stockVolumeUpDown']);
    const vi = createI18n('vi-VN');
    const en = createI18n('en-US');
    expect(vi.t('sheetEditor.chart.types.stockVolume')).toBe(
      'Chứng khoán: khối lượng–cao–thấp–đóng',
    );
    expect(en.t('sheetEditor.chart.types.stockVolumeUpDown')).toBe(
      'Stock: volume–open–high–low–close',
    );
  });
});
