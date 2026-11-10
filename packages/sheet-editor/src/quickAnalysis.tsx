/**
 * Quick Analysis (CORE-DESIGN-001, "Phân tích nhanh"): a floating button at the bottom-right of a
 * multi-cell selection opens tabs of one-click choices — conditional formats, recommended charts,
 * total rows / columns, a table or pivot table, sparklines. Every choice is sent through the
 * normal sheet commands (one `batch` where it takes several, so one undo step); the engine only
 * reads the data (`quickAnalysis`, `recommendCharts`).
 *
 * The recommended charts are shared with Insert › Chart › Recommended charts.
 */
import { useTranslation } from '@blueoffice/i18n';
import type {
  ChartRecommendation,
  CfRuleSpec,
  QuickAnalysisInfo,
  RecommendChartsResult,
  SheetCommand,
  SheetRange,
  SparklineType,
} from '@blueoffice/platform';
import { Dialog } from '@blueoffice/ui';
import {
  ArrowUpWideNarrow,
  BarChart3,
  Eraser,
  Grid3x3,
  Palette,
  Sigma,
  Table2,
  TableProperties,
  TrendingUp,
  Zap,
} from 'lucide-react';
import { useEffect, useState, type ReactNode } from 'react';
import { colName } from './grid';
import css from './quickAnalysis.module.css';

export type QuickTab = 'formatting' | 'charts' | 'totals' | 'tables' | 'sparklines';
export const QUICK_TABS: readonly QuickTab[] = [
  'formatting',
  'charts',
  'totals',
  'tables',
  'sparklines',
];

export type QuickFormat = 'dataBar' | 'colorScale' | 'iconSet' | 'greaterThan' | 'top10' | 'clear';
export const QUICK_FORMATS: readonly QuickFormat[] = [
  'dataBar',
  'colorScale',
  'iconSet',
  'greaterThan',
  'top10',
  'clear',
];

export type TotalKind = 'sum' | 'average' | 'count' | 'percent' | 'running';
export const TOTAL_KINDS: readonly TotalKind[] = ['sum', 'average', 'count', 'percent', 'running'];

/** Where the totals go: a row below the selection or a column right of it. */
export type TotalDir = 'rows' | 'cols';

export const QUICK_SPARKLINES: readonly SparklineType[] = ['line', 'column', 'stacked'];

/** Words written into the sheet with the totals (localised by the caller). */
export interface TotalLabels {
  sum: string;
  average: string;
  count: string;
  percent: string;
  running: string;
}

const abs = (row: number, col: number) => `$${colName(col)}$${row + 1}`;
const rel = (row: number, col: number) => `${colName(col)}${row + 1}`;
const absRange = (r: SheetRange) => `${abs(r.r0, r.c0)}:${abs(r.r1, r.c1)}`;

/** The numbers of the selection: right of the label columns, below the header row. */
export function dataBlock(sel: SheetRange, info: QuickAnalysisInfo | null): SheetRange {
  const top = sel.r0 + (info?.header ? 1 : 0);
  const cols = info?.numericCols.filter((c) => c >= sel.c0 && c <= sel.c1) ?? [];
  const left = cols.length ? Math.min(...cols) : sel.c0 + (info?.labelCols ?? 0);
  return {
    r0: Math.min(top, sel.r1),
    c0: Math.min(left, sel.c1),
    r1: sel.r1,
    c1: cols.length ? Math.max(...cols) : sel.c1,
  };
}

/** The conditional format command of a Formatting choice. */
export function formattingCommand(
  kind: QuickFormat,
  sheet: number,
  block: SheetRange,
): SheetCommand {
  if (kind === 'clear') return { cmd: 'clearCf', sheet, range: block };
  const rule: CfRuleSpec = (() => {
    switch (kind) {
      case 'dataBar':
        return { kind: 'dataBar', color: '#3E7CC9' };
      case 'colorScale':
        return {
          kind: 'colorScale',
          minColor: '#D9534F',
          midColor: '#F3D36B',
          maxColor: '#4FA765',
        };
      case 'iconSet':
        return { kind: 'iconSet', set: '3Arrows' };
      case 'greaterThan':
        // Above the average of the numbers (§18.3.1.10 cellIs, operator greaterThan).
        return {
          kind: 'cellIs',
          operator: 'greaterThan',
          formula1: `AVERAGE(${absRange(block)})`,
          style: { fill: '#FBE3C2', fontColor: '#7A4300' },
        };
      case 'top10':
        // The top 10 % (§18.3.1.10 top10 with percent).
        return {
          kind: 'top10',
          rank: 10,
          percent: true,
          style: { fill: '#D6ECD2', fontColor: '#1E5B26' },
        };
    }
  })();
  return { cmd: 'addCf', sheet, range: block, rule };
}

