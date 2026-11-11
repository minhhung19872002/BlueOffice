/**
 * Auto total of BlueSheet (Σ, Alt+=). The engine works out where a total goes and what it adds
 * up (`autoSum`, read only: `bo_engine::sheet::autosum_cmd`); this module turns that plan into
 * the normal sheet commands — one `batch`, so one undo step that also travels through a
 * collaboration session — and holds the "More functions" list.
 */
import { useTranslation } from '@blueoffice/i18n';
import type { AutoSumFunction, AutoSumPlan, SheetCommand } from '@blueoffice/platform';
import { Dialog } from '@blueoffice/ui';
import { useEffect, useMemo, useState } from 'react';
import css from './autoSum.module.css';

/** The functions of the Σ menu, in its order. */
export const AUTO_SUM_FUNCTIONS: readonly AutoSumFunction[] = [
  'sum',
  'average',
  'count',
  'max',
  'min',
];

/** Command id of each function (`formula.autoSum` is the Σ button and Alt+=). */
export const AUTO_SUM_COMMANDS: Record<AutoSumFunction, string> = {
  sum: 'formula.autoSum',
  average: 'formula.autoAverage',
  count: 'formula.autoCount',
  max: 'formula.autoMax',
  min: 'formula.autoMin',
};

/**
 * What the auto total commands need from the editor at the time they run: the commands are
 * registered once, the editor attaches its latest values from an effect.
 */
export class AutoSumLink<S> {
  private pick: ((s: S) => void) | null = null;
  private edit: ProposeFormula | null = null;
  private busy = false;

  /**
   * The latest selection setter, the function opening the cell editor on a proposed formula,
   * and whether a cell is being edited (call from an effect).
   */
  attach(select: (s: S) => void, propose: ProposeFormula, editing: boolean): void {
    this.pick = select;
    this.edit = propose;
    this.busy = editing;
  }

  /** A cell is being edited (an auto total does not start then). */
  editing(): boolean {
    return this.busy;
  }

  select(s: S): void {
    this.pick?.(s);
  }

  propose(...args: Parameters<ProposeFormula>): void {
    this.edit?.(...args);
  }
}

/** Opens the cell editor on `pos` with `text`, `from..to` of it selected. */
export type ProposeFormula = (
  pos: { row: number; col: number },
  text: string,
  from: number,
  to: number,
  numFmt?: string,
) => void;

/**
 * The command that applies a `write` or `table` plan (`null`: nothing to run — the `edit`
 * plan opens the cell editor instead, `none` has nothing to add up).
 */
export function autoSumCommand(
  plan: AutoSumPlan,
  sheet: number,
  /** Label of a totals row shown for the first time. */
  totalLabel: string,
): SheetCommand | null {
  if (plan.mode === 'write') {
    if (!plan.cells.length) return null;
    const cmds: SheetCommand[] = [{ cmd: 'setInputs', sheet, cells: plan.cells }];
    for (const f of plan.formats)
      cmds.push({
        cmd: 'format',
        sheet,
        range: { r0: f.row, c0: f.col, r1: f.row, c1: f.col },
        patch: { numFmt: f.numFmt },
      });
    return { cmd: 'batch', cmds };
  }
  if (plan.mode === 'table') {
    const cmds: SheetCommand[] = [];
    if (plan.show)
      cmds.push({
        cmd: 'tableUpdate',
        sheet,
        table: plan.table,
        totalsRow: true,
        totalsLabel: totalLabel,
      });
    for (const column of plan.clear)
      cmds.push({ cmd: 'tableTotal', sheet, table: plan.table, column, function: 'none' });
    for (const column of plan.columns)
      cmds.push({ cmd: 'tableTotal', sheet, table: plan.table, column, function: plan.function });
    return cmds.length ? { cmd: 'batch', cmds } : null;
  }
  return null;
}

/** Names matching what was typed: those starting with it first, then those containing it. */
export function filterFunctions(names: readonly string[], typed: string): string[] {
  const q = typed.trim().toUpperCase();
  if (!q) return [...names];
  const starts = names.filter((n) => n.startsWith(q));
  const inside = names.filter((n) => !n.startsWith(q) && n.includes(q));
  return [...starts, ...inside];
}

/** "More functions…": a searchable list of the worksheet functions. */
export function InsertFunctionDialog({
  query,
  onInsert,
  onClose,
}: {
  /** Runs a read-only command. */
  query: <T>(cmd: SheetCommand) => Promise<T | null>;
  /** Start a formula with the chosen function in the active cell. */
  onInsert: (name: string) => void;
  onClose: () => void;
}) {
  const { t } = useTranslation();
  const [names, setNames] = useState<string[] | null>(null);
  const [typed, setTyped] = useState('');
  const [picked, setPicked] = useState<string | null>(null);
  useEffect(() => {
    let live = true;
    void query<{ names: string[] }>({ cmd: 'functionList' }).then((r) => {
      if (live) setNames(r?.names ?? []);
    });
    return () => {
      live = false;
    };
  }, [query]);
  const shown = useMemo(() => filterFunctions(names ?? [], typed), [names, typed]);
  const current = picked && shown.includes(picked) ? picked : (shown[0] ?? null);
  const insert = (name: string | null) => {
    if (!name) return;
    onInsert(name);
    onClose();
  };
  return (
    <Dialog
      open
      onOpenChange={(o) => !o && onClose()}
      title={t('autoSum.insertFunction.title')}
      testId="insert-function-dialog"
      width={360}
      footer={
        <>
          <button type="button" onClick={onClose}>
            {t('autoSum.insertFunction.cancel')}
          </button>
          <button
            type="button"
            data-testid="insert-function-ok"
            disabled={!current}
            onClick={() => insert(current)}
          >
            {t('autoSum.insertFunction.insert')}
          </button>
        </>
      }
    >
      <input
        className={css.search}
        autoFocus
        value={typed}
        aria-label={t('autoSum.insertFunction.search')}
        placeholder={t('autoSum.insertFunction.search')}
        data-testid="insert-function-search"
        onChange={(e) => setTyped(e.currentTarget.value)}
        onKeyDown={(e) => {
          if (e.key === 'Enter') {
            e.preventDefault();
            insert(current);
          } else if (e.key === 'ArrowDown' || e.key === 'ArrowUp') {
            e.preventDefault();
            const i = current ? shown.indexOf(current) : -1;
            const next =
              shown[Math.max(0, Math.min(shown.length - 1, i + (e.key === 'ArrowDown' ? 1 : -1)))];
            if (next) setPicked(next);
          }
        }}
      />
      <div
        className={css.list}
        role="listbox"
        aria-label={t('autoSum.insertFunction.title')}
        data-testid="insert-function-list"
      >
        {names === null ? (
          <p className={css.empty}>{t('autoSum.insertFunction.loading')}</p>
        ) : shown.length === 0 ? (
          <p className={css.empty}>{t('autoSum.insertFunction.none')}</p>
        ) : (
          shown.map((n) => (
            <button
              key={n}
              type="button"
              role="option"
              aria-selected={n === current}
              className={n === current ? css.optionActive : css.option}
              data-function={n}
              onClick={() => setPicked(n)}
              onDoubleClick={() => insert(n)}
            >
              {n}
            </button>
          ))
        )}
      </div>
    </Dialog>
  );
}
