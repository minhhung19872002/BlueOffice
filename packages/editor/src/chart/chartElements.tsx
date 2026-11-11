/**
 * "Add chart element" (CORE-CHART-005): the quick choices of the Chart Design tab shared by
 * BlueSheet, BlueSlide and BlueDoc — axes, axis titles, chart title, data labels, data table,
 * error bars, gridlines, legend, lines, trend line, up/down bars. Each choice is a command
 * (`chart.el.*`) that sends one `elements` patch through the host's chart design command (one
 * undo step); "More options…" opens the Format chart pane (`chart.format`).
 */
import type { useTranslation } from '@blueoffice/i18n';
import type {
  ChartDataLabels,
  ChartElementsInfo,
  ChartElementsPatch,
  ChartErrorBars,
  ChartTrendline,
} from '@blueoffice/platform';
import { Popover, type RibbonGroup } from '@blueoffice/ui';
import { ListPlus, SlidersHorizontal } from 'lucide-react';
import styles from './ChartFormat.module.css';

type T = ReturnType<typeof useTranslation>['t'];
type Info = ChartElementsInfo | null | undefined;

/** Command id of the Format chart pane. */
export const CHART_FORMAT_COMMAND = 'chart.format';

/** One quick choice of the "Add chart element" menu. */
export interface ChartElementAction {
  /** Command id (`chart.el.<group>.<choice>`). */
  id: string;
  /** Menu section (`axes`, `title`, `labels`, …). */
  group: string;
  /** The change it makes to the chart with elements `info`; `null` = nothing to do. */
  patch: (info: ChartElementsInfo, t: T) => ChartElementsPatch | null;
  /** The chart can take it. */
  enabled: (info: ChartElementsInfo) => boolean;
  /** The chart is in that state now. */
  checked: (info: ChartElementsInfo) => boolean;
}

const always = () => true;
const hasAxes = (i: ChartElementsInfo) => !!i.axes;
const trendable = (i: ChartElementsInfo) => i.series.filter((s) => s.canTrend);

/** The same labels on the whole chart. */
function labels(pos: string | null): ChartElementAction['patch'] {
  return (i) => {
    const d: ChartDataLabels = i.has.pie ? { percent: true } : { value: true };
    return { labels: { ...d, pos } };
  };
}

const labelsAt = (pos: string | null) => (i: ChartElementsInfo) => {
  const d = i.labels;
  return !!d && !!(d.value || d.percent || d.category || d.series) && (d.pos ?? null) === pos;
};

function errorBars(e: ChartErrorBars | null): ChartElementAction['patch'] {
  return (i) => ({ series: trendable(i).map((s) => ({ index: s.index, errorBars: e })) });
}

const errorBarsAre = (type: string | null) => (i: ChartElementsInfo) => {
  const s = trendable(i);
  return s.length > 0 && s.every((x) => (x.errorBars?.valueType ?? null) === type);
};

function trendline(list: ChartTrendline[]): ChartElementAction['patch'] {
  return (i) => ({ series: trendable(i).map((s) => ({ index: s.index, trendlines: list })) });
}

const trendlineIs = (kind: string | null, forward: boolean) => (i: ChartElementsInfo) => {
  const s = trendable(i);
  if (s.length === 0) return false;
  return s.every((x) => {
    const tl = x.trendlines[0];
    if (kind === null) return x.trendlines.length === 0;
    return !!tl && tl.kind === kind && !!tl.forward === forward;
  });
};

function axisToggle(id: 'cat' | 'val'): ChartElementAction {
  return {
    id: `chart.el.axes.${id}`,
    group: 'axes',
    patch: (i) => (i.axes ? { axes: { [id]: { deleted: !i.axes[id].deleted } } } : null),
    enabled: hasAxes,
    checked: (i) => !!i.axes && !i.axes[id].deleted,
  };
}

function axisTitleToggle(id: 'cat' | 'val'): ChartElementAction {
  return {
    id: `chart.el.axisTitles.${id}`,
    group: 'axisTitles',
    patch: (i, t) => {
      if (!i.axes) return null;
      // The category axis runs across unless the bars are horizontal.
      const across = (id === 'cat') !== i.has.horizontal;
      const text = t(across ? 'chartDesign.xTitleText' : 'chartDesign.yTitleText');
      return { axes: { [id]: { title: i.axes[id].title ? null : text } } };
    },
    enabled: hasAxes,
    checked: (i) => !!i.axes?.[id].title,
  };
}

