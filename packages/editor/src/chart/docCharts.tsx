/**
 * Charts in BlueDoc (CORE-CHART-007 / CORE-CHART-004): Insert › Chart, the chart's data / type /
 * title / legend through the shared chart dialog, the shared Chart Design groups, quick type
 * changes and text wrapping (inline, square, tight, top and bottom, behind, in front). Every
 * action is a registry command; edits are `insertChart` / `setChart` engine commands through the
 * document controller (one undo step each, synced to collaborators like any edit).
 */
import type { useTranslation } from '@blueoffice/i18n';
import type {
  ChartDesignSpec,
  ChartGallery,
  DocChartInfo,
  DocChartWrap,
  EditCommand,
  EditResult,
  SlideChartKind,
} from '@blueoffice/platform';
import type { RibbonGroup, RibbonItem, RibbonTab } from '@blueoffice/ui';
import type { ReactNode } from 'react';
import type { CommandRegistry } from '../commands';
import type { ChartDraft } from './ChartDialog';
import { chartDesignGroups, layoutTexts } from './chartDesign';

type T = ReturnType<typeof useTranslation>['t'];

/** Quick chart type buttons of the Chart Design tab. */
export const DOC_CHART_TYPES: SlideChartKind[] = ['column', 'bar', 'line', 'area', 'pie'];

/** Text wrapping choices (mirrors `bo_engine::doc_chart::WRAPS`). */
export const DOC_CHART_WRAPS: DocChartWrap[] = [
  'inline',
  'square',
  'tight',
  'topAndBottom',
  'behind',
  'inFront',
];

/** The editor state the chart commands read. */
export interface ChartState {
  doc?: unknown;
  edit?: Pick<EditResult, 'format' | 'chart'> | null;
}

/** Fields of a new chart: a column chart of localized sample data. */
export function newChartDraft(t: T): ChartDraft {
  return {
    kind: 'column',
    title: '',
    legend: 'b',
    data: {
      categories: [1, 2, 3, 4].map((n) => t('slideEditor.chartDefaultCategory', { n })),
      series: [
        [4.3, 2.5, 3.5, 4.5],
        [2.4, 4.4, 1.8, 2.8],
        [2, 2, 3, 5],
      ].map((values, j) => ({ name: t('slideEditor.chartSeries', { n: j + 1 }), values })),
    },
  };
}

/** Dialog fields of an existing chart. */
export function chartDraftOf(c: DocChartInfo): ChartDraft {
  return {
    kind: c.kind === 'other' ? 'column' : c.kind,
    title: c.title ?? '',
    legend: c.legend ?? 'none',
    data: c.data,
  };
}

/** The engine command a dialog result applies (`insertChart` or `setChart`). */
export function chartCommand(mode: 'insert' | 'edit', d: ChartDraft, t: T): EditCommand {
  // A chosen chart template brings its type and formatting.
  const tpl = d.template ? { template: d.template, texts: layoutTexts(t) } : {};
  const fields = { kind: d.kind, title: d.title, legend: d.legend, data: d.data, ...tpl };
  return mode === 'insert' ? { cmd: 'insertChart', ...fields } : { cmd: 'setChart', ...fields };
}

export interface DocChartDeps {
  t: T;
  /** Runs an engine command through the document controller. */
  run: (c: EditCommand) => Promise<EditResult | null | undefined>;
  /** Opens the chart dialog. */
  open: (mode: 'insert' | 'edit', initial: ChartDraft) => void;
}

/** Registers `insert.chart`, `chart.edit`, `chart.type.*` and `chart.wrap.*`. */
export function registerDocChartCommands<S extends ChartState>(
  r: CommandRegistry<S>,
  { t, run, open }: DocChartDeps,
): void {
  const hasDoc = (s: S) => !!s.doc;
  const chart = (s: S) => hasDoc(s) && !!s.edit?.format.chart;
  r.register({
    id: 'insert.chart',
    isEnabled: hasDoc,
    run: () => open('insert', newChartDraft(t)),
  });
  r.register({
    id: 'chart.edit',
    isEnabled: chart,
    run: async () => {
      const c = (await run({ cmd: 'chartInfo' }))?.chart;
      if (c) open('edit', chartDraftOf(c));
    },
  });
  for (const kind of DOC_CHART_TYPES) {
    r.register({
      id: `chart.type.${kind}`,
      isEnabled: chart,
      isChecked: (s) => s.edit?.chart?.kind === kind,
      run: () => void run({ cmd: 'setChart', kind }),
    });
  }
  for (const wrap of DOC_CHART_WRAPS) {
    r.register({
      id: `chart.wrap.${wrap}`,
      isEnabled: chart,
      isChecked: (s) => s.edit?.chart?.wrap === wrap,
      run: () => void run({ cmd: 'setChart', wrap }),
    });
  }
}

/** Gallery of the selected chart through the document command channel. */
export function docGalleryLoader(
  run: (c: EditCommand) => Promise<EditResult | null | undefined>,
): () => Promise<ChartGallery | null> {
  return async () =>
    (await run({ cmd: 'chartGallery', width: 100, height: 68 }))?.chartGallery ?? null;
}

/** A registry command as a ribbon item (the host's `item`). */
type ItemFn = (
  id: string,
  labelKey: string,
  icon: ReactNode,
  size?: 'large' | 'small',
) => RibbonItem;

/** The Chart Design contextual tab of the chart at the selection. */
export function docChartTabs({
  t,
  item,
  icons,
  shown,
  load,
  apply,
  onSaveTemplate,
}: {
  t: T;
  item: ItemFn;
  icons: { edit: ReactNode; types: Record<string, ReactNode>; wrap: ReactNode };
  shown: boolean;
  load: () => Promise<ChartGallery | null>;
  apply: (spec: ChartDesignSpec) => void;
  onSaveTemplate: () => void;
}): RibbonTab[] {
  if (!shown) return [];
  const groups: RibbonGroup[] = [
    {
      id: 'chartData',
      label: t('slideEditor.groupChartData'),
      items: [item('chart.edit', 'slideEditor.editChart', icons.edit)],
    },
    {
      id: 'chartType',
      label: t('slideEditor.chartType'),
      items: DOC_CHART_TYPES.map((k) =>
        item(`chart.type.${k}`, `slideEditor.chartKinds.${k}`, icons.types[k], 'small'),
      ),
    },
    ...chartDesignGroups({ t, load, apply, enabled: shown, onSaveTemplate }),
    {
      id: 'chartWrap',
      label: t('docChart.wrapGroup'),
      items: DOC_CHART_WRAPS.map((w) =>
        item(`chart.wrap.${w}`, `docChart.wrap.${w}`, icons.wrap, 'small'),
      ),
    },
  ];
  return [{ id: 'chart', context: 'chart', label: t('slideEditor.chartTab'), groups }];
}
