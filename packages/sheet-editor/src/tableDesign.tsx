/**
 * BlueSheet tables (SHEET-TABLE-001, ECMA-376 Part 1, §18.5): the Table Design contextual tab
 * shown while the active cell is in a table — name and resize, convert to range, remove
 * duplicates, slicer, style options (header / totals rows, banded rows / columns, first / last
 * column, filter buttons), the style gallery (BlueSheet's own palette names), totals functions,
 * and the automatic behaviour (expansion, calculated columns) with a bar offering to undo it.
 * Every change is one undo step through the command bridge (`table*` commands).
 */
import { useTranslation } from '@blueoffice/i18n';
import type {
  SheetCommand,
  SheetHost,
  SheetRange,
  TableInfo,
  TableList,
  TableStyleEntry,
  TableTotalFunction,
} from '@blueoffice/platform';
import { Dialog, Popover, type RibbonGroup, type RibbonItem } from '@blueoffice/ui';
import {
  Columns3,
  Eraser,
  ListFilter,
  Paintbrush,
  PencilLine,
  Scaling,
  Sigma,
  Table2,
} from 'lucide-react';
import { useEffect, useState, type ReactNode } from 'react';
import { parseA1, rangeA1, rangeOf, type Range } from './grid';
import styles from './SheetEditor.module.css';
import tstyles from './tableDesign.module.css';

type T = ReturnType<typeof useTranslation>['t'];

/** Style families and accent numbers of the gallery (same order as the engine's `tableStyles`). */
export const TABLE_STYLE_FAMILIES = ['Light', 'Medium', 'Dark'] as const;
export const TABLE_STYLE_NAMES: string[] = TABLE_STYLE_FAMILIES.flatMap((f) =>
  [1, 2, 3, 4, 5, 6, 7].map((n) => `TableStyle${f}${n}`),
);

/** Totals functions offered for a column (§18.18.86). */
export const TOTAL_FUNCTIONS: TableTotalFunction[] = [
  'none',
  'sum',
  'average',
  'count',
  'countNums',
  'max',
  'min',
  'stdDev',
  'var',
  'custom',
];

/** The tables of `sheet`, reloaded after every edit (`revision`). */
export function useTables(
  host: SheetHost | undefined,
  sheet: number,
  revision: number,
): TableList | null {
  const [list, setList] = useState<TableList | null>(null);
  useEffect(() => {
    if (!host) return;
    let live = true;
    host
      .exec<TableList>({ cmd: 'tables', sheet })
      .then((l) => {
        if (live) setList(l && Array.isArray(l.tables) ? l : null);
      })
      .catch(() => {
        if (live) setList(null);
      });
    return () => {
      live = false;
    };
  }, [host, sheet, revision]);
  return list;
}

/** The table containing (`row`, `col`). */
export function tableAt(list: TableList | null, row: number, col: number): TableInfo | null {
  return (
    list?.tables.find(
      (t) => row >= t.range.r0 && row <= t.range.r1 && col >= t.range.c0 && col <= t.range.c1,
    ) ?? null
  );
}

/** Translated gallery name of a style, e.g. "Vừa · Lam" for `TableStyleMedium2`. */
export function styleLabel(t: T, name: string | null): string {
  if (!name) return t('tableDesign.styleNone');
  const m = /^TableStyle(Light|Medium|Dark)(\d+)$/.exec(name);
  if (!m) return name;
  const family = t(`tableDesign.family${m[1]}`);
  const accent = t(`tableDesign.accent${((Number(m[2]) - 1) % 7) + 1}`);
  return t('tableDesign.styleName', { family, accent });
}

/** A range `A1:B2` of the current sheet; `null` when invalid. */
export function parseTableRange(text: string): SheetRange | null {
  const sel = parseA1(text.trim().replace(/\$/g, ''));
  return sel ? rangeOf(sel) : null;
}

type Add<S> = (
  id: string,
  run: (s: S) => Promise<unknown>,
  extra?: Partial<{
    isEnabled: (s: S) => boolean;
    isChecked: (s: S) => boolean;
  }>,
) => void;

/** What the table commands read from the editor state. */
export interface TableState {
  sheet: number;
  hasWorkbook: boolean;
  /** Table of the active cell. */
  table: TableInfo | null;
  /** Automatic behaviour options (from `tables`). */
  tableAuto: { autoExpand: boolean; autoCalc: boolean } | null;
  selection: { active: { row: number; col: number } };
}

