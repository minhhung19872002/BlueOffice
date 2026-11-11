import { useTranslation } from '@blueoffice/i18n';
import type {
  ChartTemplate,
  SlideChartData,
  SlideChartKind,
  SlideLegend,
} from '@blueoffice/platform';
import { ChartTemplateList, Dialog, deleteChartTemplate, loadChartTemplates } from '@blueoffice/ui';
import { useState } from 'react';
import styles from './ChartDialog.module.css';

export const CHART_KINDS: SlideChartKind[] = [
  'column',
  'bar',
  'line',
  'area',
  'pie',
  'doughnut',
  'stackedColumn',
  'percentColumn',
  'stackedBar',
  'percentBar',
  'stackedLine',
  'percentLine',
  'stackedArea',
  'percentArea',
  'explodedPie',
  'pieOfPie',
  'barOfPie',
  'radar',
  'radarMarker',
  'radarFilled',
  'stock',
  'stockUpDown',
  'stockVolume',
  'stockVolumeUpDown',
  'surface',
  'column3D',
  'clusteredColumn3D',
  'stackedColumn3D',
  'percentColumn3D',
  'cylinder3D',
  'cone3D',
  'pyramid3D',
  'bar3D',
  'stackedBar3D',
  'percentBar3D',
  'line3D',
  'area3D',
  'stackedArea3D',
  'percentArea3D',
  'pie3D',
  'explodedPie3D',
  'surface3D',
  'waterfall',
  'treemap',
  'sunburst',
  'histogram',
  'pareto',
  'boxWhisker',
  'funnel',
];
const LEGENDS: SlideLegend[] = ['none', 'r', 'b', 't', 'l'];

export interface ChartDraft {
  kind: SlideChartKind;
  title: string;
  legend: SlideLegend;
  data: SlideChartData;
  /** Saved chart template to format the chart with (CORE-CHART-007). */
  template?: ChartTemplate | null;
}

/** Grid cells as text (numbers in the UI locale are parsed back by {@link parseValue}). */
interface Grid {
  categories: string[];
  names: string[];
  /** `[row][series]`. */
  cells: string[][];
}

/** Number typed in the grid (`1.234,5` and `1234.5` both work); empty = no point. */
export function parseValue(s: string): number | null {
  const t = s.trim().replace(/\s/g, '');
  if (!t) return null;
  const normalized = /,\d*$/.test(t) ? t.replace(/\./g, '').replace(',', '.') : t.replace(/,/g, '');
  const n = Number(normalized);
  return Number.isFinite(n) ? n : null;
}

function toGrid(d: SlideChartData): Grid {
  return {
    categories: [...d.categories],
    names: d.series.map((s) => s.name),
    cells: d.categories.map((_, r) =>
      d.series.map((s) => {
        const v = s.values[r];
        return v === null || v === undefined ? '' : String(v);
      }),
    ),
  };
}

export function fromGrid(g: Grid): SlideChartData {
  return {
    categories: g.categories,
    series: g.names.map((name, j) => ({
      name,
      values: g.categories.map((_, r) => parseValue(g.cells[r]?.[j] ?? '')),
    })),
  };
}

export interface ChartDialogProps {
  open: boolean;
  /** Insert a new chart or edit the selected one. */
  mode: 'insert' | 'edit';
  initial: ChartDraft;
  onClose: () => void;
  onApply: (draft: ChartDraft) => void;
}

/**
 * Chart type, title, legend and the data grid (categories down, series
 * across). The engine writes the chart part and its embedded workbook.
 * Mounted for each use (fields start from `initial`).
 */