/**
 * The commands of a Totals choice as one `batch`: formulas in the row below (`rows`) or the
 * column right of (`cols`) the selection, a label in the label column / header row, bold, and a
 * percent format for `% of total`. `null` when the selection has no numbers.
 */
export function totalsCommand(
  kind: TotalKind,
  dir: TotalDir,
  sheet: number,
  sel: SheetRange,
  info: QuickAnalysisInfo,
  labels: TotalLabels,
): SheetCommand | null {
  const b = dataBlock(sel, info);
  const grand = absRange(b);
  const cells: [number, number, string][] = [];
  const fn = kind === 'average' ? 'AVERAGE' : kind === 'count' ? 'COUNT' : 'SUM';
  if (dir === 'rows') {
    const row = sel.r1 + 1;
    const cols = info.numericCols.filter((c) => c >= b.c0 && c <= b.c1);
    if (!cols.length) return null;
    for (const c of cols) {
      const span = `${rel(b.r0, c)}:${rel(b.r1, c)}`;
      const f =
        kind === 'percent'
          ? `=SUM(${span})/SUM(${grand})`
          : kind === 'running'
            ? `=SUM(${abs(b.r0, b.c0)}:${rel(b.r1, c)})`
            : `=${fn}(${span})`;
      cells.push([row, c, f]);
    }
    if (info.labelCols > 0 && sel.c0 < b.c0) cells.push([row, sel.c0, labels[kind]]);
    const span: SheetRange = { r0: row, c0: sel.c0, r1: row, c1: sel.c1 };
    return batch(
      sheet,
      cells,
      span,
      kind === 'percent' ? { r0: row, c0: b.c0, r1: row, c1: b.c1 } : null,
    );
  }
  const col = sel.c1 + 1;
  const rows = info.numericRows.filter((r) => r >= b.r0 && r <= b.r1);
  if (!rows.length) return null;
  for (const r of rows) {
    const span = `${rel(r, b.c0)}:${rel(r, b.c1)}`;
    const f =
      kind === 'percent'
        ? `=SUM(${span})/SUM(${grand})`
        : kind === 'running'
          ? `=SUM(${abs(b.r0, b.c0)}:${rel(r, b.c1)})`
          : `=${fn}(${span})`;
    cells.push([r, col, f]);
  }
  if (info.header && sel.r0 < b.r0) cells.push([sel.r0, col, labels[kind]]);
  const span: SheetRange = { r0: sel.r0, c0: col, r1: sel.r1, c1: col };
  return batch(
    sheet,
    cells,
    span,
    kind === 'percent' ? { r0: b.r0, c0: col, r1: b.r1, c1: col } : null,
  );
}

function batch(
  sheet: number,
  cells: [number, number, string][],
  bold: SheetRange,
  percent: SheetRange | null,
): SheetCommand {
  const cmds: SheetCommand[] = [
    { cmd: 'setInputs', sheet, cells },
    { cmd: 'format', sheet, range: bold, patch: { bold: true } },
  ];
  if (percent) cmds.push({ cmd: 'format', sheet, range: percent, patch: { numFmt: '0.0%' } });
  return { cmd: 'batch', cmds };
}

/** Sparklines of the selection's rows in the column right of it. */
export function sparklineCommand(
  type: SparklineType,
  sheet: number,
  sel: SheetRange,
  info: QuickAnalysisInfo | null,
): SheetCommand {
  const b = dataBlock(sel, info);
  return {
    cmd: 'sparklineInsert',
    sheet,
    type,
    data: `${rel(b.r0, b.c0)}:${rel(b.r1, b.c1)}`,
    location: { r0: b.r0, c0: sel.c1 + 1, r1: b.r1, c1: sel.c1 + 1 },
  };
}

const svgUrl = (svg: string) => `data:image/svg+xml;charset=utf-8,${encodeURIComponent(svg)}`;