/** Registers the Table Design commands (`tableDesign.*`). */
export function registerTableCommands<S extends TableState>(
  add: Add<S>,
  deps: {
    exec: (cmd: SheetCommand) => Promise<unknown>;
    /** Localized label of a new totals row. */
    totalLabel: () => string;
    openRename: () => void;
    openResize: () => void;
    openTotals: () => void;
    /** Remove Duplicates over the table range. */
    openDedupe: (range: SheetRange) => void;
  },
): void {
  const on = (s: S) => s.hasWorkbook && s.table !== null;
  const update =
    (patch: (t: TableInfo) => Record<string, unknown>) =>
    async (s: S): Promise<unknown> => {
      const t = s.table;
      if (!t) return null;
      return deps.exec({
        cmd: 'tableUpdate',
        sheet: s.sheet,
        table: t.index,
        ...patch(t),
      } as SheetCommand);
    };
  add('tableDesign.rename', async () => deps.openRename(), { isEnabled: on });
  add('tableDesign.resize', async () => deps.openResize(), { isEnabled: on });
  add(
    'tableDesign.convert',
    async (s) => {
      if (s.table) await deps.exec({ cmd: 'tableConvert', sheet: s.sheet, table: s.table.index });
    },
    { isEnabled: on },
  );
  add(
    'tableDesign.removeDuplicates',
    async (s) => {
      if (s.table) deps.openDedupe(s.table.range);
    },
    { isEnabled: on },
  );
  add(
    'tableDesign.slicer',
    (s) =>
      deps.exec({
        cmd: 'insertSlicer',
        sheet: s.sheet,
        row: s.selection.active.row,
        col: s.selection.active.col,
      }),
    { isEnabled: (s) => on(s) && !!s.table?.headerRow },
  );
  add('tableDesign.totals', async () => deps.openTotals(), {
    isEnabled: (s) => on(s) && !!s.table?.totalsRow,
  });
  const flags = [
    ['headerRow', 'headerRow'],
    ['totalsRow', 'totalsRow'],
    ['bandedRows', 'bandedRows'],
    ['bandedColumns', 'bandedColumns'],
    ['firstColumn', 'firstColumn'],
    ['lastColumn', 'lastColumn'],
    ['filterButton', 'filterButton'],
  ] as const;
  for (const [id, key] of flags) {
    add(
      `tableDesign.${id}`,
      update((t) => ({
        [key]: !t[key],
        ...(key === 'totalsRow' && !t.totalsRow ? { totalsLabel: deps.totalLabel() } : {}),
      })),
      {
        isEnabled: (s) => on(s) && (key !== 'filterButton' || !!s.table?.headerRow),
        isChecked: (s) => s.table?.[key] === true,
      },
    );
  }
  add(
    'tableDesign.styleNone',
    update(() => ({ style: '' })),
    {
      isEnabled: on,
      isChecked: (s) => s.table !== null && s.table.style === null,
    },
  );
  for (const name of TABLE_STYLE_NAMES) {
    add(
      `tableDesign.style.${name}`,
      update(() => ({ style: name })),
      {
        isEnabled: on,
        isChecked: (s) => s.table?.style === name,
      },
    );
  }
  add(
    'tableDesign.autoExpand',
    (s) => deps.exec({ cmd: 'tableOptions', autoExpand: !(s.tableAuto?.autoExpand ?? true) }),
    { isEnabled: (s) => s.hasWorkbook, isChecked: (s) => s.tableAuto?.autoExpand ?? true },
  );
  add(
    'tableDesign.autoCalc',
    (s) => deps.exec({ cmd: 'tableOptions', autoCalc: !(s.tableAuto?.autoCalc ?? true) }),
    { isEnabled: (s) => s.hasWorkbook, isChecked: (s) => s.tableAuto?.autoCalc ?? true },
  );
}

type Item = (id: string, label: string, icon: ReactNode, size?: 'large' | 'small') => RibbonItem;

