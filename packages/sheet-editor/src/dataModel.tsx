/**
 * The workbook data model in BlueSheet (SHEET-DM-001): Data ▸ Data model — the tables of the
 * model, the relationships dialog (a list without a diagram: add, edit, delete, cardinality
 * detection), the measures and calculated columns dialog (formula with suggestions of tables,
 * columns, measures and functions; syntax errors shown while typing) and "pivot table from
 * model". The engine does the work (`bo_engine::sheet::model_cmd`).
 */
import { useTranslation } from '@blueoffice/i18n';
import type {
  ModelCardinality,
  ModelCheckResult,
  ModelInfo,
  ModelRelationshipInfo,
  PivotLabels,
  SheetHost,
} from '@blueoffice/platform';
import { Dialog } from '@blueoffice/ui';
import { Pencil, Plus, Trash2 } from 'lucide-react';
import { useCallback, useEffect, useRef, useState, type ReactNode } from 'react';
import styles from './SheetEditor.module.css';

/** Which data model dialog is open. */
export type ModelDialogKind = 'tables' | 'relationships' | 'measures';

/** A suggestion of the formula box. */
export interface ModelSuggestion {
  label: string;
  /** Text replacing the typed prefix. */
  insert: string;
  kind: 'table' | 'column' | 'measure' | 'function';
}

const IDENT = /^[\p{L}_][\p{L}\p{N}_.]*$/u;

/** A table name as the expression language writes it (quoted unless an identifier). */
export function tableRef(name: string): string {
  return IDENT.test(name) ? name : `'${name.replace(/'/g, "''")}'`;
}

const starts = (s: string, prefix: string) => s.toLowerCase().startsWith(prefix.toLowerCase());

/**
 * Suggestions for the text before `caret`: the columns of `Table[…`, the measures (and, for a
 * calculated column, the columns of its table) after `[`, otherwise functions and tables
 * starting with the word being typed. `from` is where the replaced prefix starts.
 */
export function suggest(
  info: ModelInfo,
  text: string,
  caret: number,
  columnTable?: string,
): { from: number; items: ModelSuggestion[] } {
  const before = text.slice(0, caret);
  const tables = info.tables.filter((t) => t.inModel && !t.missing);
  const col = /(?:'((?:[^']|'')*)'|([\p{L}_][\p{L}\p{N}_.]*))\[([^\]]*)$/u.exec(before);
  if (col) {
    const name = (col[1] ?? col[2] ?? '').replace(/''/g, "'");
    const partial = col[3] ?? '';
    const t = tables.find((x) => x.name.toLowerCase() === name.toLowerCase());
    const items = (t?.columns ?? [])
      .filter((c) => starts(c.name, partial))
      .map((c) => ({ label: c.name, insert: `${c.name}]`, kind: 'column' as const }));
    return { from: caret - partial.length, items };
  }
  const bracket = /\[([^\]]*)$/.exec(before);
  if (bracket) {
    const partial = bracket[1] ?? '';
    const own = columnTable
      ? (tables.find((x) => x.name === columnTable)?.columns ?? []).map((c) => c.name)
      : [];
    const names = [...info.measures.map((m) => m.name), ...own];
    const items = names
      .filter((n) => starts(n, partial))
      .map((n) => ({
        label: n,
        insert: `${n}]`,
        kind: own.includes(n) ? ('column' as const) : ('measure' as const),
      }));
    return { from: caret - partial.length, items };
  }
  const word = /[\p{L}_][\p{L}\p{N}_.]*$/u.exec(before);
  if (!word) return { from: caret, items: [] };
  const w = word[0];
  const fns = info.functions
    .filter((f) => starts(f.name, w))
    .map((f) => ({ label: f.name, insert: `${f.name}(`, kind: 'function' as const }));
  const tbl = tables
    .filter((x) => starts(x.name, w))
    .map((x) => ({ label: x.name, insert: tableRef(x.name), kind: 'table' as const }));
  return { from: caret - w.length, items: [...tbl, ...fns] };
}

/** Text of a check result for the formula box. */
export function checkText(
  r: ModelCheckResult | null,
  t: (k: string, o?: Record<string, unknown>) => string,
): string {
  if (!r) return '';
  if (r.ok) return t('sheetModel.measure.ok');
  const e = r.error;
  if (!e) return '';
  return e.end > e.start
    ? t('sheetModel.measure.errorAt', { pos: e.start + 1, message: e.message })
    : t('sheetModel.measure.error', { message: e.message });
}