function gridToggle(id: 'cat' | 'val', minor: boolean): ChartElementAction {
  const key = minor ? 'minorGrid' : 'majorGrid';
  return {
    id: `chart.el.gridlines.${id}${minor ? 'Minor' : 'Major'}`,
    group: 'gridlines',
    patch: (i) => (i.axes ? { axes: { [id]: { [key]: !i.axes[id][key] } } } : null),
    enabled: hasAxes,
    checked: (i) => !!i.axes?.[id][key],
  };
}

function legend(pos: string): ChartElementAction {
  return {
    id: `chart.el.legend.${pos}`,
    group: 'legend',
    patch: () => ({ legend: { pos } }),
    enabled: always,
    checked: (i) => i.legend.pos === pos,
  };
}

/** Every quick choice, in menu order. */
export const CHART_ELEMENT_ACTIONS: ChartElementAction[] = [
  axisToggle('cat'),
  axisToggle('val'),
  axisTitleToggle('cat'),
  axisTitleToggle('val'),
  {
    id: 'chart.el.title.none',
    group: 'title',
    patch: () => ({ title: { text: '' } }),
    enabled: always,
    checked: (i) => !i.title.text,
  },
  {
    id: 'chart.el.title.above',
    group: 'title',
    patch: (i, t) => ({
      title: { text: i.title.text || t('chartDesign.titleText'), overlay: false },
    }),
    enabled: always,
    checked: (i) => !!i.title.text && !i.title.overlay,
  },
  {
    id: 'chart.el.title.overlay',
    group: 'title',
    patch: (i, t) => ({
      title: { text: i.title.text || t('chartDesign.titleText'), overlay: true },
    }),
    enabled: always,
    checked: (i) => !!i.title.text && i.title.overlay,
  },
  {
    id: 'chart.el.labels.none',
    group: 'labels',
    patch: () => ({ labels: null }),
    enabled: always,
    checked: (i) => !i.labels && i.series.every((s) => !s.labels && s.pointLabels.length === 0),
  },
  {
    id: 'chart.el.labels.show',
    group: 'labels',
    patch: labels(null),
    enabled: always,
    checked: labelsAt(null),
  },
  {
    id: 'chart.el.labels.ctr',
    group: 'labels',
    patch: labels('ctr'),
    enabled: always,
    checked: labelsAt('ctr'),
  },
  {
    id: 'chart.el.labels.inEnd',
    group: 'labels',
    patch: labels('inEnd'),
    enabled: (i) => i.has.bars || i.has.pie,
    checked: labelsAt('inEnd'),
  },
  {
    id: 'chart.el.labels.outEnd',
    group: 'labels',
    patch: labels('outEnd'),
    enabled: (i) => i.has.bars || i.has.pie,
    checked: labelsAt('outEnd'),
  },
  {
    id: 'chart.el.table.none',
    group: 'table',
    patch: () => ({ dataTable: { show: false } }),
    enabled: (i) => i.has.dataTable,
    checked: (i) => !i.dataTable.show,
  },
  {
    id: 'chart.el.table.keys',
    group: 'table',
    patch: () => ({ dataTable: { show: true, keys: true } }),
    enabled: (i) => i.has.dataTable,
    checked: (i) => i.dataTable.show && i.dataTable.keys,
  },
  {
    id: 'chart.el.table.noKeys',
    group: 'table',
    patch: () => ({ dataTable: { show: true, keys: false } }),
    enabled: (i) => i.has.dataTable,
    checked: (i) => i.dataTable.show && !i.dataTable.keys,
  },
  {
    id: 'chart.el.errorBars.none',
    group: 'errorBars',
    patch: errorBars(null),
    enabled: (i) => trendable(i).length > 0,
    checked: errorBarsAre(null),
  },
  {
    id: 'chart.el.errorBars.stdErr',
    group: 'errorBars',
    patch: errorBars({ kind: 'both', valueType: 'stdErr', value: 1 }),
    enabled: (i) => trendable(i).length > 0,
    checked: errorBarsAre('stdErr'),
  },
  {
    id: 'chart.el.errorBars.percentage',
    group: 'errorBars',
    patch: errorBars({ kind: 'both', valueType: 'percentage', value: 5 }),
    enabled: (i) => trendable(i).length > 0,
    checked: errorBarsAre('percentage'),
  },
  {
    id: 'chart.el.errorBars.stdDev',
    group: 'errorBars',
    patch: errorBars({ kind: 'both', valueType: 'stdDev', value: 1 }),
    enabled: (i) => trendable(i).length > 0,
    checked: errorBarsAre('stdDev'),
  },
  gridToggle('val', false),
  gridToggle('val', true),
  gridToggle('cat', false),
  gridToggle('cat', true),
  legend('none'),
  legend('r'),
  legend('t'),
  legend('l'),
  legend('b'),
  {
    id: 'chart.el.lines.none',
    group: 'lines',
    patch: (i) => ({
      groups: [{ dropLines: false, ...(i.has.lines ? { hiLowLines: false } : {}) }],
    }),
    enabled: (i) => i.has.dropLines,
    checked: (i) => i.groups.every((g) => !g.dropLines && (!g.hiLowLines || g.kind === 'stock')),
  },
  {
    id: 'chart.el.lines.drop',
    group: 'lines',
    patch: () => ({ groups: [{ dropLines: true }] }),
    enabled: (i) => i.has.dropLines,
    checked: (i) => i.groups.some((g) => g.dropLines),
  },
  {
    id: 'chart.el.lines.hiLow',
    group: 'lines',
    patch: () => ({ groups: [{ hiLowLines: true }] }),
    enabled: (i) => i.has.lines,
    checked: (i) => i.groups.some((g) => g.hiLowLines && g.kind === 'line'),
  },
  {
    id: 'chart.el.trendline.none',
    group: 'trendline',
    patch: trendline([]),
    enabled: (i) => trendable(i).length > 0,
    checked: trendlineIs(null, false),
  },
  {
    id: 'chart.el.trendline.linear',
    group: 'trendline',
    patch: trendline([{ kind: 'linear' }]),
    enabled: (i) => trendable(i).length > 0,
    checked: trendlineIs('linear', false),
  },
  {
    id: 'chart.el.trendline.exp',
    group: 'trendline',
    patch: trendline([{ kind: 'exp' }]),
    enabled: (i) => trendable(i).length > 0,
    checked: trendlineIs('exp', false),
  },
  {
    id: 'chart.el.trendline.forecast',
    group: 'trendline',
    patch: trendline([{ kind: 'linear', forward: 2 }]),
    enabled: (i) => trendable(i).length > 0,
    checked: trendlineIs('linear', true),
  },
  {
    id: 'chart.el.trendline.movingAvg',
    group: 'trendline',
    patch: trendline([{ kind: 'movingAvg', order: 2 }]),
    enabled: (i) => trendable(i).length > 0,
    checked: trendlineIs('movingAvg', false),
  },
  {
    id: 'chart.el.upDownBars.toggle',
    group: 'upDownBars',
    patch: (i) => ({ groups: [{ upDownBars: !i.groups.some((g) => g.upDownBars) }] }),
    enabled: (i) => i.has.upDownBars,
    checked: (i) => i.groups.some((g) => g.upDownBars),
  },
];

