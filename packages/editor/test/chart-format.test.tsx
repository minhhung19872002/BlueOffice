// @vitest-environment happy-dom
import { createI18n, I18nextProvider } from '@blueoffice/i18n';
import type {
  ChartAxisInfo,
  ChartElementsInfo,
  ChartElementsPatch,
  ChartSeriesInfo,
} from '@blueoffice/platform';
import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { CommandRegistry } from '../src/commands';
import {
  ChartFormatPane,
  chartElementIds,
  chartElementName,
  parseAmounts,
  parseNumber,
} from '../src/chart/ChartFormatPane';
import {
  CHART_ELEMENT_ACTIONS,
  CHART_FORMAT_COMMAND,
  ChartElementsMenu,
  chartElementLabelKey,
  chartElementsGroup,
  registerChartElementCommands,
  type ChartCommandSpec,
} from '../src/chart/chartElements';
import { docChartTabs, registerDocChartCommands, type ChartState } from '../src/chart/docCharts';

// CORE-CHART-005: the shared Format chart pane and the Add chart element menu only translate
// what the user does into `elements` patches of the host's chart design command.

afterEach(cleanup);

const i18n = createI18n('en-US');
const t = i18n.t.bind(i18n);

const axis = (kind: ChartAxisInfo['kind'], over: Partial<ChartAxisInfo> = {}): ChartAxisInfo => ({
  kind,
  title: null,
  min: null,
  max: null,
  majorUnit: null,
  minorUnit: null,
  logBase: null,
  reverse: false,
  crosses: null,
  crossesAt: null,
  dispUnit: null,
  dispUnitLabel: false,
  numFmt: null,
  majorTick: null,
  minorTick: null,
  labelPos: null,
  majorGrid: kind === 'value',
  minorGrid: false,
  deleted: false,
  baseUnit: null,
  ...over,
});

const series = (index: number, over: Partial<ChartSeriesInfo> = {}): ChartSeriesInfo => ({
  index,
  plot: 0,
  kind: 'bar',
  name: index === 0 ? 'North' : 'South',
  points: 3,
  categories: ['Q1', 'Q2', 'Q3'],
  secondary: false,
  fill: null,
  line: null,
  lineWidth: null,
  marker: null,
  smooth: false,
  explosion: 0,
  labels: null,
  pointLabels: [],
  trendlines: [],
  errorBars: null,
  canTrend: true,
  canSecondary: true,
  ...over,
});

function column(over: Partial<ChartElementsInfo> = {}): ChartElementsInfo {
  return {
    has: {
      axes: true,
      scatter: false,
      horizontal: false,
      pie: false,
      secondary: false,
      dataTable: true,
      bars: true,
      lines: false,
      dropLines: false,
      upDownBars: false,
    },
    title: { text: 'Sales', overlay: false },
    legend: { pos: 'b', overlay: false },
    chartArea: { fill: null, line: null, rounded: false },
    plotArea: { fill: null },
    gridlines: { color: null, width: null },
    axes: { cat: axis('category'), val: axis('value'), val2: null },
    dataTable: { show: false, keys: true },
    groups: [
      {
        plot: 0,
        kind: 'bar',
        threeD: false,
        stacked: false,
        secondary: false,
        gapWidth: 150,
        overlap: 0,
        dropLines: false,
        hiLowLines: false,
        upDownBars: false,
      },
    ],
    series: [series(0), series(1)],
    labels: null,
    ...over,
  };
}

function pie(): ChartElementsInfo {
  const base = column();
  return {
    ...base,
    has: { ...base.has, axes: false, pie: true, bars: false, dataTable: false },
    axes: null,
    groups: [{ ...base.groups[0]!, kind: 'pie' }],
    series: [series(0, { kind: 'pie', canTrend: false, canSecondary: false })],
  };
}

function setup(info: ChartElementsInfo | null | undefined, element?: string) {
  const sent: ChartElementsPatch[] = [];
  const apply = vi.fn((p: ChartElementsPatch) => void sent.push(p));
  const view = render(
    <I18nextProvider i18n={i18n}>
      <ChartFormatPane info={info} apply={apply} element={element} />
    </I18nextProvider>,
  );
  const pick = (id: string) =>
    fireEvent.change(screen.getByTestId('chart-format-element'), { target: { value: id } });
  const type = (testId: string, text: string) => {
    const input = screen.getByTestId(testId);
    fireEvent.change(input, { target: { value: text } });
    fireEvent.keyDown(input, { key: 'Enter' });
  };
  return { sent, pick, type, view };
}