/** Options of {@link useDataModel}. */
export interface DataModelOptions {
  host: SheetHost | null | undefined;
  labels: PivotLabels;
  /** Redraw after the workbook changed. */
  onChanged: () => Promise<void>;
  /** A pivot table over the model was created on `sheet`. */
  onPivot: (sheet: number, index: number) => void;
  fail: (e: unknown) => void;
}

/** Data model state of the editor: the dialogs and "pivot table from model". */
export function useDataModel(o: DataModelOptions) {
  const { host, labels, onChanged, onPivot, fail } = o;
  const [open, setOpen] = useState<ModelDialogKind | null>(null);
  const [info, setInfo] = useState<ModelInfo | null>(null);

  const load = useCallback(async () => {
    if (!host) return null;
    const r = await host.exec<ModelInfo>({ cmd: 'modelInfo' });
    setInfo(r);
    return r;
  }, [host]);

  const show = useCallback(
    async (k: ModelDialogKind) => {
      try {
        await load();
        setOpen(k);
      } catch (e) {
        fail(e);
      }
    },
    [load, fail],
  );

  /** Run an editing command; the engine answers with the new model. */
  const edit = useCallback(
    async (cmd: Parameters<SheetHost['exec']>[0]) => {
      if (!host) return;
      const r = await host.exec<ModelInfo>({ ...cmd, labels } as Parameters<SheetHost['exec']>[0]);
      setInfo(r);
      await onChanged();
    },
    [host, labels, onChanged],
  );

  const createPivot = useCallback(async () => {
    if (!host) return;
    try {
      const m = await load();
      if (!m || !m.tables.some((t) => t.inModel)) {
        setOpen('tables');
        return;
      }
      const r = await host.exec<{ sheet: number; index: number }>({
        cmd: 'modelPivotCreate',
        labels,
      });
      await onChanged();
      onPivot(r.sheet, r.index);
    } catch (e) {
      fail(e);
    }
  }, [host, labels, load, onChanged, onPivot, fail]);

  const close = () => setOpen(null);
  let element: ReactNode = null;
  if (host && info && open === 'tables') {
    element = <ModelTablesDialog info={info} edit={edit} onClose={close} fail={fail} />;
  } else if (host && info && open === 'relationships') {
    element = (
      <RelationshipsDialog host={host} info={info} edit={edit} onClose={close} fail={fail} />
    );
  } else if (host && info && open === 'measures') {
    element = <MeasuresDialog host={host} info={info} edit={edit} onClose={close} />;
  }
  return { open: show, createPivot, element, dialog: open };
}

type Edit = (cmd: Parameters<SheetHost['exec']>[0]) => Promise<void>;

function CloseFooter({ onClose }: { onClose: () => void }) {
  const { t } = useTranslation();
  return (
    <button type="button" className={styles.btn} onClick={onClose} data-testid="model-close">
      {t('sheetModel.close')}
    </button>
  );
}

// ----------------------------------------------------------------- tables

export function ModelTablesDialog({
  info,
  edit,
  onClose,
  fail,
}: {
  info: ModelInfo;
  edit: Edit;
  onClose: () => void;
  fail: (e: unknown) => void;
}) {
  const { t } = useTranslation();
  const toggle = (name: string, inModel: boolean) =>
    edit(inModel ? { cmd: 'modelRemoveTable', name } : { cmd: 'modelAddTable', name }).catch(fail);
  return (
    <Dialog
      open
      onOpenChange={(v) => !v && onClose()}
      title={t('sheetModel.tables.title')}
      testId="model-tables-dialog"
      width={520}
      footer={<CloseFooter onClose={onClose} />}
    >
      <div className={styles.form}>
        <p className={styles.hint}>{t('sheetModel.tables.hint')}</p>
        {info.tables.length === 0 && <p className={styles.hint}>{t('sheetModel.tables.none')}</p>}
        <div className={styles.valueList}>
          {info.tables.map((tb) => (
            <label key={tb.name} className={styles.check}>
              <input
                type="checkbox"
                data-testid={`model-table-${tb.name}`}
                checked={tb.inModel}
                onChange={() => void toggle(tb.name, tb.inModel)}
              />
              <span>
                {tb.missing ? t('sheetModel.tables.missing', { name: tb.name }) : tb.name}
                {tb.sheetName ? ` · ${tb.sheetName}` : ''}
                {typeof tb.rows === 'number'
                  ? ` · ${t('sheetModel.tables.rows', { n: tb.rows })}`
                  : ''}
                {tb.query ? ` · ${t('sheetModel.tables.query', { name: tb.query })}` : ''}
              </span>
            </label>
          ))}
        </div>
        {info.foreign.length > 0 && (
          <div data-testid="model-foreign">
            <strong>{t('sheetModel.foreign.title')}</strong>
            <p className={styles.hint}>{t('sheetModel.foreign.hint')}</p>
            <ul>
              {info.foreign.map((f) => (
                <li key={`${f.kind}-${f.name}`}>
                  {t(`sheetModel.foreign.${f.kind}`, { name: f.name, size: f.size })}
                </li>
              ))}
            </ul>
          </div>
        )}
      </div>
    </Dialog>
  );
}