/** The style gallery: a dropdown of swatches, one per style. */
export function TableStyleGallery({
  load,
  current,
  run,
}: {
  /** Loads the gallery (`tableStyles`). */
  load: () => Promise<TableStyleEntry[]>;
  current: string | null;
  /** Runs a command id. */
  run: (id: string) => void;
}) {
  const { t } = useTranslation();
  const [entries, setEntries] = useState<TableStyleEntry[] | null>(null);
  useEffect(() => {
    let live = true;
    load()
      .then((e) => {
        if (live) setEntries(Array.isArray(e) ? e : []);
      })
      .catch(() => {
        if (live) setEntries([]);
      });
    return () => {
      live = false;
    };
  }, [load]);
  return (
    <Popover
      trigger={
        <span className={tstyles.trigger}>
          <Paintbrush size={18} aria-hidden="true" />
          <span>{t('tableDesign.styles')}</span>
        </span>
      }
      label={t('tableDesign.styles')}
      testId="table-styles"
    >
      {(close) => (
        <div className={tstyles.gallery} data-testid="table-style-gallery">
          <button
            type="button"
            className={current === null ? tstyles.swatchOn : tstyles.swatch}
            title={t('tableDesign.styleNone')}
            aria-label={t('tableDesign.styleNone')}
            data-testid="table-style-none"
            onClick={() => {
              close();
              run('tableDesign.styleNone');
            }}
          >
            <span className={tstyles.none}>∅</span>
          </button>
          {(entries ?? []).map((e) => {
            const label = styleLabel(t, e.name);
            return (
              <button
                key={e.name}
                type="button"
                className={current === e.name ? tstyles.swatchOn : tstyles.swatch}
                title={label}
                aria-label={label}
                data-testid={`table-style-${e.name}`}
                onClick={() => {
                  close();
                  run(`tableDesign.style.${e.name}`);
                }}
              >
                <Swatch entry={e} />
              </button>
            );
          })}
          {entries === null && <p className={styles.hint}>{t('tableDesign.loading')}</p>}
        </div>
      )}
    </Popover>
  );
}

/** A small picture of a style: header row, two banded rows, totals row. */
function Swatch({ entry }: { entry: TableStyleEntry }) {
  const hex = (c: string | null) => (c ? `#${c}` : 'transparent');
  return (
    <span className={tstyles.preview} aria-hidden="true">
      <span style={{ background: hex(entry.header) }} />
      <span style={{ background: hex(entry.stripe) }} />
      <span />
      <span style={{ background: hex(entry.stripe) }} />
      <span style={{ background: hex(entry.totals) }} />
    </span>
  );
}

/** Groups of the Table Design contextual tab. */
export function tableDesignTabGroups(
  t: T,
  item: Item,
  deps: {
    table: TableInfo;
    loadStyles: () => Promise<TableStyleEntry[]>;
    run: (id: string) => void;
  },
): RibbonGroup[] {
  const s = 16;
  const check = (id: string, key: string): RibbonItem => item(id, t(key), null, 'small');
  return [
    {
      id: 'tableProps',
      label: t('tableDesign.groupProperties'),
      items: [
        {
          id: 'tableDesign.name',
          label: t('tableDesign.name'),
          onSelect: () => deps.run('tableDesign.rename'),
          render: (
            <span className={tstyles.name} data-testid="table-name">
              {deps.table.name}
            </span>
          ),
        },
        item('tableDesign.rename', t('tableDesign.rename'), <PencilLine size={s} />, 'small'),
        item('tableDesign.resize', t('tableDesign.resize'), <Scaling size={s} />, 'small'),
      ],
    },
    {
      id: 'tableTools',
      label: t('tableDesign.groupTools'),
      items: [
        item('tableDesign.convert', t('tableDesign.convert'), <Table2 size={s} />, 'small'),
        item(
          'tableDesign.removeDuplicates',
          t('tableDesign.removeDuplicates'),
          <Eraser size={s} />,
          'small',
        ),
        item('tableDesign.slicer', t('tableDesign.slicer'), <ListFilter size={s} />, 'small'),
      ],
    },
    {
      id: 'tableOptions',
      label: t('tableDesign.groupOptions'),
      items: [
        check('tableDesign.headerRow', 'tableDesign.headerRow'),
        check('tableDesign.totalsRow', 'tableDesign.totalsRow'),
        check('tableDesign.bandedRows', 'tableDesign.bandedRows'),
        check('tableDesign.firstColumn', 'tableDesign.firstColumn'),
        check('tableDesign.lastColumn', 'tableDesign.lastColumn'),
        check('tableDesign.bandedColumns', 'tableDesign.bandedColumns'),
        check('tableDesign.filterButton', 'tableDesign.filterButton'),
      ],
    },
    {
      id: 'tableStyles',
      label: t('tableDesign.groupStyles'),
      items: [
        {
          id: 'tableDesign.styles',
          label: t('tableDesign.styles'),
          onSelect: () => undefined,
          render: (
            <TableStyleGallery load={deps.loadStyles} current={deps.table.style} run={deps.run} />
          ),
        },
        item('tableDesign.totals', t('tableDesign.totals'), <Sigma size={18} />),
      ],
    },
    {
      id: 'tableAuto',
      label: t('tableDesign.groupAuto'),
      items: [
        check('tableDesign.autoExpand', 'tableDesign.autoExpand'),
        check('tableDesign.autoCalc', 'tableDesign.autoCalc'),
      ],
    },
  ];
}

