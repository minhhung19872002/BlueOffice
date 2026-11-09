/**
 * BlueSheet sparklines (SHEET-SPARK-001, [MS-XLSX] `x14:sparklineGroup`): Insert › Sparklines
 * (line, column, win/loss from a data range into a location range), the Sparkline contextual
 * tab (type, highlighted points, colours and axis, group / ungroup, clear) and the dialogs
 * behind them. The engine draws the sparklines in their cells; every change is one undo step
 * through the command bridge (`sparkline*` commands).
 */
import { useTranslation } from '@blueoffice/i18n';
import type {
  SheetCommand,
  SheetHost,
  SheetRange,
  SparklineAxis,
  SparklineEmpty,
  SparklineGroupInfo,
  SparklinePatch,
  SparklineType,
} from '@blueoffice/platform';
import { Dialog, type RibbonGroup, type RibbonItem } from '@blueoffice/ui';
import {
  ChartColumn,
  ChartColumnStacked,
  ChartLine,
  Eraser,
  Group,
  Palette,
  Ungroup,
} from 'lucide-react';
import { useEffect, useState, type ReactNode } from 'react';
import { parseA1, rangeA1, rangeOf, type Range } from './grid';
import styles from './SheetEditor.module.css';

type T = ReturnType<typeof useTranslation>['t'];

/** The sparkline groups of `sheet`, reloaded after every edit (`revision`). */
export function useSparklines(
  host: SheetHost | undefined,
  sheet: number,
  revision: number,
): SparklineGroupInfo[] {
  const [groups, setGroups] = useState<SparklineGroupInfo[]>([]);
  useEffect(() => {
    if (!host) return;
    let live = true;
    host
      .exec<SparklineGroupInfo[]>({ cmd: 'sparklines', sheet })
      .then((g) => {
        if (live) setGroups(Array.isArray(g) ? g : []);
      })
      .catch(() => {
        if (live) setGroups([]);
      });
    return () => {
      live = false;
    };
  }, [host, sheet, revision]);
  return groups;
}

/** The group with a sparkline in cell (`row`, `col`). */
export function groupAt(
  groups: readonly SparklineGroupInfo[],
  row: number,
  col: number,
): SparklineGroupInfo | null {
  return (
    groups.find((g) =>
      g.sparklines.some((s) => s.location.row === row && s.location.col === col),
    ) ?? null
  );
}

/** A range `A1:B2`, optionally sheet-qualified (`'Bảng 1'!A1:B2`); `null` when invalid. */
export function parseRef(text: string): { sheet: string | null; range: Range } | null {
  const t = text.trim();
  const bang = t.lastIndexOf('!');
  const sheet =
    bang >= 0
      ? t
          .slice(0, bang)
          .replace(/^'(.*)'$/, '$1')
          .replace(/''/g, "'")
      : null;
  if (sheet === '') return null;
  const sel = parseA1(bang >= 0 ? t.slice(bang + 1) : t);
  return sel ? { sheet, range: rangeOf(sel) } : null;
}

/** Location proposed for data `r`: the column right of it (one cell per data row). */
export function proposeLocation(r: Range): Range {
  const col = r.c1 + 1;
  return { r0: r.r0, c0: col, r1: r.r1, c1: col };
}

type Add<S> = (
  id: string,
  run: (s: S) => Promise<unknown>,
  extra?: Partial<{
    isEnabled: (s: S) => boolean;
    isChecked: (s: S) => boolean;
  }>,
) => void;

/** Registers the sparkline commands (`insert.sparklines`, `sparkline.*`). */
export function registerSparklineCommands<
  S extends { sheet: number; hasWorkbook: boolean; sparkline: SparklineGroupInfo | null },
