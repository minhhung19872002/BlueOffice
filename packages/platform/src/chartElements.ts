/**
 * Chart element options (CORE-CHART-005): what the shared "Format chart" pane of BlueSheet,
 * BlueSlide and BlueDoc reads (`ChartElementsInfo`, the `elements` field of each app's chart
 * info) and changes (`ChartElementsPatch`, the `elements` field of the chart design command).
 * Mirrors `bo_engine::chart_elements`. In a patch an absent field keeps its value and `null`
 * returns it to automatic. Colours are `#RRGGBB` or a theme colour name (`accent1` …).
 */

/** Data labels (`c:dLbls` / `c:dLbl`): what they show and where. */
export interface ChartDataLabels {
  value?: boolean;
  percent?: boolean;
  category?: boolean;
  series?: boolean;
  /** `outEnd`, `inEnd`, `inBase`, `ctr`, `bestFit`, `t`, `b`, `l`, `r`; `null` = default. */
  pos?: string | null;
  /** Leader lines of pie labels (default on). */
  leaderLines?: boolean | null;
  /** Number format code of the values. */
  numFmt?: string | null;
  /** Text between the parts of a label (default `"; "`). */
  separator?: string | null;
}

/** Marker of a line / scatter series. */
export interface ChartMarker {
  /** `circle`, `dash`, `diamond`, `dot`, `none`, `plus`, `square`, `star`, `triangle`, `x`. */
  symbol: string;
  /** Points (2–72). */
  size?: number | null;
}

export type ChartTrendlineKind = 'linear' | 'exp' | 'log' | 'poly' | 'power' | 'movingAvg';

/** A trend line (`c:trendline`). */
export interface ChartTrendline {
  kind: ChartTrendlineKind;
  /** Polynomial order (2–6) or moving-average period. */
  order?: number;
  dispEq?: boolean;
  dispR2?: boolean;
  name?: string | null;
  /** Units the line runs past the last / before the first point. */
  forward?: number | null;
  backward?: number | null;
  /** Value at x = 0 (linear, polynomial, exponential). */
  intercept?: number | null;
}

/** Error bars (`c:errBars`). */
export interface ChartErrorBars {
  /** `both`, `plus`, `minus`. */
  kind: string;
  /** `fixedVal`, `percentage`, `stdDev`, `stdErr`, `cust`. */
  valueType: string;
  value?: number;
  noEndCap?: boolean;
  /** `x` on a scatter series; `null` = the value direction. */
  dir?: string | null;
  /** Custom amounts (literal numbers) and / or the ranges holding them. */
  plus?: (number | null)[];
  minus?: (number | null)[];
  plusRef?: string | null;
  minusRef?: string | null;
}

/** Options of one axis as read. */
export interface ChartAxisInfo {
  /** `category`, `date` or `value`. */
  kind: 'category' | 'date' | 'value';
  title: string | null;
  min: number | null;
  max: number | null;
  majorUnit: number | null;
  minorUnit: number | null;
  logBase: number | null;
  reverse: boolean;
  /** `min` / `max`; `null` = automatic. */
  crosses: string | null;
  crossesAt: number | null;
  dispUnit: string | null;
  dispUnitLabel: boolean;
  numFmt: string | null;
  /** `in`, `cross`, `none`; `null` = automatic (outside). */
  majorTick: string | null;
  /** `in`, `out`, `cross`; `null` = none. */
  minorTick: string | null;
  /** `high`, `low`, `none`; `null` = next to the axis. */
  labelPos: string | null;
  majorGrid: boolean;
  minorGrid: boolean;
  deleted: boolean;
  /** `months` / `years` of a date axis; `null` = days. */
  baseUnit: string | null;
}