/** Table Design › Rename. */
export function TableNameDialog({
  sheet,
  table,
  exec,
  onClose,
}: {
  sheet: number;
  table: TableInfo;
  /** Runs a command; resolves to `null` when it failed. */
  exec: (cmd: SheetCommand) => Promise<unknown>;
  onClose: () => void;
}) {
  const { t } = useTranslation();
  const [name, setName] = useState(table.name);
  const [error, setError] = useState<string | null>(null);
  const ok = async () => {
    if (!/^[\p{L}_\\][\p{L}\p{N}_.]*$/u.test(name.trim())) {
      setError(t('tableDesign.invalidName'));
      return;
    }
    const r = await exec({ cmd: 'tableUpdate', sheet, table: table.index, name: name.trim() });
    if (r !== null) onClose();
    else setError(t('tableDesign.invalidName'));
  };
  return (
    <Dialog
      open
      onOpenChange={(o) => !o && onClose()}
      title={t('tableDesign.renameTitle')}
      testId="table-rename"
      width={360}
      footer={<Footer t={t} onCancel={onClose} onOk={() => void ok()} testId="table-rename-ok" />}
    >
      <div className={styles.form}>
        <label>
          {t('tableDesign.name')}
          <input
            data-testid="table-rename-input"
            value={name}
            onChange={(e) => setName(e.target.value)}
          />
        </label>
        <p className={styles.hint}>{t('tableDesign.nameHint')}</p>
        {error && (
          <p className={styles.error} role="alert">
            {error}
          </p>
        )}
      </div>
    </Dialog>
  );
}

/** Table Design › Resize. */
export function TableResizeDialog({
  sheet,
  table,
  exec,
  onClose,
}: {
  sheet: number;
  table: TableInfo;
  exec: (cmd: SheetCommand) => Promise<unknown>;
  onClose: () => void;
}) {
  const { t } = useTranslation();
  const [text, setText] = useState(rangeA1(table.range as Range));
  const [error, setError] = useState<string | null>(null);
  const ok = async () => {
    const r = parseTableRange(text);
    if (!r || r.r0 !== table.range.r0) {
      setError(t('tableDesign.resizeInvalid'));
      return;
    }
    const res = await exec({ cmd: 'tableResize', sheet, table: table.index, range: r });
    if (res !== null) onClose();
    else setError(t('tableDesign.resizeInvalid'));
  };
  return (
    <Dialog
      open
      onOpenChange={(o) => !o && onClose()}
      title={t('tableDesign.resizeTitle')}
      testId="table-resize"
      width={360}
      footer={<Footer t={t} onCancel={onClose} onOk={() => void ok()} testId="table-resize-ok" />}
    >
      <div className={styles.form}>
        <label>
          {t('tableDesign.resizeRange')}
          <input
            data-testid="table-resize-input"
            value={text}
            onChange={(e) => setText(e.target.value)}
          />
        </label>
        <p className={styles.hint}>{t('tableDesign.resizeHint')}</p>
        {error && (
          <p className={styles.error} role="alert">
            {error}
          </p>
        )}
      </div>
    </Dialog>
  );
}

/** The commands that apply the totals chosen in the dialog (unchanged columns left out). */
export function totalsCommands(
  sheet: number,
  table: TableInfo,
  choice: { function: TableTotalFunction; formula: string }[],
): SheetCommand[] {
  const out: SheetCommand[] = [];
  table.columns.forEach((c, i) => {
    const ch = choice[i];
    if (!ch) return;
    const formula = ch.formula.trim().replace(/^=/, '');
    const same =
      ch.function === c.totalsFunction &&
      (ch.function !== 'custom' || formula === (c.totalsFormula ?? ''));
    if (same) return;
    out.push({
      cmd: 'tableTotal',
      sheet,
      table: table.index,
      column: i,
      function: ch.function,
      ...(ch.function === 'custom' ? { formula } : {}),
    });
  });
  return out;
}