>(
  add: Add<S>,
  deps: {
    /** The selection as an engine range. */
    range: (s: S) => SheetRange;
    exec: (cmd: SheetCommand) => Promise<unknown>;
    openInsert: () => void;
    openOptions: () => void;
  },
): void {
  const has = (s: S) => s.hasWorkbook;
  const on = (s: S) => s.hasWorkbook && s.sparkline !== null;
  add('insert.sparklines', async () => deps.openInsert(), { isEnabled: has });
  const edit = (patch: (g: SparklineGroupInfo) => SparklinePatch) => async (s: S) => {
    const g = s.sparkline;
    if (!g) return;
    await deps.exec({ cmd: 'sparklineEdit', sheet: s.sheet, group: g.index, ...patch(g) });
  };
  const types: [string, SparklineType][] = [
    ['sparkline.typeLine', 'line'],
    ['sparkline.typeColumn', 'column'],
    ['sparkline.typeWinLoss', 'stacked'],
  ];
  for (const [id, type] of types) {
    add(
      id,
      edit(() => ({ type })),
      { isEnabled: on, isChecked: (s) => s.sparkline?.type === type },
    );
  }
  const flags = ['markers', 'high', 'low', 'first', 'last', 'negative'] as const;
  for (const f of flags) {
    add(
      `sparkline.${f}`,
      edit((g) => ({ [f]: !g[f] })),
      {
        isEnabled: (s) => on(s) && (f !== 'markers' || s.sparkline?.type === 'line'),
        isChecked: (s) => s.sparkline?.[f] === true,
      },
    );
  }
  add('sparkline.options', async () => deps.openOptions(), { isEnabled: on });
  add(
    'sparkline.group',
    (s) => deps.exec({ cmd: 'sparklineGroup', sheet: s.sheet, range: deps.range(s) }),
    {
      isEnabled: has,
    },
  );
  add(
    'sparkline.ungroup',
    (s) => deps.exec({ cmd: 'sparklineUngroup', sheet: s.sheet, range: deps.range(s) }),
    { isEnabled: (s) => on(s) && (s.sparkline?.sparklines.length ?? 0) > 1 },
  );
  add(
    'sparkline.clear',
    (s) => deps.exec({ cmd: 'sparklineClear', sheet: s.sheet, range: deps.range(s) }),
    {
      isEnabled: on,
    },
  );
  add(
    'sparkline.clearGroup',
    (s) =>
      deps.exec({ cmd: 'sparklineClear', sheet: s.sheet, range: deps.range(s), wholeGroups: true }),
    { isEnabled: on },
  );
}

type Item = (id: string, label: string, icon: ReactNode, size?: 'large' | 'small') => RibbonItem;

/** Groups of the Sparkline contextual tab. */
export function sparklineTabGroups(t: T, item: Item): RibbonGroup[] {
  const s = 16;
  const check = (id: string, key: string): RibbonItem => item(id, t(key), null, 'small');
  return [
    {
      id: 'sparklineType',
      label: t('sparkline.groupType'),
      items: [
        item('sparkline.typeLine', t('sparkline.line'), <ChartLine size={18} />),
        item('sparkline.typeColumn', t('sparkline.column'), <ChartColumn size={18} />),
        item('sparkline.typeWinLoss', t('sparkline.winLoss'), <ChartColumnStacked size={18} />),
      ],
    },
    {
      id: 'sparklineShow',
      label: t('sparkline.groupShow'),
      items: [
        check('sparkline.high', 'sparkline.high'),
        check('sparkline.low', 'sparkline.low'),
        check('sparkline.negative', 'sparkline.negative'),
        check('sparkline.first', 'sparkline.first'),
        check('sparkline.last', 'sparkline.last'),
        check('sparkline.markers', 'sparkline.markers'),
      ],
    },
    {
      id: 'sparklineStyle',
      label: t('sparkline.groupStyle'),
      items: [item('sparkline.options', t('sparkline.options'), <Palette size={18} />)],
    },
    {
      id: 'sparklineGroup',
      label: t('sparkline.groupGroup'),
      items: [
        item('sparkline.group', t('sparkline.group'), <Group size={s} />, 'small'),
        item('sparkline.ungroup', t('sparkline.ungroup'), <Ungroup size={s} />, 'small'),
        item('sparkline.clear', t('sparkline.clear'), <Eraser size={s} />, 'small'),
        item('sparkline.clearGroup', t('sparkline.clearGroup'), <Eraser size={s} />, 'small'),
      ],
    },
  ];
}