/** i18n key of an action's label (`chartFormat.el.<group>.<choice>`). */
export function chartElementLabelKey(id: string): string {
  return `chartFormat.${id.replace(/^chart\./, '')}`;
}

/** One command as a host's registry takes it (`S`: the host's editor state). */
export interface ChartCommandSpec<S> {
  id: string;
  run: (s: S) => void;
  isEnabled: (s: S) => boolean;
  isChecked?: (s: S) => boolean;
}

/**
 * Registers the `chart.el.*` commands and `chart.format` with a host's command registry.
 * `info` reads the elements of the selected chart from the host's editor state.
 */
export function registerChartElementCommands<S>(
  register: (command: ChartCommandSpec<S>) => void,
  deps: {
    t: T;
    info: (s: S) => Info;
    /** Sends one change through the host's chart design command. */
    apply: (patch: ChartElementsPatch, s: S) => void;
    /** Shows or hides the Format chart pane. */
    openPane: () => void;
  },
): void {
  for (const a of CHART_ELEMENT_ACTIONS) {
    register({
      id: a.id,
      run: (s) => {
        const i = deps.info(s);
        if (!i || !a.enabled(i)) return;
        const patch = a.patch(i, deps.t);
        if (patch) deps.apply(patch, s);
      },
      isEnabled: (s) => {
        const i = deps.info(s);
        return !!i && a.enabled(i);
      },
      isChecked: (s) => {
        const i = deps.info(s);
        return !!i && a.checked(i);
      },
    });
  }
  register({
    id: CHART_FORMAT_COMMAND,
    run: deps.openPane,
    isEnabled: (s) => !!deps.info(s),
  });
}