describe('typed numbers', () => {
  it('reads decimal commas and rejects text', () => {
    expect(parseNumber('1,5')).toBe(1.5);
    expect(parseNumber(' -20 ')).toBe(-20);
    expect(parseNumber('1e3')).toBe(1000);
    expect(parseNumber('abc')).toBeNull();
    expect(parseNumber('')).toBeNull();
  });

  it('takes custom error amounts as numbers or as a range', () => {
    expect(parseAmounts('1; 2,5; 3')).toEqual({ ref: null, values: [1, 2.5, 3] });
    expect(parseAmounts('=Sheet1!$D$2:$D$7')).toEqual({ ref: '=Sheet1!$D$2:$D$7', values: [] });
    expect(parseAmounts('  ')).toEqual({ ref: null, values: [] });
  });
});

describe('Format chart pane', () => {
  it('says so when no chart is selected', () => {
    setup(undefined);
    expect(screen.getByTestId('chart-format-pane').textContent).toContain('Select a chart');
    expect(screen.queryByTestId('chart-format-element')).toBeNull();
    cleanup();
    // A chart without element options (chartex).
    setup(null);
    expect(screen.getByTestId('chart-format-pane').textContent).toContain('no element options');
  });

  it('offers the elements the chart has', () => {
    expect(chartElementIds(column())).toEqual([
      'chartArea',
      'plotArea',
      'title',
      'legend',
      'axis.cat',
      'axis.val',
      'gridlines',
      'series.0',
      'series.1',
      'labels',
      'dataTable',
      'trendline',
      'errorBars',
    ]);
    // A pie has no axes, gridlines, data table, trend lines or error bars.
    expect(chartElementIds(pie())).toEqual(['chartArea', 'title', 'legend', 'series.0', 'labels']);
    const secondary = column({
      axes: { cat: axis('category'), val: axis('value'), val2: axis('value') },
    });
    expect(chartElementIds(secondary)).toContain('axis.val2');
    expect(chartElementName('series.1', column(), t)).toBe('Series "South"');
    setup(column());
    const options = [...screen.getByTestId('chart-format-element').querySelectorAll('option')];
    expect(options.map((o) => o.textContent)).toContain('Value axis');
  });

  it('sets a logarithmic scale and a fixed maximum, and returns them to automatic', () => {
    const { sent, pick, type } = setup(column());
    pick('axis.val');
    fireEvent.click(screen.getByTestId('cf-axis-log'));
    type('cf-axis-max', '1000');
    type('cf-axis-major', '2,5');
    expect(sent).toEqual([
      { axes: { val: { logBase: 10 } } },
      { axes: { val: { max: 1000 } } },
      { axes: { val: { majorUnit: 2.5 } } },
    ]);
    cleanup();
    const fixed = column({
      axes: {
        cat: axis('category'),
        val: axis('value', { logBase: 10, max: 1000, crossesAt: 5 }),
        val2: null,
      },
    });
    const again = setup(fixed, 'axis.val');
    expect((screen.getByTestId('cf-axis-max') as HTMLInputElement).value).toBe('1000');
    expect((screen.getByTestId('cf-axis-log') as HTMLInputElement).checked).toBe(true);
    again.type('cf-axis-max', '');
    again.type('cf-axis-log-base', '2');
    fireEvent.click(screen.getByTestId('cf-axis-log'));
    fireEvent.change(screen.getByTestId('cf-axis-crosses'), { target: { value: 'auto' } });
    // Not a number: nothing is sent.
    again.type('cf-axis-min', 'ten');
    expect(again.sent).toEqual([
      { axes: { val: { max: null } } },
      { axes: { val: { logBase: 2 } } },
      { axes: { val: { logBase: null } } },
      { axes: { val: { crosses: null, crossesAt: null } } },
    ]);
  });

  it('edits the other axis options', () => {
    const { sent, pick, type } = setup(column());
    pick('axis.cat');
    // A category axis has no scale.
    expect(screen.queryByTestId('cf-axis-min')).toBeNull();
    fireEvent.click(screen.getByTestId('cf-axis-reverse'));
    fireEvent.change(screen.getByTestId('cf-axis-kind'), { target: { value: 'date' } });
    type('cf-axis-title', 'Quarter');
    fireEvent.change(screen.getByTestId('cf-axis-major-tick'), { target: { value: 'cross' } });
    fireEvent.change(screen.getByTestId('cf-axis-label-pos'), { target: { value: 'high' } });
    pick('axis.val');
    fireEvent.change(screen.getByTestId('cf-axis-disp-unit'), { target: { value: 'thousands' } });
    type('cf-axis-num-fmt', '#,##0');
    fireEvent.change(screen.getByTestId('cf-axis-crosses'), { target: { value: 'max' } });
    fireEvent.click(screen.getByTestId('cf-axis-minor-grid'));
    fireEvent.click(screen.getByTestId('cf-axis-show'));
    expect(sent).toEqual([
      { axes: { cat: { reverse: true } } },
      { axes: { cat: { kind: 'date' } } },
      { axes: { cat: { title: 'Quarter' } } },
      { axes: { cat: { majorTick: 'cross' } } },
      { axes: { cat: { labelPos: 'high' } } },
      { axes: { val: { dispUnit: 'thousands' } } },
      { axes: { val: { numFmt: '#,##0' } } },
      { axes: { val: { crosses: 'max' } } },
      { axes: { val: { minorGrid: true } } },
      { axes: { val: { deleted: true } } },
    ]);
  });

  it('sets data labels for the chart, a series and a point', () => {
    const { sent, pick } = setup(column());
    pick('labels');
    fireEvent.click(screen.getByTestId('cf-labels-on'));
    // A column chart has no percentages.
    expect(screen.queryByTestId('cf-labels-percent')).toBeNull();
    fireEvent.change(screen.getByTestId('cf-series-pick'), { target: { value: '1' } });
    fireEvent.click(screen.getByTestId('cf-labels-category'));
    fireEvent.change(screen.getByTestId('cf-labels-point'), { target: { value: '2' } });
    fireEvent.click(screen.getByTestId('cf-labels-value'));
    expect(sent).toEqual([
      { labels: { value: true } },
      { series: [{ index: 1, labels: { category: true } }] },
      { series: [{ index: 1, pointLabels: [{ point: 2, labels: { value: true } }] }] },
    ]);
  });

  it('shows percentages and leader lines on a pie', () => {
    const { sent, pick } = setup(pie());
    pick('labels');
    // Turning the labels of a pie on shows percentages.
    fireEvent.click(screen.getByTestId('cf-labels-on'));
    fireEvent.click(screen.getByTestId('cf-labels-category'));
    expect(sent).toEqual([{ labels: { percent: true } }, { labels: { category: true } }]);
    cleanup();
    const labelled = pie();
    labelled.labels = { percent: true, pos: 'outEnd' };
    const again = setup(labelled, 'labels');
    expect((screen.getByTestId('cf-labels-percent') as HTMLInputElement).checked).toBe(true);
    const own = { percent: true, pos: 'outEnd' };
    fireEvent.change(screen.getByTestId('cf-labels-pos'), { target: { value: 'inEnd' } });
    fireEvent.click(screen.getByTestId('cf-labels-leader'));
    again.type('cf-labels-separator', ' | ');
    again.type('cf-labels-num-fmt', '0.0%');
    fireEvent.click(screen.getByTestId('cf-labels-on'));
    expect(again.sent).toEqual([
      { labels: { ...own, pos: 'inEnd' } },
      { labels: { ...own, leaderLines: false } },
      { labels: { ...own, separator: ' | ' } },
      { labels: { ...own, numFmt: '0.0%' } },
      { labels: null },
    ]);
  });

  it('adds, edits and removes trend lines of a series', () => {
    const { sent, pick } = setup(column());
    pick('trendline');
    fireEvent.click(screen.getByTestId('cf-trend-add'));
    expect(sent).toEqual([{ series: [{ index: 0, trendlines: [{ kind: 'linear' }] }] }]);
    cleanup();
    const info = column();
    info.series[1]!.trendlines = [{ kind: 'linear' }, { kind: 'exp', forward: 1 }];
    const again = setup(info, 'trendline');
    fireEvent.change(screen.getByTestId('cf-series-pick'), { target: { value: '1' } });
    expect(screen.getByTestId('cf-trend-list').querySelectorAll('li')).toHaveLength(2);
    fireEvent.change(screen.getByTestId('cf-trend-kind'), { target: { value: 'poly' } });
    fireEvent.click(screen.getByTestId('cf-trend-eq'));
    fireEvent.click(screen.getByTestId('cf-trend-r2'));
    again.type('cf-trend-forward', '2');
    again.type('cf-trend-name', 'Trend');
    fireEvent.click(screen.getByTestId('cf-trend-intercept-on'));
    fireEvent.click(screen.getByTestId('cf-trend-remove-1'));
    const rest = { kind: 'exp', forward: 1 };
    expect(again.sent).toEqual([
      { series: [{ index: 1, trendlines: [{ kind: 'poly', order: 2 }, rest] }] },
      { series: [{ index: 1, trendlines: [{ kind: 'linear', dispEq: true }, rest] }] },
      { series: [{ index: 1, trendlines: [{ kind: 'linear', dispR2: true }, rest] }] },
      { series: [{ index: 1, trendlines: [{ kind: 'linear', forward: 2 }, rest] }] },
      { series: [{ index: 1, trendlines: [{ kind: 'linear', name: 'Trend' }, rest] }] },
      { series: [{ index: 1, trendlines: [{ kind: 'linear', intercept: 0 }, rest] }] },
      { series: [{ index: 1, trendlines: [{ kind: 'linear' }] }] },
    ]);
    cleanup();
    // The order of a polynomial stays within 2–6; a moving average has a period and no equation.
    const poly = column();
    poly.series[0]!.trendlines = [{ kind: 'poly', order: 2 }];
    const third = setup(poly, 'trendline');
    third.type('cf-trend-order', '9');
    fireEvent.change(screen.getByTestId('cf-trend-kind'), { target: { value: 'movingAvg' } });
    expect(third.sent).toEqual([
      { series: [{ index: 0, trendlines: [{ kind: 'poly', order: 6 }] }] },
      { series: [{ index: 0, trendlines: [{ kind: 'movingAvg', order: 2 }] }] },
    ]);
    cleanup();
    const avg = column();
    avg.series[0]!.trendlines = [{ kind: 'movingAvg', order: 3 }];
    setup(avg, 'trendline');
    expect(screen.queryByTestId('cf-trend-eq')).toBeNull();
    expect((screen.getByTestId('cf-trend-period') as HTMLInputElement).value).toBe('3');
  });

  it('sets error bars: direction, end style, amount and custom ranges', () => {
    const { sent, pick } = setup(column());
    pick('errorBars');
    fireEvent.click(screen.getByTestId('cf-err-on'));
    expect(sent).toEqual([
      { series: [{ index: 0, errorBars: { kind: 'both', valueType: 'fixedVal', value: 1 } }] },
    ]);
    cleanup();
    const info = column();
    info.series[0]!.errorBars = { kind: 'both', valueType: 'fixedVal', value: 1 };
    const again = setup(info, 'errorBars');
    const base = { kind: 'both', valueType: 'fixedVal', value: 1 };
    fireEvent.change(screen.getByTestId('cf-err-kind'), { target: { value: 'plus' } });
    fireEvent.change(screen.getByTestId('cf-err-cap'), { target: { value: 'none' } });
    fireEvent.change(screen.getByTestId('cf-err-type'), { target: { value: 'percentage' } });
    again.type('cf-err-value', '2,5');
    fireEvent.change(screen.getByTestId('cf-err-type'), { target: { value: 'cust' } });
    fireEvent.click(screen.getByTestId('cf-err-on'));
    expect(again.sent).toEqual([
      { series: [{ index: 0, errorBars: { ...base, kind: 'plus' } }] },
      { series: [{ index: 0, errorBars: { ...base, noEndCap: true } }] },
      { series: [{ index: 0, errorBars: { ...base, valueType: 'percentage', value: 5 } }] },
      { series: [{ index: 0, errorBars: { ...base, value: 2.5 } }] },
      {
        series: [
          {
            index: 0,
            errorBars: {
              ...base,
              valueType: 'cust',
              plus: [1],
              minus: [1],
              plusRef: null,
              minusRef: null,
            },
          },
        ],
      },
      { series: [{ index: 0, errorBars: null }] },
    ]);
    cleanup();
    const custom = column();
    custom.series[0]!.errorBars = { kind: 'both', valueType: 'cust', plus: [1, 2], minus: [] };
    const third = setup(custom, 'errorBars');
    expect((screen.getByTestId('cf-err-plus') as HTMLInputElement).value).toBe('1; 2');
    third.type('cf-err-minus', '=Sheet1!$E$2:$E$4');
    third.type('cf-err-plus', '0,5; 1');
    const cust = { kind: 'both', valueType: 'cust', plus: [1, 2], minus: [] };
    expect(third.sent).toEqual([
      { series: [{ index: 0, errorBars: { ...cust, minusRef: '=Sheet1!$E$2:$E$4', minus: [] } }] },
      { series: [{ index: 0, errorBars: { ...cust, plusRef: null, plus: [0.5, 1] } }] },
    ]);
  });

  it('formats a series and its bar group', () => {
    const info = column();
    info.series[1] = series(1, {
      kind: 'line',
      plot: 1,
      lineWidth: 2.25,
      marker: { symbol: 'none' },
    });
    info.groups.push({ ...info.groups[0]!, plot: 1, kind: 'line' });
    const { sent, pick, type } = setup(info);
    pick('series.0');
    type('cf-series-gap-width', '60');
    type('cf-series-overlap', '-20');
    fireEvent.click(screen.getByTestId('cf-series-secondary'));
    fireEvent.change(screen.getByTestId('cf-series-fill'), { target: { value: '#c0504d' } });
    pick('series.1');
    type('cf-series-line-width', '3');
    fireEvent.change(screen.getByTestId('cf-series-marker'), { target: { value: 'diamond' } });
    fireEvent.click(screen.getByTestId('cf-series-smooth'));
    expect(sent).toEqual([
      { groups: [{ plot: 0, gapWidth: 60 }] },
      { groups: [{ plot: 0, overlap: -20 }] },
      { series: [{ index: 0, secondary: true }] },
      { series: [{ index: 0, fill: '#C0504D' }] },
      { series: [{ index: 1, lineWidth: 3 }] },
      { series: [{ index: 1, marker: { symbol: 'diamond', size: 5 } }] },
      { series: [{ index: 1, smooth: true }] },
    ]);
    // A series' shortcuts open its trend lines.
    fireEvent.click(screen.getByTestId('cf-series-trendline'));
    expect((screen.getByTestId('chart-format-element') as HTMLSelectElement).value).toBe(
      'trendline',
    );
    expect((screen.getByTestId('cf-series-pick') as HTMLSelectElement).value).toBe('1');
  });

  it('covers titles, legend, areas, gridlines, the data table and lines', () => {
    const info = column();
    info.has.dropLines = true;
    info.has.lines = true;
    info.has.upDownBars = true;
    info.dataTable.show = true;
    const { sent, pick, type } = setup(info);
    fireEvent.click(screen.getByTestId('cf-area-rounded'));
    fireEvent.change(screen.getByTestId('cf-area-fill'), { target: { value: '#f4f6f8' } });
    pick('title');
    type('cf-title-text', 'Revenue');
    fireEvent.click(screen.getByTestId('cf-title-overlay'));
    pick('legend');
    fireEvent.change(screen.getByTestId('cf-legend-pos'), { target: { value: 't' } });
    fireEvent.click(screen.getByTestId('cf-legend-overlay'));
    pick('gridlines');
    fireEvent.click(screen.getByTestId('cf-grid-val-minor'));
    type('cf-grid-width', '0,5');
    pick('dataTable');
    fireEvent.click(screen.getByTestId('cf-table-keys'));
    pick('lines');
    fireEvent.click(screen.getByTestId('cf-lines-drop'));
    fireEvent.click(screen.getByTestId('cf-lines-hilow'));
    fireEvent.click(screen.getByTestId('cf-lines-updown'));
    expect(sent).toEqual([
      { chartArea: { rounded: true } },
      { chartArea: { fill: '#F4F6F8' } },
      { title: { text: 'Revenue' } },
      { title: { overlay: true } },
      { legend: { pos: 't' } },
      { legend: { overlay: true } },
      { axes: { val: { minorGrid: true } } },
      { gridlines: { width: 0.5 } },
      { dataTable: { keys: false } },
      { groups: [{ dropLines: true }] },
      { groups: [{ hiLowLines: true }] },
      { groups: [{ upDownBars: true }] },
    ]);
  });
});