/** Table Design › Totals: the function of each column's total. */
export function TableTotalsDialog({
  sheet,
  table,
  exec,
  onClose,
}: {
  sheet: number;
  table: TableInfo;
  exec: (cmd: SheetCommand) => Promise<unknown>;
  onClose: () => void;
}) {
  const { t } = useTranslation();
  const [choice, setChoice] = useState(
    table.columns.map((c) => ({ function: c.totalsFunction, formula: c.totalsFormula ?? '' })),
  );
  const [error, setError] = useState<string | null>(null);
  const ok = async () => {
    if (choice.some((c) => c.function === 'custom' && !c.formula.trim())) {
      setError(t('tableDesign.formulaMissing'));
      return;
    }
    const cmds = totalsCommands(sheet, table, choice);
    if (cmds.length === 0) {
      onClose();
      return;
    }
    const r = await exec(cmds.length === 1 ? cmds[0]! : { cmd: 'batch', cmds });
    if (r !== null) onClose();
    else setError(t('tableDesign.formulaInvalid'));
  };
  return (
    <Dialog
      open
      onOpenChange={(o) => !o && onClose()}
      title={t('tableDesign.totalsTitle')}
      testId="table-totals"
      width={460}
      footer={<Footer t={t} onCancel={onClose} onOk={() => void ok()} testId="table-totals-ok" />}
    >
      <div className={tstyles.totals}>
        {table.columns.map((c, i) => (
          <div key={c.name} className={tstyles.totalRow}>
            <span className={tstyles.colName}>{c.name}</span>
            <select
              aria-label={t('tableDesign.totalFor', { name: c.name })}
              data-testid={`table-total-${i}`}
              value={choice[i]?.function ?? 'none'}
              onChange={(e) => {
                const f = e.target.value as TableTotalFunction;
                setChoice((p) => p.map((x, j) => (j === i ? { ...x, function: f } : x)));
              }}
            >
              {TOTAL_FUNCTIONS.map((f) => (
                <option key={f} value={f}>
                  {t(`tableDesign.fn.${f}`)}
                </option>
              ))}
            </select>
            {choice[i]?.function === 'custom' && (
              <input
                aria-label={t('tableDesign.customFormula')}
                data-testid={`table-total-formula-${i}`}
                value={choice[i]?.formula ?? ''}
                placeholder="=SUM(…)"
                onChange={(e) => {
                  const v = e.target.value;
                  setChoice((p) => p.map((x, j) => (j === i ? { ...x, formula: v } : x)));
                }}
              />
            )}
          </div>
        ))}
        <p className={styles.hint}>{t('tableDesign.totalsHint')}</p>
        {error && (
          <p className={styles.error} role="alert">
            {error}
          </p>
        )}
      </div>
    </Dialog>
  );
}

function Footer({
  t,
  onCancel,
  onOk,
  testId,
}: {
  t: T;
  onCancel: () => void;
  onOk: () => void;
  testId: string;
}) {
  return (
    <>
      <button type="button" className={styles.btn} onClick={onCancel}>
        {t('tableDesign.cancel')}
      </button>
      <button type="button" className={styles.btnPrimary} data-testid={testId} onClick={onOk}>
        {t('tableDesign.ok')}
      </button>
    </>
  );
}

/**
 * The bar shown after typing expanded a table or filled a calculated column: undo just that
 * (it is its own undo step) or stop doing it automatically.
 */
export function TableAutoBar({
  last,
  run,
}: {
  last: TableList['lastAuto'];
  /** Runs a command id (`edit.undo`, `tableDesign.autoExpand`, `tableDesign.autoCalc`). */
  run: (id: string) => void;
}) {
  const { t } = useTranslation();
  const [hidden, setHidden] = useState<number | null>(null);
  if (!last || hidden === last.seq) return null;
  const expand = last.kind === 'expand';
  return (
    <div className={tstyles.autoBar} role="status" data-testid="table-auto-bar">
      <Columns3 size={14} aria-hidden="true" />
      <span>{t(expand ? 'tableDesign.autoExpanded' : 'tableDesign.autoFilled')}</span>
      <button
        type="button"
        className={styles.btn}
        data-testid="table-auto-undo"
        onClick={() => {
          setHidden(last.seq);
          run('edit.undo');
        }}
      >
        {t(expand ? 'tableDesign.undoExpand' : 'tableDesign.undoFill')}
      </button>
      <button
        type="button"
        className={styles.btn}
        data-testid="table-auto-stop"
        onClick={() => {
          setHidden(last.seq);
          run(expand ? 'tableDesign.autoExpand' : 'tableDesign.autoCalc');
        }}
      >
        {t(expand ? 'tableDesign.stopExpand' : 'tableDesign.stopFill')}
      </button>
      <button
        type="button"
        className={tstyles.close}
        aria-label={t('tableDesign.dismiss')}
        onClick={() => setHidden(last.seq)}
      >
        ×
      </button>
    </div>
  );
}