/** The "Add chart element" dropdown: one section per element, a check mark on the current state. */
export function ChartElementsMenu({
  t,
  info,
  run,
  disabled,
}: {
  t: T;
  info: Info;
  /** Runs a command of the host's registry. */
  run: (id: string) => void;
  disabled?: boolean;
}) {
  const groups: string[] = [];
  for (const a of CHART_ELEMENT_ACTIONS) if (!groups.includes(a.group)) groups.push(a.group);
  return (
    <Popover
      label={t('chartFormat.addElement')}
      testId="chart-add-element"
      disabled={disabled || !info}
      trigger={<ListPlus size={18} aria-hidden="true" />}
    >
      {(close) => (
        <div className={styles.menu} role="menu" data-testid="chart-add-element-menu">
          {groups.map((g) => (
            <div
              key={g}
              className={styles.menuGroup}
              role="group"
              aria-label={t(`chartFormat.el.${g}.label`)}
            >
              <div className={styles.menuHead}>{t(`chartFormat.el.${g}.label`)}</div>
              {CHART_ELEMENT_ACTIONS.filter((a) => a.group === g).map((a) => {
                const on = !!info && a.checked(info);
                return (
                  <button
                    key={a.id}
                    type="button"
                    role="menuitemcheckbox"
                    aria-checked={on}
                    data-command={a.id}
                    className={styles.menuItem}
                    disabled={!info || !a.enabled(info)}
                    onMouseDown={(e) => e.preventDefault()}
                    onClick={() => {
                      run(a.id);
                      close();
                    }}
                  >
                    <span className={styles.menuCheck} aria-hidden="true">
                      {on ? '✓' : ''}
                    </span>
                    {t(chartElementLabelKey(a.id))}
                  </button>
                );
              })}
            </div>
          ))}
          <button
            type="button"
            role="menuitem"
            data-command={CHART_FORMAT_COMMAND}
            className={styles.menuItem}
            onMouseDown={(e) => e.preventDefault()}
            onClick={() => {
              run(CHART_FORMAT_COMMAND);
              close();
            }}
          >
            <span className={styles.menuCheck} aria-hidden="true" />
            {t('chartFormat.moreOptions')}
          </button>
        </div>
      )}
    </Popover>
  );
}

/** The "Chart elements" ribbon group: the Add chart element menu and the Format chart pane. */
export function chartElementsGroup({
  t,
  info,
  run,
  enabled,
  paneOpen,
}: {
  t: T;
  info: Info;
  run: (id: string) => void;
  enabled: boolean;
  paneOpen: boolean;
}): RibbonGroup {
  return {
    id: 'chartElements',
    label: t('chartFormat.group'),
    items: [
      {
        id: 'chart.addElement',
        label: t('chartFormat.addElement'),
        onSelect: () => {},
        enabled: enabled && !!info,
        render: <ChartElementsMenu t={t} info={info} run={run} disabled={!enabled} />,
      },
      {
        id: CHART_FORMAT_COMMAND,
        label: t('chartFormat.title'),
        icon: <SlidersHorizontal size={18} aria-hidden="true" />,
        size: 'large',
        enabled: enabled && !!info,
        checked: paneOpen,
        onSelect: () => run(CHART_FORMAT_COMMAND),
      },
    ],
  };
}