/** Recommended charts of a range as preview buttons (Quick Analysis and Insert › Chart). */
export function RecommendedCharts({
  sheet,
  range,
  query,
  onPick,
  limit = 6,
  width = 132,
  height = 86,
  onHover,
}: {
  sheet: number;
  range: SheetRange;
  /** Runs a read-only command. */
  query: <T>(cmd: SheetCommand) => Promise<T | null>;
  onPick: (kind: string) => void;
  limit?: number;
  width?: number;
  height?: number;
  /** The previewed recommendation under the pointer (`null` when it leaves). */
  onHover?: (rec: ChartRecommendation | null) => void;
}) {
  const { t } = useTranslation();
  const key = `${sheet}:${range.r0}:${range.c0}:${range.r1}:${range.c1}`;
  // The answer for the selection it was asked for (a newer selection waits for its own).
  const [answer, setAnswer] = useState<{ key: string; res: RecommendChartsResult } | null>(null);
  const res = answer?.key === key ? answer.res : null;
  useEffect(() => {
    let live = true;
    void query<RecommendChartsResult>({
      cmd: 'recommendCharts',
      sheet,
      range,
      width,
      height,
      limit,
    }).then((r) => {
      if (live) setAnswer({ key, res: r ?? { shape: null, range, charts: [] } });
    });
    return () => {
      live = false;
    };
    // `range` is compared through `key`.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [query, key, limit, width, height]);
  if (!res) return <p className={css.hint}>{t('quickAnalysis.loading')}</p>;
  if (!res.charts.length) return <p className={css.hint}>{t('quickAnalysis.charts.none')}</p>;
  return (
    <div className={css.charts} role="list" data-testid="recommended-charts">
      {res.charts.map((c) => (
        <button
          key={c.kind}
          type="button"
          role="listitem"
          className={css.chart}
          data-testid={`recommended-chart-${c.kind}`}
          title={t(`quickAnalysis.reason.${c.reason}`)}
          onMouseEnter={() => onHover?.(c)}
          onMouseLeave={() => onHover?.(null)}
          onFocus={() => onHover?.(c)}
          onBlur={() => onHover?.(null)}
          onClick={() => onPick(c.kind)}
        >
          <img src={svgUrl(c.svg)} alt="" width={width} height={height} />
          <span>{t(`sheetEditor.chart.types.${c.kind}`)}</span>
        </button>
      ))}
    </div>
  );
}

/** Insert › Chart › Recommended charts. */
export function RecommendedChartsDialog({
  sheet,
  range,
  query,
  onInsert,
  onClose,
}: {
  sheet: number;
  range: SheetRange;
  query: <T>(cmd: SheetCommand) => Promise<T | null>;
  onInsert: (kind: string) => void;
  onClose: () => void;
}) {
  const { t } = useTranslation();
  const [hover, setHover] = useState<ChartRecommendation | null>(null);
  return (
    <Dialog
      open
      onOpenChange={(o) => !o && onClose()}
      title={t('quickAnalysis.recommended.title')}
      testId="recommended-charts-dialog"
      width={640}
      footer={
        <button type="button" onClick={onClose}>
          {t('quickAnalysis.close')}
        </button>
      }
    >
      <p className={css.hint}>
        {hover ? t(`quickAnalysis.reason.${hover.reason}`) : t('quickAnalysis.recommended.hint')}
      </p>
      <RecommendedCharts
        sheet={sheet}
        range={range}
        query={query}
        limit={8}
        width={168}
        height={108}
        onHover={setHover}
        onPick={(k) => {
          onInsert(k);
          onClose();
        }}
      />
    </Dialog>
  );
}

const FORMAT_ICONS: Record<QuickFormat, ReactNode> = {
  dataBar: <BarChart3 size={18} />,
  colorScale: <Palette size={18} />,
  iconSet: <ArrowUpWideNarrow size={18} />,
  greaterThan: <Zap size={18} />,
  top10: <TrendingUp size={18} />,
  clear: <Eraser size={18} />,
};

/** The Quick Analysis button and its panel. */
export function QuickAnalysis({
  sheet,
  range,
  anchor,
  exec,
  query,
  insertChart,
  runCommand,
  onNotice,
  open,
  onOpenChange,
}: {
  sheet: number;
  /** The multi-cell selection. */
  range: SheetRange;
  /** Bottom-right corner of the selection in the grid's coordinates (CSS pixels). */
  anchor: { x: number; y: number };
  /** Runs an editing command. */
  exec: <T>(cmd: SheetCommand) => Promise<T | null>;
  /** Runs a read-only command. */
  query: <T>(cmd: SheetCommand) => Promise<T | null>;
  /** Insert a chart of `kind` over the selection (selects it). */
  insertChart: (kind: string) => void;
  /** Run a registry command (`insert.pivot`). */
  runCommand: (id: string) => void;
  /** Show a message (why a choice is not possible). */
  onNotice: (text: string) => void;
  /** The panel is open (`format.quickAnalysis` opens it too). */
  open: boolean;
  onOpenChange: (open: boolean) => void;
}) {
  const { t } = useTranslation();
  const setOpen = (o: boolean | ((x: boolean) => boolean)) =>
    onOpenChange(typeof o === 'function' ? o(open) : o);
  const [tab, setTab] = useState<QuickTab>('formatting');
  const [hint, setHint] = useState<string | null>(null);
  const key = `${sheet}:${range.r0}:${range.c0}:${range.r1}:${range.c1}`;
  // The data of the selection it was read for.
  const [read, setRead] = useState<{ key: string; info: QuickAnalysisInfo | null } | null>(null);
  const info = read?.key === key ? read.info : null;
  useEffect(() => {
    if (!open) return;
    let live = true;
    void query<QuickAnalysisInfo>({ cmd: 'quickAnalysis', sheet, range }).then((r) => {
      if (live) setRead({ key, info: r ?? null });
    });
    return () => {
      live = false;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [open, key, query]);
  const labels: TotalLabels = {
    sum: t('quickAnalysis.totals.label.sum'),
    average: t('quickAnalysis.totals.label.average'),
    count: t('quickAnalysis.totals.label.count'),
    percent: t('quickAnalysis.totals.label.percent'),
    running: t('quickAnalysis.totals.label.running'),
  };
  const done = (p: Promise<unknown>) => void p.then(() => setOpen(false));
  const block = dataBlock(range, info);
  const total = (kind: TotalKind, dir: TotalDir) => {
    if (!info) return;
    if (dir === 'rows' ? !info.belowFree : !info.rightFree) {
      onNotice(t('quickAnalysis.totals.notFree'));
      return;
    }
    const cmd = totalsCommand(kind, dir, sheet, range, info, labels);
    if (!cmd) {
      onNotice(t('quickAnalysis.totals.noNumbers'));
      return;
    }
    done(exec(cmd));
  };
  const tile = (id: string, label: string, icon: ReactNode, onClick: () => void, desc?: string) => (
    <button
      key={id}
      type="button"
      className={css.tile}
      data-testid={`qa-${id}`}
      onClick={onClick}
      onMouseEnter={() => setHint(desc ?? label)}
      onMouseLeave={() => setHint(null)}
    >
      {icon}
      <span>{label}</span>
    </button>
  );
  const body = () => {
    switch (tab) {
      case 'formatting':
        return QUICK_FORMATS.map((k) =>
          tile(
            `fmt-${k}`,
            t(`quickAnalysis.fmt.${k}`),
            FORMAT_ICONS[k],
            () => done(exec(formattingCommand(k, sheet, block))),
            t(`quickAnalysis.fmtHint.${k}`),
          ),
        );
      case 'charts':
        return (
          <RecommendedCharts
            sheet={sheet}
            range={range}
            query={query}
            limit={5}
            onHover={(c) => setHint(c ? t(`quickAnalysis.reason.${c.reason}`) : null)}
            onPick={(k) => {
              insertChart(k);
              setOpen(false);
            }}
          />
        );
      case 'totals':
        return (['rows', 'cols'] as const).flatMap((dir) =>
          TOTAL_KINDS.map((k) =>
            tile(
              `total-${dir}-${k}`,
              t(`quickAnalysis.totals.${dir}.${k}`),
              <Sigma size={18} />,
              () => total(k, dir),
            ),
          ),
        );
      case 'tables':
        return [
          tile('table', t('quickAnalysis.tables.table'), <Table2 size={18} />, () =>
            done(exec({ cmd: 'createTable', sheet, range })),
          ),
          tile('pivot', t('quickAnalysis.tables.pivot'), <TableProperties size={18} />, () => {
            setOpen(false);
            runCommand('insert.pivot');
          }),
        ];
      case 'sparklines':
        return QUICK_SPARKLINES.map((k) =>
          tile(`spark-${k}`, t(`quickAnalysis.spark.${k}`), <Grid3x3 size={18} />, () => {
            if (info && !info.rightFree) {
              onNotice(t('quickAnalysis.totals.notFree'));
              return;
            }
            done(exec(sparklineCommand(k, sheet, range, info)));
          }),
        );
    }
  };
  return (
    <>
      <button
        type="button"
        className={css.button}
        style={{ left: anchor.x + 2, top: anchor.y + 2 }}
        data-testid="quick-analysis"
        aria-label={t('quickAnalysis.button')}
        title={t('quickAnalysis.button')}
        aria-expanded={open}
        onMouseDown={(e) => e.stopPropagation()}
        onClick={() => setOpen((o) => !o)}
      >
        <Zap size={14} aria-hidden />
      </button>
      {open && (
        <div
          className={css.panel}
          style={{ left: anchor.x + 2, top: anchor.y + 26 }}
          role="dialog"
          aria-label={t('quickAnalysis.title')}
          data-testid="quick-analysis-panel"
          onMouseDown={(e) => e.stopPropagation()}
          onKeyDown={(e) => {
            if (e.key === 'Escape') setOpen(false);
          }}
        >
          <div className={css.tabs} role="tablist">
            {QUICK_TABS.map((k) => (
              <button
                key={k}
                type="button"
                role="tab"
                aria-selected={tab === k}
                className={tab === k ? css.tabActive : css.tab}
                data-testid={`qa-tab-${k}`}
                onClick={() => setTab(k)}
              >
                {t(`quickAnalysis.tabs.${k}`)}
              </button>
            ))}
          </div>
          <div className={css.body} role="tabpanel">
            {body()}
          </div>
          <p className={css.hint}>{hint ?? t(`quickAnalysis.tabHint.${tab}`)}</p>
        </div>
      )}
    </>
  );
}