// ----------------------------------------------------------------- relationships

interface RelDraft {
  index?: number;
  fromTable: string;
  fromColumn: string;
  toTable: string;
  toColumn: string;
  active: boolean;
}

function columnsOf(info: ModelInfo, table: string): string[] {
  return info.tables.find((x) => x.name === table)?.columns.map((c) => c.name) ?? [];
}

export function RelationshipsDialog({
  host,
  info,
  edit,
  onClose,
  fail,
}: {
  host: SheetHost;
  info: ModelInfo;
  edit: Edit;
  onClose: () => void;
  fail: (e: unknown) => void;
}) {
  const { t } = useTranslation();
  const tables = info.tables.filter((x) => x.inModel && !x.missing).map((x) => x.name);
  const [draft, setDraft] = useState<RelDraft | null>(null);
  const [detectedFor, setDetected] = useState<{
    key: string;
    cardinality: ModelCardinality;
    unmatched: number;
  } | null>(null);
  const [error, setError] = useState('');

  const blank = (): RelDraft => {
    const a = tables[0] ?? '';
    const b = tables[1] ?? tables[0] ?? '';
    return {
      fromTable: a,
      fromColumn: columnsOf(info, a)[0] ?? '',
      toTable: b,
      toColumn: columnsOf(info, b)[0] ?? '',
      active: true,
    };
  };

  // Cardinality of the chosen columns (kept with the columns it was computed for).
  const detectKey = draft
    ? JSON.stringify([draft.fromTable, draft.fromColumn, draft.toTable, draft.toColumn])
    : '';
  const detected = detectedFor && detectedFor.key === detectKey ? detectedFor : null;
  useEffect(() => {
    if (!draft || !draft.fromColumn || !draft.toColumn || draft.fromTable === draft.toTable) return;
    let live = true;
    host
      .exec<{ cardinality: ModelCardinality; unmatched: number }>({
        cmd: 'modelDetect',
        fromTable: draft.fromTable,
        fromColumn: draft.fromColumn,
        toTable: draft.toTable,
        toColumn: draft.toColumn,
      })
      .then((r) => live && setDetected({ key: detectKey, ...r }))
      .catch(() => undefined);
    return () => {
      live = false;
    };
  }, [host, draft, detectKey]);

  const save = async () => {
    if (!draft) return;
    try {
      setError('');
      await edit({ cmd: 'modelSetRelationship', ...draft });
      setDraft(null);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    }
  };

  const editOf = (r: ModelRelationshipInfo): RelDraft => ({
    index: r.index,
    fromTable: r.fromTable,
    fromColumn: r.fromColumn,
    toTable: r.toTable,
    toColumn: r.toColumn,
    active: r.active,
  });

  const side = (which: 'from' | 'to') => {
    if (!draft) return null;
    const table = which === 'from' ? draft.fromTable : draft.toTable;
    const column = which === 'from' ? draft.fromColumn : draft.toColumn;
    return (
      <>
        <label>
          {t(which === 'from' ? 'sheetModel.rel.from' : 'sheetModel.rel.to')}
          <select
            data-testid={`model-rel-${which}-table`}
            value={table}
            onChange={(e) => {
              const tb = e.target.value;
              const c = columnsOf(info, tb)[0] ?? '';
              setDraft(
                which === 'from'
                  ? { ...draft, fromTable: tb, fromColumn: c }
                  : { ...draft, toTable: tb, toColumn: c },
              );
            }}
          >
            {tables.map((n) => (
              <option key={n} value={n}>
                {n}
              </option>
            ))}
          </select>
        </label>
        <label>
          {t(which === 'from' ? 'sheetModel.rel.fromColumn' : 'sheetModel.rel.toColumn')}
          <select
            data-testid={`model-rel-${which}-column`}
            value={column}
            onChange={(e) =>
              setDraft(
                which === 'from'
                  ? { ...draft, fromColumn: e.target.value }
                  : { ...draft, toColumn: e.target.value },
              )
            }
          >
            {columnsOf(info, table).map((n) => (
              <option key={n} value={n}>
                {n}
              </option>
            ))}
          </select>
        </label>
      </>
    );
  };

  return (
    <Dialog
      open
      onOpenChange={(v) => !v && onClose()}
      title={t('sheetModel.rel.title')}
      testId="model-rel-dialog"
      width={560}
      footer={<CloseFooter onClose={onClose} />}
    >
      <div className={styles.form}>
        {info.relationships.length === 0 && (
          <p className={styles.hint}>{t('sheetModel.rel.none')}</p>
        )}
        <ul className={styles.valueList} data-testid="model-rel-list">
          {info.relationships.map((r) => (
            <li key={r.index} className={styles.row} data-testid={`model-rel-${r.index}`}>
              <span>
                {t('sheetModel.rel.summary', {
                  many: r.fromTable,
                  manyColumn: r.fromColumn,
                  one: r.toTable,
                  oneColumn: r.toColumn,
                })}
                {!r.active && ` (${t('sheetModel.rel.inactive')})`}
                {r.problem && <span className={styles.error}> {r.problem}</span>}
              </span>
              <button
                type="button"
                className={styles.iconBtn}
                aria-label={t('sheetModel.rel.edit')}
                title={t('sheetModel.rel.edit')}
                onClick={() => setDraft(editOf(r))}
              >
                <Pencil size={12} />
              </button>
              <button
                type="button"
                className={styles.iconBtn}
                aria-label={t('sheetModel.rel.delete')}
                title={t('sheetModel.rel.delete')}
                data-testid={`model-rel-delete-${r.index}`}
                onClick={() =>
                  void edit({ cmd: 'modelDeleteRelationship', index: r.index }).catch(fail)
                }
              >
                <Trash2 size={12} />
              </button>
            </li>
          ))}
        </ul>
        {!draft && (
          <button
            type="button"
            className={styles.btn}
            data-testid="model-rel-add"
            disabled={tables.length < 2}
            onClick={() => setDraft(blank())}
          >
            <Plus size={12} /> {t('sheetModel.rel.add')}
          </button>
        )}
        {draft && (
          <div className={styles.form} data-testid="model-rel-form">
            {side('from')}
            {side('to')}
            <label className={styles.check}>
              <input
                type="checkbox"
                data-testid="model-rel-active"
                checked={draft.active}
                onChange={(e) => setDraft({ ...draft, active: e.target.checked })}
              />
              {t('sheetModel.rel.active')}
            </label>
            {detected && (
              <p
                className={detected.cardinality === 'manyToMany' ? styles.error : styles.hint}
                data-testid="model-rel-cardinality"
              >
                {t(`sheetModel.rel.card.${detected.cardinality}`)}
                {detected.unmatched > 0 &&
                  ` ${t('sheetModel.rel.unmatched', { n: detected.unmatched })}`}
              </p>
            )}
            {error && <p className={styles.error}>{error}</p>}
            <div className={styles.row}>
              <button type="button" className={styles.btn} onClick={() => setDraft(null)}>
                {t('sheetModel.rel.cancel')}
              </button>
              <button
                type="button"
                className={styles.btnPrimary}
                data-testid="model-rel-save"
                disabled={detected?.cardinality === 'manyToMany'}
                onClick={() => void save()}
              >
                {t('sheetModel.rel.save')}
              </button>
            </div>
          </div>
        )}
      </div>
    </Dialog>
  );
}