export function ChartDialog({ open, mode, initial, onClose, onApply }: ChartDialogProps) {
  const { t } = useTranslation();
  const [kind, setKind] = useState(initial.kind);
  const [title, setTitle] = useState(initial.title);
  const [legend, setLegend] = useState(initial.legend);
  const [grid, setGrid] = useState<Grid>(() => toGrid(initial.data));
  const [templates, setTemplates] = useState(() => loadChartTemplates<ChartTemplate>());
  const [template, setTemplate] = useState<string | null>(null);

  const update = (f: (g: Grid) => void) =>
    setGrid((g) => {
      const n: Grid = {
        categories: [...g.categories],
        names: [...g.names],
        cells: g.cells.map((r) => [...r]),
      };
      f(n);
      return n;
    });

  const addRow = () =>
    update((g) => {
      g.categories.push(`${g.categories.length + 1}`);
      g.cells.push(g.names.map(() => ''));
    });
  const addSeries = () =>
    update((g) => {
      g.names.push(t('slideEditor.chartSeries', { n: g.names.length + 1 }));
      for (const r of g.cells) r.push('');
    });
  const removeRow = () =>
    update((g) => {
      if (g.categories.length <= 1) return;
      g.categories.pop();
      g.cells.pop();
    });
  const removeSeries = () =>
    update((g) => {
      if (g.names.length <= 1) return;
      g.names.pop();
      for (const r of g.cells) r.pop();
    });

  return (
    <Dialog
      open={open}
      onOpenChange={(o) => !o && onClose()}
      title={mode === 'insert' ? t('slideEditor.insertChart') : t('slideEditor.editChart')}
      width={640}
      testId="chart-dialog"
      footer={
        <>
          <button type="button" onClick={onClose}>
            {t('slideEditor.cancel')}
          </button>
          <button
            type="button"
            data-testid="chart-ok"
            onClick={() =>
              onApply({
                kind,
                title,
                legend,
                data: fromGrid(grid),
                template: templates.find((x) => x.name === template) ?? null,
              })
            }
          >
            {t('slideEditor.ok')}
          </button>
        </>
      }
    >
      <div className={styles.chartForm}>
        <label>
          {t('slideEditor.chartType')}
          <select
            data-testid="chart-kind"
            value={kind}
            onChange={(e) => setKind(e.target.value as SlideChartKind)}
          >
            {CHART_KINDS.map((k) => (
              <option key={k} value={k}>
                {t(`slideEditor.chartKinds.${k}`)}
              </option>
            ))}
          </select>
        </label>
        <label>
          {t('slideEditor.chartTitle')}
          <input
            data-testid="chart-title"
            value={title}
            onChange={(e) => setTitle(e.target.value)}
          />
        </label>
        <ChartTemplateList
          templates={templates}
          value={template}
          onChange={setTemplate}
          onDelete={(name) => {
            setTemplates(deleteChartTemplate<ChartTemplate>(name));
            if (template === name) setTemplate(null);
          }}
          labels={{
            title: t('chartDesign.templates'),
            none: t('chartDesign.noTemplates'),
            noTemplate: t('chartDesign.noTemplate'),
            remove: t('chartDesign.removeTemplate'),
          }}
        />
        <label>
          {t('slideEditor.legend')}
          <select
            data-testid="chart-legend"
            value={legend}
            onChange={(e) => setLegend(e.target.value as SlideLegend)}
          >
            {LEGENDS.map((l) => (
              <option key={l} value={l}>
                {t(`slideEditor.legends.${l}`)}
              </option>
            ))}
          </select>
        </label>
      </div>
      <div className={styles.chartGridWrap}>
        <table className={styles.chartGrid} data-testid="chart-grid">
          <thead>
            <tr>
              <th>{t('slideEditor.chartCategory')}</th>
              {grid.names.map((n, j) => (
                <th key={j}>
                  <input
                    aria-label={t('slideEditor.chartSeriesName', { n: j + 1 })}
                    data-testid={`chart-name-${j}`}
                    value={n}
                    onChange={(e) => update((g) => (g.names[j] = e.target.value))}
                  />
                </th>
              ))}
            </tr>
          </thead>
          <tbody>
            {grid.categories.map((c, r) => (
              <tr key={r}>
                <th>
                  <input
                    aria-label={t('slideEditor.chartCategoryN', { n: r + 1 })}
                    data-testid={`chart-cat-${r}`}
                    value={c}
                    onChange={(e) => update((g) => (g.categories[r] = e.target.value))}
                  />
                </th>
                {grid.names.map((_, j) => (
                  <td key={j}>
                    <input
                      inputMode="decimal"
                      aria-label={t('slideEditor.chartValue', { row: r + 1, col: j + 1 })}
                      data-testid={`chart-cell-${r}-${j}`}
                      value={grid.cells[r]?.[j] ?? ''}
                      onChange={(e) => update((g) => (g.cells[r]![j] = e.target.value))}
                    />
                  </td>
                ))}
              </tr>
            ))}
          </tbody>
        </table>
      </div>
      <div className={styles.chartGridButtons}>
        <button type="button" data-testid="chart-add-row" onClick={addRow}>
          {t('slideEditor.chartAddRow')}
        </button>
        <button type="button" onClick={removeRow}>
          {t('slideEditor.chartRemoveRow')}
        </button>
        <button type="button" data-testid="chart-add-series" onClick={addSeries}>
          {t('slideEditor.chartAddSeries')}
        </button>
        <button type="button" onClick={removeSeries}>
          {t('slideEditor.chartRemoveSeries')}
        </button>
      </div>
    </Dialog>
  );
}