describe('Add chart element', () => {
  const action = (id: string) => CHART_ELEMENT_ACTIONS.find((a) => a.id === id)!;

  it('has a label for every choice and element in both languages', () => {
    for (const lang of ['vi-VN', 'en-US'] as const) {
      const i = createI18n(lang);
      const groups = new Set<string>();
      for (const a of CHART_ELEMENT_ACTIONS) {
        expect(i.exists(chartElementLabelKey(a.id)), `${lang} ${a.id}`).toBe(true);
        groups.add(a.group);
      }
      for (const g of groups) expect(i.exists(`chartFormat.el.${g}.label`), g).toBe(true);
      const secondary = column({
        axes: { cat: axis('category'), val: axis('value'), val2: axis('value') },
      });
      secondary.has.dropLines = true;
      for (const id of chartElementIds(secondary)) {
        const key = id.startsWith('series.')
          ? 'chartFormat.element.series'
          : `chartFormat.element.${{ 'axis.cat': 'axisCat', 'axis.val': 'axisVal', 'axis.val2': 'axisVal2' }[id] ?? id}`;
        expect(i.exists(key), `${lang} ${key}`).toBe(true);
      }
    }
  });

  it('turns each choice into one patch', () => {
    const info = column();
    expect(action('chart.el.axes.val').patch(info, t)).toEqual({
      axes: { val: { deleted: true } },
    });
    expect(action('chart.el.axisTitles.cat').patch(info, t)).toEqual({
      axes: { cat: { title: t('chartDesign.xTitleText') } },
    });
    // On horizontal bars the value axis runs across.
    const bars = { ...info, has: { ...info.has, horizontal: true } };
    expect(action('chart.el.axisTitles.val').patch(bars, t)).toEqual({
      axes: { val: { title: t('chartDesign.xTitleText') } },
    });
    expect(action('chart.el.title.overlay').patch(info, t)).toEqual({
      title: { text: 'Sales', overlay: true },
    });
    expect(
      action('chart.el.title.above').patch({ ...info, title: { text: '', overlay: false } }, t),
    ).toEqual({
      title: { text: t('chartDesign.titleText'), overlay: false },
    });
    expect(action('chart.el.labels.outEnd').patch(info, t)).toEqual({
      labels: { value: true, pos: 'outEnd' },
    });
    expect(action('chart.el.labels.show').patch(pie(), t)).toEqual({
      labels: { percent: true, pos: null },
    });
    expect(action('chart.el.table.noKeys').patch(info, t)).toEqual({
      dataTable: { show: true, keys: false },
    });
    expect(action('chart.el.errorBars.percentage').patch(info, t)).toEqual({
      series: [0, 1].map((index) => ({
        index,
        errorBars: { kind: 'both', valueType: 'percentage', value: 5 },
      })),
    });
    expect(action('chart.el.gridlines.valMajor').patch(info, t)).toEqual({
      axes: { val: { majorGrid: false } },
    });
    expect(action('chart.el.legend.none').patch(info, t)).toEqual({ legend: { pos: 'none' } });
    expect(action('chart.el.trendline.forecast').patch(info, t)).toEqual({
      series: [0, 1].map((index) => ({ index, trendlines: [{ kind: 'linear', forward: 2 }] })),
    });
    expect(action('chart.el.upDownBars.toggle').patch(info, t)).toEqual({
      groups: [{ upDownBars: true }],
    });
    // What a chart cannot take is disabled; the current state is checked.
    expect(action('chart.el.trendline.linear').enabled(pie())).toBe(false);
    expect(action('chart.el.axes.cat').enabled(pie())).toBe(false);
    expect(action('chart.el.table.keys').enabled(pie())).toBe(false);
    expect(action('chart.el.lines.drop').enabled(info)).toBe(false);
    expect(action('chart.el.legend.b').checked(info)).toBe(true);
    expect(action('chart.el.title.above').checked(info)).toBe(true);
    expect(action('chart.el.labels.none').checked(info)).toBe(true);
    expect(action('chart.el.trendline.none').checked(info)).toBe(true);
    expect(action('chart.el.gridlines.valMajor').checked(info)).toBe(true);
  });

  it('registers the choices as commands of the host and runs them through its channel', () => {
    interface State {
      info: ChartElementsInfo | null;
    }
    const state: State = { info: column() };
    const sent: ChartElementsPatch[] = [];
    const commands = new Map<string, ChartCommandSpec<State>>();
    let open = false;
    registerChartElementCommands<State>((c) => commands.set(c.id, c), {
      t,
      info: (s) => s.info,
      apply: (p) => void sent.push(p),
      openPane: () => {
        open = !open;
      },
    });
    expect(commands.size).toBe(CHART_ELEMENT_ACTIONS.length + 1);
    commands.get('chart.el.legend.r')!.run(state);
    commands.get(CHART_FORMAT_COMMAND)!.run(state);
    // Not available for this chart: nothing is sent.
    commands.get('chart.el.lines.drop')!.run(state);
    expect(sent).toEqual([{ legend: { pos: 'r' } }]);
    expect(open).toBe(true);
    expect(commands.get('chart.el.lines.drop')!.isEnabled(state)).toBe(false);
    expect(commands.get('chart.el.legend.b')!.isChecked?.(state)).toBe(true);
    // Without a chart nothing is enabled.
    expect(commands.get('chart.el.legend.r')!.isEnabled({ info: null })).toBe(false);
    expect(commands.get(CHART_FORMAT_COMMAND)!.isEnabled({ info: null })).toBe(false);
  });

  it('shows the menu with the current state checked and runs the picked command', () => {
    const run = vi.fn();
    render(
      <div className="bo-root">
        <ChartElementsMenu t={t} info={column()} run={run} />
      </div>,
    );
    fireEvent.click(screen.getByTestId('chart-add-element'));
    const menu = screen.getByTestId('chart-add-element-menu');
    const item = (id: string) => menu.querySelector(`[data-command="${id}"]`) as HTMLButtonElement;
    expect(item('chart.el.legend.b').getAttribute('aria-checked')).toBe('true');
    expect(item('chart.el.legend.r').getAttribute('aria-checked')).toBe('false');
    expect(item('chart.el.lines.drop').disabled).toBe(true);
    expect(item('chart.el.trendline.linear').textContent).toContain('Linear');
    fireEvent.click(item('chart.el.trendline.linear'));
    expect(run).toHaveBeenCalledWith('chart.el.trendline.linear');
    expect(screen.queryByTestId('chart-add-element-menu')).toBeNull();
    const group = chartElementsGroup({ t, info: column(), run, enabled: true, paneOpen: true });
    expect(group.items.map((i) => i.id)).toEqual(['chart.addElement', CHART_FORMAT_COMMAND]);
    expect(group.items[1]!.checked).toBe(true);
  });

  it('BlueDoc sends the choices as setChart commands with an elements patch', async () => {
    const info = column();
    const sent: unknown[] = [];
    const r = new CommandRegistry<ChartState>();
    let open = false;
    registerDocChartCommands(r, {
      t,
      run: async (c) => {
        sent.push(c);
        return null;
      },
      open: () => {},
      pane: { toggle: () => (open = !open) },
    });
    const state = {
      doc: {},
      edit: { format: { chart: true }, chart: { elements: info } },
    } as unknown as ChartState;
    expect(r.isEnabled('chart.el.legend.t', state)).toBe(true);
    expect(r.isEnabled('chart.el.legend.t', { doc: {} })).toBe(false);
    expect(r.isChecked('chart.el.legend.b', state)).toBe(true);
    await r.execute('chart.el.legend.t', state);
    await r.execute(CHART_FORMAT_COMMAND, state);
    expect(sent).toEqual([{ cmd: 'setChart', elements: { legend: { pos: 't' } } }]);
    expect(open).toBe(true);
    const tabs = docChartTabs({
      t,
      item: (id, labelKey) => ({ id, label: t(labelKey), onSelect: () => {} }),
      icons: { edit: null, types: {}, wrap: null },
      shown: true,
      load: async () => null,
      apply: () => {},
      onSaveTemplate: () => {},
      elements: { info, run: () => {}, paneOpen: false },
    });
    expect(tabs[0]!.groups.map((g) => g.id)).toContain('chartElements');
  });
});