/** Insert › Sparklines: type, data range and location range. */
export function InsertSparklinesDialog({
  sheet,
  selection,
  exec,
  onClose,
}: {
  sheet: number;
  /** The selection, proposed as the data range. */
  selection: Range;
  /** Runs a command; resolves to `null` when it failed (the error is shown by the caller). */
  exec: (cmd: SheetCommand) => Promise<unknown>;
  onClose: () => void;
}) {
  const { t } = useTranslation();
  const multi = selection.r0 !== selection.r1 || selection.c0 !== selection.c1;
  const [type, setType] = useState<SparklineType>('line');
  const [data, setData] = useState(multi ? rangeA1(selection) : '');
  const [location, setLocation] = useState(
    multi ? rangeA1(proposeLocation(selection)) : rangeA1(selection),
  );
  const [error, setError] = useState<string | null>(null);
  const insert = async () => {
    const d = parseRef(data);
    const l = parseRef(location);
    if (!d || !l || l.sheet !== null || (l.range.r0 !== l.range.r1 && l.range.c0 !== l.range.c1)) {
      setError(t('sparkline.invalidRange'));
      return;
    }
    const r = await exec({
      cmd: 'sparklineInsert',
      sheet,
      type,
      data: data.trim(),
      location: l.range,
    });
    if (r !== null) onClose();
  };
  const kinds: [SparklineType, string][] = [
    ['line', 'sparkline.line'],
    ['column', 'sparkline.column'],
    ['stacked', 'sparkline.winLoss'],
  ];
  return (
    <Dialog
      open
      onOpenChange={(o) => !o && onClose()}
      title={t('sparkline.insertTitle')}
      testId="sparkline-insert"
      width={420}
      footer={
        <>
          <button type="button" className={styles.btn} onClick={onClose}>
            {t('sparkline.cancel')}
          </button>
          <button
            type="button"
            className={styles.btnPrimary}
            data-testid="sparkline-insert-ok"
            onClick={() => void insert()}
          >
            {t('sparkline.ok')}
          </button>
        </>
      }
    >
      <div className={styles.form}>
        <fieldset className={styles.row} aria-label={t('sparkline.type')}>
          {kinds.map(([k, key]) => (
            <label key={k} className={styles.check}>
              <input
                type="radio"
                name="sparkline-type"
                data-testid={`sparkline-type-${k}`}
                checked={type === k}
                onChange={() => setType(k)}
              />
              {t(key)}
            </label>
          ))}
        </fieldset>
        <label>
          {t('sparkline.data')}
          <input
            data-testid="sparkline-data"
            value={data}
            onChange={(e) => setData(e.target.value)}
          />
        </label>
        <p className={styles.hint}>{t('sparkline.dataHint')}</p>
        <label>
          {t('sparkline.location')}
          <input
            data-testid="sparkline-location"
            value={location}
            onChange={(e) => setLocation(e.target.value)}
          />
        </label>
        <p className={styles.hint}>{t('sparkline.locationHint')}</p>
        {error && (
          <p className={styles.error} role="alert">
            {error}
          </p>
        )}
      </div>
    </Dialog>
  );
}

const COLOR_KEYS = [
  ['series', 'colorSeries'],
  ['negative', 'colorNegative'],
  ['axis', 'colorAxis'],
  ['markers', 'colorMarkers'],
  ['first', 'colorFirst'],
  ['last', 'colorLast'],
  ['high', 'colorHigh'],
  ['low', 'colorLow'],
] as const;

/** The patch the options dialog sends for draft `d` (every setting it shows). */
export function optionsPatch(d: SparklineGroupInfo): SparklinePatch {
  const p: SparklinePatch = {
    emptyAs: d.emptyAs,
    displayXAxis: d.displayXAxis,
    displayHidden: d.displayHidden,
    rightToLeft: d.rightToLeft,
    minAxis: d.minAxis,
    maxAxis: d.maxAxis,
    lineWeight: d.lineWeight,
  };
  if (d.minAxis === 'custom' && d.manualMin !== null) p.manualMin = d.manualMin;
  if (d.maxAxis === 'custom' && d.manualMax !== null) p.manualMax = d.manualMax;
  for (const [k, patchKey] of COLOR_KEYS) p[patchKey] = d.colorsHex[k];
  return p;
}

