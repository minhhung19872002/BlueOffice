/**
 * BlueSheet what-if scenarios (ECMA-376 Part 1, §18.3.1 `scenarios`): a list of named
 * input sets for the sheet with add / edit / delete, apply (enter the values, one undo
 * step), copy from another sheet and a report on a new sheet. Every action goes through
 * the engine command bridge (`SheetHost.exec`, `scenario*` commands).
 */
import { useTranslation } from '@blueoffice/i18n';
import type {
  ScenarioInfo,
  ScenarioList,
  ScenarioSummaryLabels,
  SheetHost,
} from '@blueoffice/platform';
import { Dialog } from '@blueoffice/ui';
import { useCallback, useEffect, useState } from 'react';
import { a1, parseA1, rangeOf } from './grid';
import styles from './SheetEditor.module.css';

/** Most input cells one scenario may have (engine limit). */
const MAX_CELLS = 32;

/** Expand `B1:B3, D5` into its cells (A1); `null` when an address is invalid or too many. */
export function expandCells(text: string): string[] | null {
  const out: string[] = [];
  const parts = text.split(/[,;\s]+/).filter((p) => p !== '');
  for (const p of parts) {
    const sel = parseA1(p.includes('!') ? p.slice(p.lastIndexOf('!') + 1) : p);
    if (!sel) return null;
    const r = rangeOf(sel);
    for (let row = r.r0; row <= r.r1; row++) {
      for (let col = r.c0; col <= r.c1; col++) {
        const c = a1({ row, col });
        if (!out.includes(c)) out.push(c);
        if (out.length > MAX_CELLS) return null;
      }
    }
  }
  return out;
}

interface Draft {
  index: number | null;
  name: string;
  cells: string;
  values: Record<string, string>;
  comment: string;
  locked: boolean;
  hidden: boolean;
}

