/**
 * Region map options of the chart dialog (CORE-CHART-006): projection, area level, region labels and
 * the colour scale of a `regionMap` chart. Changes go through the engine command `setChartMap`.
 */
import { useTranslation } from '@blueoffice/i18n';
import type { ChartMapInfo, ChartMapOptions } from '@blueoffice/platform';
import styles from './SheetEditor.module.css';

export const MAP_PROJECTIONS = ['', 'mercator', 'miller', 'robinson', 'albers'] as const;
export const MAP_LEVELS = ['', 'countryRegion', 'world', 'state', 'dataOnly'] as const;
export const MAP_LABELS = ['none', 'bestFitOnly', 'showAll'] as const;

/** The editable options of a listed map. */
export function mapOptionsOf(info: ChartMapInfo | null | undefined): ChartMapOptions {
  return {
    projection: info?.projection ?? '',
    level: info?.level ?? '',
    labels: info?.labels ?? 'none',
    scale: info?.scale ?? 'sequential',
    minColor: info?.minColor ?? '',
    midColor: info?.midColor ?? '',
    maxColor: info?.maxColor ?? '',
  };
}

/** `RRGGBB` for a colour input (BlueSheet's default scale colours when unset). */
function hexOr(v: string, fallback: string): string {
  return `#${/^[0-9a-f]{6}$/i.test(v) ? v : fallback}`;
}

export function MapChartOptions({
  value,
  info,
  onChange,
}: {
  value: ChartMapOptions;
  info: ChartMapInfo | null | undefined;
  onChange: (v: ChartMapOptions) => void;
}) {
  const { t } = useTranslation();
  const set = <K extends keyof ChartMapOptions>(k: K, v: ChartMapOptions[K]) =>
    onChange({ ...value, [k]: v });
  const diverging = value.scale === 'diverging';
  const needsDataset = info?.status === 'needsDataset';
  return (
    <fieldset className={styles.form} data-testid="map-options">
      <legend>{t('sheetEditor.chart.map.title')}</legend>
      {needsDataset && (
        <p className={styles.hint} role="status" data-testid="map-needs-dataset">
          {t('sheetEditor.chart.map.needsDataset')}
        </p>
      )}
      <label>
        {t('sheetEditor.chart.map.projection')}
        <select
          data-testid="map-projection"
          value={value.projection}
          onChange={(e) => set('projection', e.target.value)}
        >
          {MAP_PROJECTIONS.map((p) => (
            <option key={p} value={p}>
              {t(`sheetEditor.chart.map.projections.${p || 'auto'}`)}
            </option>
          ))}
        </select>
      </label>
      <label>
        {t('sheetEditor.chart.map.level')}
        <select
          data-testid="map-level"
          value={value.level}
          onChange={(e) => set('level', e.target.value)}
        >
          {MAP_LEVELS.map((l) => (
            <option key={l} value={l}>
              {t(`sheetEditor.chart.map.levels.${l || 'auto'}`)}
            </option>
          ))}
        </select>
      </label>
      <label>
        {t('sheetEditor.chart.map.labels')}
        <select
          data-testid="map-labels"
          value={value.labels}
          onChange={(e) => set('labels', e.target.value)}
        >
          {MAP_LABELS.map((l) => (
            <option key={l} value={l}>
              {t(`sheetEditor.chart.map.labelModes.${l}`)}
            </option>
          ))}
        </select>
      </label>
      {info?.categorical ? (
        <p className={styles.hint}>{t('sheetEditor.chart.map.categorical')}</p>
      ) : (
        <>
          <label>
            {t('sheetEditor.chart.map.scale')}
            <select
              data-testid="map-scale"
              value={value.scale}
              onChange={(e) => set('scale', e.target.value as ChartMapOptions['scale'])}
            >
              {(['sequential', 'diverging'] as const).map((s) => (
                <option key={s} value={s}>
                  {t(`sheetEditor.chart.map.scales.${s}`)}
                </option>
              ))}
            </select>
          </label>
          <label>
            {t('sheetEditor.chart.map.minColor')}
            <input
              type="color"
              data-testid="map-min-color"
              value={hexOr(value.minColor, diverging ? 'C0504D' : 'DCE9F5')}
              onChange={(e) => set('minColor', e.target.value.slice(1).toUpperCase())}
            />
          </label>
          {diverging && (
            <label>
              {t('sheetEditor.chart.map.midColor')}
              <input
                type="color"
                data-testid="map-mid-color"
                value={hexOr(value.midColor, 'F5F1E6')}
                onChange={(e) => set('midColor', e.target.value.slice(1).toUpperCase())}
              />
            </label>
          )}
          <label>
            {t('sheetEditor.chart.map.maxColor')}
            <input
              type="color"
              data-testid="map-max-color"
              value={hexOr(value.maxColor, diverging ? '2E6DB4' : '1F4E89')}
              onChange={(e) => set('maxColor', e.target.value.slice(1).toUpperCase())}
            />
          </label>
        </>
      )}
    </fieldset>
  );
}