// ----------------------------------------------------------------- measures

interface MeasureDraft {
  kind: 'measure' | 'column';
  original?: string;
  name: string;
  table: string;
  expression: string;
  format: string;
}

export function MeasuresDialog({
  host,
  info,
  edit,
  onClose,
}: {
  host: SheetHost;
  info: ModelInfo;
  edit: Edit;
  onClose: () => void;
}) {
  const { t } = useTranslation();
  const tables = info.tables.filter((x) => x.inModel && !x.missing).map((x) => x.name);
  const [draft, setDraft] = useState<MeasureDraft | null>(null);
  const [checked, setCheck] = useState<{ key: string; r: ModelCheckResult } | null>(null);
  const [caret, setCaret] = useState(0);
  const [error, setError] = useState('');
  const box = useRef<HTMLTextAreaElement>(null);

  // Check the formula while typing (debounced); a result counts for the text it checked.
  const checkKey = draft
    ? JSON.stringify([draft.kind, draft.table, draft.name, draft.expression])
    : '';
  const check =
    draft && draft.expression.trim() !== '' && checked?.key === checkKey ? checked.r : null;
  useEffect(() => {
    if (!draft || draft.expression.trim() === '') return;
    let live = true;
    const timer = setTimeout(() => {
      host
        .exec<ModelCheckResult>({
          cmd: 'modelCheck',
          expression: draft.expression,
          ...(draft.kind === 'column' ? { table: draft.table } : { name: draft.name }),
        })
        .then((r) => live && setCheck({ key: checkKey, r }))
        .catch(() => undefined);
    }, 200);
    return () => {
      live = false;
      clearTimeout(timer);
    };
  }, [host, draft, checkKey]);

  const suggestions = draft
    ? suggest(info, draft.expression, caret, draft.kind === 'column' ? draft.table : undefined)
    : { from: 0, items: [] };

  const pick = (s: ModelSuggestion) => {
    if (!draft) return;
    const text =
      draft.expression.slice(0, suggestions.from) + s.insert + draft.expression.slice(caret);
    const at = suggestions.from + s.insert.length;
    setDraft({ ...draft, expression: text });
    setCaret(at);
    requestAnimationFrame(() => {
      box.current?.focus();
      box.current?.setSelectionRange(at, at);
    });
  };

  const save = async () => {
    if (!draft) return;
    try {
      setError('');
      if (draft.kind === 'measure') {
        await edit({
          cmd: 'modelSetMeasure',
          ...(draft.original ? { original: draft.original } : {}),
          name: draft.name,
          table: draft.table,
          expression: draft.expression,
          ...(draft.format.trim() ? { format: draft.format } : {}),
        });
      } else {
        await edit({
          cmd: 'modelSetColumn',
          ...(draft.original ? { original: draft.original } : {}),
          table: draft.table,
          name: draft.name,
          expression: draft.expression,
        });
      }
      setDraft(null);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    }
  };

  const remove = async (kind: 'measure' | 'column', name: string, table: string) => {
    try {
      await edit(
        kind === 'measure'
          ? { cmd: 'modelDeleteMeasure', name }
          : { cmd: 'modelDeleteColumn', table, name },
      );
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    }
  };

  const rows = [
    ...info.measures.map((m) => ({ kind: 'measure' as const, ...m, format: m.format ?? '' })),
    ...info.columns.map((c) => ({ kind: 'column' as const, ...c, format: '' })),
  ];

  return (
    <Dialog
      open
      onOpenChange={(v) => !v && onClose()}
      title={t('sheetModel.measure.title')}
      testId="model-measure-dialog"
      width={600}
      footer={<CloseFooter onClose={onClose} />}
    >
      <div className={styles.form}>
        {rows.length === 0 && <p className={styles.hint}>{t('sheetModel.measure.none')}</p>}
        <ul className={styles.valueList} data-testid="model-measure-list">
          {rows.map((r) => (
            <li key={`${r.kind}-${r.table}-${r.name}`} className={styles.row}>
              <span>
                <strong>{r.kind === 'measure' ? `[${r.name}]` : `${r.table}[${r.name}]`}</strong>{' '}
                <code>{r.expression}</code>
                {r.error && <span className={styles.error}> {r.error}</span>}
              </span>
              <button
                type="button"
                className={styles.iconBtn}
                aria-label={t('sheetModel.measure.edit')}
                title={t('sheetModel.measure.edit')}
                onClick={() =>
                  setDraft({
                    kind: r.kind,
                    original: r.name,
                    name: r.name,
                    table: r.table,
                    expression: r.expression,
                    format: r.format,
                  })
                }
              >
                <Pencil size={12} />
              </button>
              <button
                type="button"
                className={styles.iconBtn}
                aria-label={t('sheetModel.measure.delete')}
                title={t('sheetModel.measure.delete')}
                onClick={() => void remove(r.kind, r.name, r.table)}
              >
                <Trash2 size={12} />
              </button>
            </li>
          ))}
        </ul>
        {!draft && (
          <button
            type="button"
            className={styles.btn}
            data-testid="model-measure-new"
            disabled={tables.length === 0}
            onClick={() =>
              setDraft({
                kind: 'measure',
                name: '',
                table: tables[0] ?? '',
                expression: '',
                format: '',
              })
            }
          >
            <Plus size={12} /> {t('sheetModel.measure.new')}
          </button>
        )}
        {draft && (
          <div className={styles.form} data-testid="model-measure-form">
            <label>
              {t('sheetModel.measure.kind')}
              <select
                data-testid="model-measure-kind"
                value={draft.kind}
                disabled={!!draft.original}
                onChange={(e) =>
                  setDraft({ ...draft, kind: e.target.value as MeasureDraft['kind'] })
                }
              >
                <option value="measure">{t('sheetModel.measure.kindMeasure')}</option>
                <option value="column">{t('sheetModel.measure.kindColumn')}</option>
              </select>
            </label>
            <label>
              {t('sheetModel.measure.table')}
              <select
                data-testid="model-measure-table"
                value={draft.table}
                onChange={(e) => setDraft({ ...draft, table: e.target.value })}
              >
                {tables.map((n) => (
                  <option key={n} value={n}>
                    {n}
                  </option>
                ))}
              </select>
            </label>
            <label>
              {t('sheetModel.measure.name')}
              <input
                data-testid="model-measure-name"
                value={draft.name}
                onChange={(e) => setDraft({ ...draft, name: e.target.value })}
              />
            </label>
            <label>
              {t('sheetModel.measure.expression')}
              <textarea
                ref={box}
                rows={3}
                data-testid="model-measure-expression"
                spellCheck={false}
                value={draft.expression}
                onChange={(e) => {
                  setDraft({ ...draft, expression: e.target.value });
                  setCaret(e.target.selectionStart ?? e.target.value.length);
                }}
                onSelect={(e) => setCaret(e.currentTarget.selectionStart ?? 0)}
              />
            </label>
            {suggestions.items.length > 0 && (
              <div
                className={styles.row}
                role="listbox"
                aria-label={t('sheetModel.measure.suggestions')}
                data-testid="model-suggestions"
              >
                {suggestions.items.slice(0, 12).map((s) => (
                  <button
                    key={`${s.kind}-${s.label}`}
                    type="button"
                    role="option"
                    aria-selected={false}
                    className={styles.btn}
                    data-testid={`model-suggest-${s.label}`}
                    onClick={() => pick(s)}
                  >
                    {s.label}
                  </button>
                ))}
              </div>
            )}
            <p
              className={check && !check.ok ? styles.error : styles.hint}
              data-testid="model-measure-check"
            >
              {checkText(check, t)}
            </p>
            {draft.kind === 'measure' && (
              <label>
                {t('sheetModel.measure.format')}
                <input
                  data-testid="model-measure-format"
                  value={draft.format}
                  placeholder="#,##0"
                  onChange={(e) => setDraft({ ...draft, format: e.target.value })}
                />
              </label>
            )}
            <p className={styles.hint}>{t('sheetModel.measure.help')}</p>
            {error && <p className={styles.error}>{error}</p>}
            <div className={styles.row}>
              <button type="button" className={styles.btn} onClick={() => setDraft(null)}>
                {t('sheetModel.rel.cancel')}
              </button>
              <button
                type="button"
                className={styles.btnPrimary}
                data-testid="model-measure-save"
                disabled={!draft.name.trim() || !!(check && !check.ok)}
                onClick={() => void save()}
              >
                {t('sheetModel.measure.save')}
              </button>
            </div>
          </div>
        )}
      </div>
    </Dialog>
  );
}