export function ScenarioDialog({
  host,
  sheet,
  sheets,
  selection,
  author,
  onClose,
  onDone,
  onReport,
  fail,
}: {
  host: SheetHost;
  sheet: number;
  /** Names of the workbook's sheets (to copy scenarios from). */
  sheets: string[];
  /** The selection (A1), proposed as input cells of a new scenario. */
  selection: string;
  /** Recorded as the scenario's author. */
  author: string;
  onClose: () => void;
  /** Called after the workbook changed. */
  onDone: () => void | Promise<void>;
  /** A report sheet was added at this index. */
  onReport: (sheet: number) => void;
  fail: (e: unknown) => void;
}) {
  const { t } = useTranslation();
  const [list, setList] = useState<ScenarioList | null>(null);
  const [picked, setPicked] = useState<number | null>(null);
  const [draft, setDraft] = useState<Draft | null>(null);
  const [results, setResults] = useState('');
  const [from, setFrom] = useState<number>(-1);
  const [error, setError] = useState('');
  const [status, setStatus] = useState('');

  const show = useCallback((l: ScenarioList) => {
    setList(l);
    setResults((r) => (r === '' ? l.results : r));
    setPicked((p) => {
      if (p !== null && l.items.some((i) => i.index === p)) return p;
      return l.items[0]?.index ?? null;
    });
  }, []);

  const reload = async () => show(await host.exec<ScenarioList>({ cmd: 'scenarios', sheet }));

  useEffect(() => {
    let live = true;
    host
      .exec<ScenarioList>({ cmd: 'scenarios', sheet })
      .then((l) => live && show(l))
      .catch(fail);
    return () => {
      live = false;
    };
  }, [host, sheet, show, fail]);

  const current = list?.items.find((i) => i.index === picked) ?? null;
  const locked = (s: ScenarioInfo | null) => !!list?.protected && !!s?.locked;

  /** Values of `cells` as the draft should show them: kept ones, else the cell's input. */
  const fillValues = async (cells: string[], keep: Record<string, string>) => {
    const out: Record<string, string> = {};
    for (const c of cells) {
      if (c in keep) {
        out[c] = keep[c]!;
        continue;
      }
      const sel = parseA1(c);
      if (!sel) continue;
      const d = await host.cell(sheet, sel.active.row, sel.active.col);
      out[c] = d.input.startsWith('=') ? d.display : d.input;
    }
    return out;
  };

  const startNew = async () => {
    setError('');
    const cells = expandCells(selection) ?? [];
    setDraft({
      index: null,
      name: '',
      cells: selection,
      values: await fillValues(cells, {}),
      comment: '',
      locked: true,
      hidden: false,
    });
  };

  const startEdit = (s: ScenarioInfo) => {
    setError('');
    const values: Record<string, string> = {};
    s.cells.forEach((c, i) => (values[c] = s.values[i] ?? ''));
    setDraft({
      index: s.index,
      name: s.name,
      cells: s.cells.join(', '),
      values,
      comment: s.comment ?? '',
      locked: s.locked,
      hidden: s.hidden,
    });
  };

  const draftCells = draft ? expandCells(draft.cells) : null;

  const changeCells = async (text: string) => {
    if (!draft) return;
    const next = { ...draft, cells: text };
    setDraft(next);
    const cells = expandCells(text);
    if (cells) {
      const values = await fillValues(cells, draft.values);
      setDraft((d) => (d && d.cells === text ? { ...d, values } : d));
    }
  };

  const save = async () => {
    if (!draft) return;
    if (draft.name.trim() === '') {
      setError(t('sheetEditor.scenario.needName'));
      return;
    }
    const cells = expandCells(draft.cells);
    if (!cells || cells.length === 0) {
      setError(t('sheetEditor.scenario.badCells'));
      return;
    }
    const taken = list?.items.some(
      (i) =>
        i.index !== draft.index && i.name.trim().toLowerCase() === draft.name.trim().toLowerCase(),
    );
    if (taken) {
      setError(t('sheetEditor.scenario.nameTaken'));
      return;
    }
    try {
      const r = await host.exec<{ index: number }>({
        cmd: 'scenarioSave',
        sheet,
        index: draft.index,
        name: draft.name.trim(),
        cells: cells.join(','),
        values: cells.map((c) => draft.values[c] ?? ''),
        comment: draft.comment || null,
        user: author || null,
        locked: draft.locked,
        hidden: draft.hidden,
      });
      setDraft(null);
      setError('');
      setPicked(r.index);
      await reload();
      await onDone();
    } catch {
      setError(t('sheetEditor.scenario.refused'));
    }
  };

  const run = async (f: () => Promise<void>) => {
    try {
      setError('');
      await f();
    } catch {
      setError(t('sheetEditor.scenario.refused'));
    }
  };

  const apply = () =>
    run(async () => {
      if (picked === null) return;
      await host.exec({ cmd: 'scenarioShow', sheet, index: picked });
      await reload();
      await onDone();
      setStatus(t('sheetEditor.scenario.applied', { name: current?.name ?? '' }));
    });

  const remove = () =>
    run(async () => {
      if (picked === null) return;
      await host.exec({ cmd: 'scenarioDelete', sheet, index: picked });
      setPicked(null);
      await reload();
      await onDone();
    });

  const merge = () =>
    run(async () => {
      if (from < 0) return;
      const r = await host.exec<{ merged: number }>({ cmd: 'scenarioMerge', sheet, from });
      await reload();
      await onDone();
      setStatus(t('sheetEditor.scenario.merged', { count: r.merged }));
    });

  const report = () =>
    run(async () => {
      if (results.trim() !== '' && expandCells(results) === null) {
        setError(t('sheetEditor.scenario.badCells'));
        return;
      }
      const labels: ScenarioSummaryLabels = {
        sheet: t('sheetEditor.scenario.report.sheet'),
        title: t('sheetEditor.scenario.report.title', { sheet: '{sheet}' }),
        current: t('sheetEditor.scenario.report.current'),
        changing: t('sheetEditor.scenario.report.inputs'),
        results: t('sheetEditor.scenario.report.outcomes'),
        comment: t('sheetEditor.scenario.report.notes'),
        note: t('sheetEditor.scenario.report.note'),
      };
      const r = await host.exec<{ sheet: number }>({
        cmd: 'scenarioSummary',
        sheet,
        results: results.trim(),
        labels,
      });
      await onDone();
      onReport(r.sheet);
      onClose();
    });

  if (draft) {
    return (
      <Dialog
        open
        onOpenChange={(o) => !o && setDraft(null)}
        title={t(
          draft.index === null ? 'sheetEditor.scenario.addTitle' : 'sheetEditor.scenario.editTitle',
        )}
        testId="scenario-edit"
        width={460}
        footer={
          <>
            <button type="button" className={styles.btn} onClick={() => setDraft(null)}>
              {t('sheetEditor.dlg.cancel')}
            </button>
            <button
              type="button"
              className={styles.btnPrimary}
              data-testid="scenario-save"
              onClick={() => void save()}
            >
              {t('sheetEditor.scenario.save')}
            </button>
          </>
        }
      >
        <div className={styles.form}>
          <label>
            {t('sheetEditor.scenario.name')}
            <input
              data-testid="scenario-name"
              value={draft.name}
              maxLength={255}
              onChange={(e) => setDraft({ ...draft, name: e.target.value })}
            />
          </label>
          <label>
            {t('sheetEditor.scenario.cells')}
            <input
              data-testid="scenario-cells"
              value={draft.cells}
              onChange={(e) => void changeCells(e.target.value)}
            />
          </label>
          <p className={styles.hint}>{t('sheetEditor.scenario.cellsHint', { max: MAX_CELLS })}</p>
          {draftCells?.map((c) => (
            <label key={c}>
              {c}
              <input
                data-testid={`scenario-value-${c}`}
                value={draft.values[c] ?? ''}
                onChange={(e) =>
                  setDraft({ ...draft, values: { ...draft.values, [c]: e.target.value } })
                }
              />
            </label>
          ))}
          <label>
            {t('sheetEditor.scenario.comment')}
            <input
              data-testid="scenario-comment"
              value={draft.comment}
              onChange={(e) => setDraft({ ...draft, comment: e.target.value })}
            />
          </label>
          <label className={styles.check}>
            <input
              type="checkbox"
              data-testid="scenario-locked"
              checked={draft.locked}
              onChange={(e) => setDraft({ ...draft, locked: e.target.checked })}
            />
            {t('sheetEditor.scenario.locked')}
          </label>
          <label className={styles.check}>
            <input
              type="checkbox"
              data-testid="scenario-hidden"
              checked={draft.hidden}
              onChange={(e) => setDraft({ ...draft, hidden: e.target.checked })}
            />
            {t('sheetEditor.scenario.hidden')}
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

  const others = sheets.map((name, i) => ({ name, i })).filter((s) => s.i !== sheet);
  return (
    <Dialog
      open
      onOpenChange={(o) => !o && onClose()}
      title={t('sheetEditor.scenario.title')}
      testId="scenario-dialog"
      width={500}
      footer={
        <button type="button" className={styles.btn} onClick={onClose}>
          {t('sheetEditor.scenario.close')}
        </button>
      }
    >
      <div className={styles.form}>
        {list && list.items.length === 0 && (
          <p className={styles.hint} data-testid="scenario-empty">
            {t('sheetEditor.scenario.empty')}
          </p>
        )}
        {list && list.items.length > 0 && (
          <div
            role="listbox"
            aria-label={t('sheetEditor.scenario.title')}
            className={styles.valueList}
          >
            {list.items.map((s) => (
              <button
                key={s.index}
                type="button"
                role="option"
                aria-selected={s.index === picked}
                data-testid={`scenario-item-${s.index}`}
                className={s.index === picked ? styles.btnPrimary : styles.btn}
                onClick={() => setPicked(s.index)}
                onDoubleClick={() => void apply()}
              >
                {s.name}
                {list.current === s.index ? ` · ${t('sheetEditor.scenario.active')}` : ''}
              </button>
            ))}
          </div>
        )}
        {current && (
          <p className={styles.hint} data-testid="scenario-details">
            {t('sheetEditor.scenario.details', {
              cells: current.cells.join(', '),
              user: current.user ?? '—',
            })}
            {current.comment ? ` — ${current.comment}` : ''}
          </p>
        )}
        <div className={styles.row}>
          <button
            type="button"
            className={styles.btn}
            data-testid="scenario-add"
            disabled={!!list?.protected}
            onClick={() => void startNew().catch(fail)}
          >
            {t('sheetEditor.scenario.add')}
          </button>
          <button
            type="button"
            className={styles.btn}
            data-testid="scenario-edit-btn"
            disabled={!current || locked(current)}
            onClick={() => current && startEdit(current)}
          >
            {t('sheetEditor.scenario.edit')}
          </button>
          <button
            type="button"
            className={styles.btn}
            data-testid="scenario-delete"
            disabled={!current || locked(current)}
            onClick={() => void remove()}
          >
            {t('sheetEditor.scenario.delete')}
          </button>
          <button
            type="button"
            className={styles.btnPrimary}
            data-testid="scenario-apply"
            disabled={!current}
            onClick={() => void apply()}
          >
            {t('sheetEditor.scenario.apply')}
          </button>
        </div>
        {others.length > 0 && (
          <div className={styles.row}>
            <select
              data-testid="scenario-from"
              aria-label={t('sheetEditor.scenario.fromSheet')}
              value={from}
              disabled={!!list?.protected}
              onChange={(e) => setFrom(Number(e.target.value))}
            >
              <option value={-1}>{t('sheetEditor.scenario.fromSheet')}</option>
              {others.map((s) => (
                <option key={s.i} value={s.i}>
                  {s.name}
                </option>
              ))}
            </select>
            <button
              type="button"
              className={styles.btn}
              data-testid="scenario-merge"
              disabled={from < 0 || !!list?.protected}
              onClick={() => void merge()}
            >
              {t('sheetEditor.scenario.merge')}
            </button>
          </div>
        )}
        <label>
          {t('sheetEditor.scenario.results')}
          <input
            data-testid="scenario-results"
            value={results}
            placeholder="B10, C12:C14"
            onChange={(e) => setResults(e.target.value)}
          />
        </label>
        <div className={styles.row}>
          <button
            type="button"
            className={styles.btn}
            data-testid="scenario-report"
            disabled={!list || list.items.length === 0}
            onClick={() => void report()}
          >
            {t('sheetEditor.scenario.report.make')}
          </button>
        </div>
        {status && (
          <p className={styles.hint} role="status">
            {status}
          </p>
        )}
        {error && (
          <p className={styles.error} role="alert">
            {error}
          </p>
        )}
      </div>
    </Dialog>
  );
}