export interface ChartSeriesInfo {
  /** Zero-based index across the chart's groups. */
  index: number;
  plot: number;
  /** `bar`, `line`, `area`, `pie`, `doughnut`, `scatter`, `bubble`, `radar`, `stock`, … */
  kind: string;
  name: string;
  points: number;
  categories: string[];
  secondary: boolean;
  fill: string | null;
  line: string | null;
  /** Points; `null` for series without a line. */
  lineWidth: number | null;
  marker: ChartMarker | null;
  smooth: boolean;
  explosion: number;
  labels: ChartDataLabels | null;
  pointLabels: { point: number; labels: ChartDataLabels }[];
  trendlines: ChartTrendline[];
  errorBars: ChartErrorBars | null;
  canTrend: boolean;
  canSecondary: boolean;
}

export interface ChartGroupInfo {
  plot: number;
  kind: string;
  threeD: boolean;
  stacked: boolean;
  secondary: boolean;
  gapWidth: number;
  overlap: number;
  dropLines: boolean;
  hiLowLines: boolean;
  upDownBars: boolean;
}

/** The elements of a chart and their options. */
export interface ChartElementsInfo {
  has: {
    axes: boolean;
    scatter: boolean;
    horizontal: boolean;
    pie: boolean;
    secondary: boolean;
    dataTable: boolean;
    bars: boolean;
    lines: boolean;
    dropLines: boolean;
    upDownBars: boolean;
  };
  title: { text: string; overlay: boolean };
  legend: { pos: string; overlay: boolean };
  chartArea: { fill: string | null; line: string | null; rounded: boolean };
  plotArea: { fill: string | null };
  gridlines: { color: string | null; width: number | null };
  axes: { cat: ChartAxisInfo; val: ChartAxisInfo; val2: ChartAxisInfo | null } | null;
  dataTable: { show: boolean; keys: boolean };
  groups: ChartGroupInfo[];
  series: ChartSeriesInfo[];
  /** Data labels of the whole chart. */
  labels: ChartDataLabels | null;
}

/** A change to one axis. */
export type ChartAxisPatch = {
  min?: number | null;
  max?: number | null;
  majorUnit?: number | null;
  minorUnit?: number | null;
  logBase?: number | null;
  reverse?: boolean;
  crosses?: string | null;
  crossesAt?: number | null;
  dispUnit?: string | null;
  dispUnitLabel?: boolean;
  numFmt?: string | null;
  majorTick?: string | null;
  minorTick?: string | null;
  labelPos?: string | null;
  majorGrid?: boolean;
  minorGrid?: boolean;
  deleted?: boolean;
  baseUnit?: string | null;
  title?: string | null;
  /** Category axis only. */
  kind?: 'category' | 'date';
};

export type ChartAxisId = 'cat' | 'val' | 'val2';

/** A change to one series. */
export type ChartSeriesPatch = {
  index: number;
  fill?: string | null;
  line?: string | null;
  lineWidth?: number | null;
  marker?: ChartMarker | null;
  smooth?: boolean;
  explosion?: number;
  secondary?: boolean;
  labels?: ChartDataLabels | null;
  pointLabels?: { point: number; labels: ChartDataLabels | null }[];
  /** The whole list of the series' trend lines. */
  trendlines?: ChartTrendline[];
  errorBars?: ChartErrorBars | null;
};

/** A change to a chart group; without `plot`, to every group the option applies to. */
export type ChartGroupPatch = {
  plot?: number;
  gapWidth?: number | null;
  overlap?: number | null;
  dropLines?: boolean;
  hiLowLines?: boolean;
  upDownBars?: boolean;
};

/**
 * A change to the elements of a chart. A type alias, not an interface: command unions carry it
 * and must stay assignable to `{ [k: string]: unknown }`.
 */
export type ChartElementsPatch = {
  title?: { text?: string; overlay?: boolean };
  legend?: { pos?: string; overlay?: boolean };
  chartArea?: { fill?: string | null; line?: string | null; rounded?: boolean };
  plotArea?: { fill?: string | null };
  axes?: Partial<Record<ChartAxisId, ChartAxisPatch>>;
  gridlines?: { color?: string | null; width?: number | null };
  dataTable?: { show?: boolean; keys?: boolean };
  groups?: ChartGroupPatch[];
  series?: ChartSeriesPatch[];
  labels?: ChartDataLabels | null;
};