/** Colours and axis options of a sparkline group. */
export function SparklineOptionsDialog({
  sheet,
  group,
  exec,
  onClose,
}: {
  sheet: number;
  group: SparklineGroupInfo;
  exec: (cmd: SheetCommand) => Promise<unknown>;
  onClose: () => void;
}) {
  const { t } = useTranslation();
  const [d, setD] = useState<SparklineGroupInfo>(group);
  const [error, setError] = useState<string | null>(null);
  const set = (patch: Partial<SparklineGroupInfo>) => setD((x) => ({ ...x, ...patch }));
  const apply = async () => {
    if (
      d.minAxis === 'custom' &&
      d.maxAxis === 'custom' &&
      d.manualMin !== null &&
      d.manualMax !== null &&
      d.manualMin >= d.manualMax
    ) {
      setError(t('sparkline.minMax'));
      return;
    }
    const r = await exec({ cmd: 'sparklineEdit', sheet, group: group.index, ...optionsPatch(d) });
    if (r !== null) onClose();
  };
  const axisSelect = (which: 'minAxis' | 'maxAxis', manual: 'manualMin' | 'manualMax') => (
    <div className={styles.row}>
      <label>
        {t(which === 'minAxis' ? 'sparkline.minAxis' : 'sparkline.maxAxis')}
        <select
          data-testid={`sparkline-${which}`}
          value={d[which]}
          onChange={(e) => set({ [which]: e.target.value as SparklineAxis })}
        >
          <option value="individual">{t('sparkline.axisIndividual')}</option>
          <option value="group">{t('sparkline.axisGroup')}</option>
          <option value="custom">{t('sparkline.axisCustom')}</option>
        </select>
      </label>
      {d[which] === 'custom' && (
        <input
          type="number"
          aria-label={t(which === 'minAxis' ? 'sparkline.minAxis' : 'sparkline.maxAxis')}
          data-testid={`sparkline-${manual}`}
          value={d[manual] ?? ''}
          onChange={(e) => {
            const v = e.target.value === '' ? null : Number(e.target.value);
            set({ [manual]: v !== null && Number.isFinite(v) ? v : null });
          }}
        />
      )}
    </div>
  );
  const empties: [SparklineEmpty, string][] = [
    ['gap', 'sparkline.emptyGap'],
    ['zero', 'sparkline.emptyZero'],
    ['span', 'sparkline.emptySpan'],
  ];
  return (
    <Dialog
      open
      onOpenChange={(o) => !o && onClose()}
      title={t('sparkline.optionsTitle')}
      testId="sparkline-options"
      width={460}
      footer={
        <>
          <button type="button" className={styles.btn} onClick={onClose}>
            {t('sparkline.cancel')}
          </button>
          <button
            type="button"
            className={styles.btnPrimary}
            data-testid="sparkline-options-ok"
            onClick={() => void apply()}
          >
            {t('sparkline.apply')}
          </button>
        </>
      }
    >
      <div className={styles.form}>
        <fieldset className={styles.row} aria-label={t('sparkline.colors')}>
          {COLOR_KEYS.map(([k, key]) => (
            <label key={k}>
              {t(`sparkline.${key}`)}
              <input
                type="color"
                data-testid={`sparkline-${key}`}
                value={d.colorsHex[k].toLowerCase()}
                onChange={(e) =>
                  set({ colorsHex: { ...d.colorsHex, [k]: e.target.value.toUpperCase() } })
                }
              />
            </label>
          ))}
        </fieldset>
        <label>
          {t('sparkline.lineWeight')}
          <input
            type="number"
            min={0.25}
            max={6}
            step={0.25}
            data-testid="sparkline-lineWeight"
            value={d.lineWeight}
            onChange={(e) => {
              const v = Number(e.target.value);
              if (Number.isFinite(v) && v > 0) set({ lineWeight: v });
            }}
          />
        </label>
        <p className={styles.hint}>{t('sparkline.axis')}</p>
        {axisSelect('minAxis', 'manualMin')}
        {axisSelect('maxAxis', 'manualMax')}
        <label className={styles.check}>
          <input
            type="checkbox"
            data-testid="sparkline-displayXAxis"
            checked={d.displayXAxis}
            onChange={(e) => set({ displayXAxis: e.target.checked })}
          />
          {t('sparkline.showAxis')}
        </label>
        <label className={styles.check}>
          <input
            type="checkbox"
            data-testid="sparkline-rightToLeft"
            checked={d.rightToLeft}
            onChange={(e) => set({ rightToLeft: e.target.checked })}
          />
          {t('sparkline.rightToLeft')}
        </label>
        <label>
          {t('sparkline.emptyCells')}
          <select
            data-testid="sparkline-emptyAs"
            value={d.emptyAs}
            onChange={(e) => set({ emptyAs: e.target.value as SparklineEmpty })}
          >
            {empties.map(([v, key]) => (
              <option key={v} value={v}>
                {t(key)}
              </option>
            ))}
          </select>
        </label>
        <label className={styles.check}>
          <input
            type="checkbox"
            data-testid="sparkline-displayHidden"
            checked={d.displayHidden}
            onChange={(e) => set({ displayHidden: e.target.checked })}
          />
          {t('sparkline.showHidden')}
        </label>
        {error && (
          <p className={styles.error} role="alert">
            {error}
          </p>
        )}
      </div>
    </Dialog>
  );
}
