/**
 * Dialogs of the BlueDoc table tools (ECMA-376 Part 1, §17.4): sort rows, cell formula
 * (§17.16.3 `=` field) and converting text to a table and back.
 */
import { useTranslation } from '@blueoffice/i18n';
import type { EditCommand, EditResult, TableSortKey, TableState } from '@blueoffice/platform';
import { Dialog } from '@blueoffice/ui';
import { useState } from 'react';
import styles from './Editor.module.css';

type Run = (cmd: EditCommand) => Promise<EditResult | null>;

/** Footer with Cancel / OK shared by the dialogs. */
function Buttons({
  onCancel,
  onOk,
  okTestId,
  okDisabled,
}: {
  onCancel: () => void;
  onOk: () => void;
  okTestId: string;
  okDisabled?: boolean;
}) {
  const { t } = useTranslation();
  return (
    <>
      <button type="button" className={styles.dialogButton} onClick={onCancel}>
        {t('security.cancel')}
      </button>
      <button
        type="button"
        className={styles.dialogPrimary}
        data-testid={okTestId}
        disabled={okDisabled}
        onClick={onOk}
      >
        {t('pageSetup.ok')}
      </button>
    </>
  );
}

/** A sort level in the dialog; column -1 = unused. */
interface Level {
  column: number;
  kind: TableSortKey['kind'];
  descending: boolean;
}

/** Sort keys sent to the engine: the used levels, in order. */
export function sortKeys(levels: Level[]): TableSortKey[] {
  return levels
    .filter((l) => l.column >= 0)
    .map((l) => ({ column: l.column, kind: l.kind, descending: l.descending }));
}

/** Sort the table rows by up to three columns. */
export function SortDialog({
  table,
  run,
  onClose,
}: {
  table: TableState | null | undefined;
  run: Run;
  onClose: () => void;
}) {
  const { t } = useTranslation();
  const cols = Math.max(1, table?.cols ?? 1);
  const [levels, setLevels] = useState<Level[]>([
    { column: table?.col ?? 0, kind: 'text', descending: false },
    { column: -1, kind: 'text', descending: false },
    { column: -1, kind: 'text', descending: false },
  ]);
  const [header, setHeader] = useState(!!table?.headerRow);
  const [error, setError] = useState<string | null>(null);
  const set = (i: number, patch: Partial<Level>) =>
    setLevels((ls) => ls.map((l, k) => (k === i ? { ...l, ...patch } : l)));
  const keys = sortKeys(levels);
  const apply = async () => {
    const r = await run({ cmd: 'tableEdit', edit: { kind: 'sort', keys, header } });
    if (r?.error) {
      setError(r.error);
      return;
    }
    onClose();
  };
  return (
    <Dialog
      open
      onOpenChange={(o) => !o && onClose()}
      title={t('table.sortTitle')}
      testId="table-sort"
      footer={
        <Buttons
          onCancel={onClose}
          onOk={() => void apply()}
          okTestId="table-sort-ok"
          okDisabled={keys.length === 0}
        />
      }
    >
      <div className={styles.formGrid}>
        {levels.map((l, i) => (
          <fieldset key={i} className={styles.field} data-testid={`sort-level-${i}`}>
            <legend>{t('table.sortLevel', { n: i + 1 })}</legend>
            <select
              aria-label={t('table.sortLevel', { n: i + 1 })}
              data-testid={`sort-column-${i}`}
              value={l.column}
              onChange={(e) => set(i, { column: Number(e.target.value) })}
            >
              {i > 0 && <option value={-1}>{t('table.sortNone')}</option>}
              {Array.from({ length: cols }, (_, c) => (
                <option key={c} value={c}>
                  {t('table.column', { n: c + 1 })}
                </option>
              ))}
            </select>
            <select
              aria-label={t('table.kind')}
              data-testid={`sort-kind-${i}`}
              value={l.kind}
              disabled={l.column < 0}
              onChange={(e) => set(i, { kind: e.target.value as Level['kind'] })}
            >
              <option value="text">{t('table.kindText')}</option>
              <option value="number">{t('table.kindNumber')}</option>
              <option value="date">{t('table.kindDate')}</option>
            </select>
            <select
              aria-label={t('table.descending')}
              data-testid={`sort-order-${i}`}
              value={l.descending ? 'desc' : 'asc'}
              disabled={l.column < 0}
              onChange={(e) => set(i, { descending: e.target.value === 'desc' })}
            >
              <option value="asc">{t('table.ascending')}</option>
              <option value="desc">{t('table.descending')}</option>
            </select>
          </fieldset>
        ))}
        <label className={styles.field}>
          <input
            type="checkbox"
            data-testid="sort-header"
            checked={header}
            onChange={(e) => setHeader(e.target.checked)}
          />
          <span>{t('table.hasHeader')}</span>
        </label>
        {error && (
          <p role="alert" data-testid="sort-error">
            {error}
          </p>
        )}
      </div>
    </Dialog>
  );
}

/** Functions offered by the formula dialog (§17.16.3). */
export const FORMULA_FUNCTIONS = [
  'SUM',
  'AVERAGE',
  'COUNT',
  'MAX',
  'MIN',
  'PRODUCT',
  'ABS',
  'ROUND',
  'IF',
] as const;

/** Number pictures offered by the formula dialog (§17.16.4.2). */
export const NUMBER_FORMATS = ['#,##0', '#,##0.00', '0', '0.00', '0%', "#,##0 'đ'"] as const;

/**
 * The field instruction of a formula: `=` + the expression, with a `\#` numeric
 * picture switch when a format is chosen. Literal text in a picture is quoted with
 * apostrophes (§17.16.4.2), so double quotes typed in it become apostrophes.
 */
export function formulaInstr(formula: string, picture: string): string {
  const expr = formula.trim().replace(/^=\s*/, '');
  const pic = picture.trim();
  if (!pic) return `=${expr}`;
  return `=${expr} \\# "${pic.replace(/"/g, "'")}"`;
}

/** The formula proposed for a cell: the numbers above it, else those to its left. */
export function defaultFormula(table: TableState | null | undefined): string {
  return table && table.row === 0 && table.col > 0 ? '=SUM(LEFT)' : '=SUM(ABOVE)';
}

/** Insert a formula field in the caret's cell; F9 updates it later. */
export function FormulaDialog({
  table,
  run,
  onClose,
}: {
  table: TableState | null | undefined;
  run: Run;
  onClose: () => void;
}) {
  const { t } = useTranslation();
  const [formula, setFormula] = useState(defaultFormula(table));
  const [picture, setPicture] = useState('');
  const apply = async () => {
    await run({ cmd: 'insertField', instr: formulaInstr(formula, picture) });
    onClose();
  };
  return (
    <Dialog
      open
      onOpenChange={(o) => !o && onClose()}
      title={t('table.formulaTitle')}
      testId="table-formula"
      footer={
        <Buttons
          onCancel={onClose}
          onOk={() => void apply()}
          okTestId="table-formula-ok"
          okDisabled={!formula.trim().replace(/^=\s*/, '')}
        />
      }
    >
      <div className={styles.formGrid}>
        <label className={styles.field}>
          <span>{t('table.formula')}</span>
          <input
            data-testid="formula-input"
            value={formula}
            onChange={(e) => setFormula(e.target.value)}
          />
        </label>
        <p>{t('table.formulaHint')}</p>
        <label className={styles.field}>
          <span>{t('table.function')}</span>
          <select
            data-testid="formula-function"
            value=""
            onChange={(e) => {
              const f = e.target.value;
              if (f) setFormula((v) => `${v.trim() || '='}${f}()`);
            }}
          >
            <option value="" />
            {FORMULA_FUNCTIONS.map((f) => (
              <option key={f} value={f}>
                {f}
              </option>
            ))}
          </select>
        </label>
        <label className={styles.field}>
          <span>{t('table.numberFormat')}</span>
          <select
            data-testid="formula-format"
            value={picture}
            onChange={(e) => setPicture(e.target.value)}
          >
            <option value="">{t('table.numberFormatNone')}</option>
            {NUMBER_FORMATS.map((f) => (
              <option key={f} value={f}>
                {f}
              </option>
            ))}
          </select>
        </label>
      </div>
    </Dialog>
  );
}

type SepKind = 'tab' | 'comma' | 'paragraph' | 'other';

/** The separator text the engine expects for a choice of the dialog. */
export function separatorOf(kind: SepKind, other: string): string {
  switch (kind) {
    case 'tab':
      return '\t';
    case 'comma':
      return ',';
    case 'paragraph':
      return '\n';
    default:
      return other;
  }
}

/** Convert the selected paragraphs to a table (`toTable`) or the caret's table to text. */
export function ConvertDialog({
  toTable,
  run,
  onClose,
}: {
  toTable: boolean;
  run: Run;
  onClose: () => void;
}) {
  const { t } = useTranslation();
  const [kind, setKind] = useState<SepKind>('tab');
  const [other, setOther] = useState('-');
  const [cols, setCols] = useState(2);
  const separator = separatorOf(kind, other);
  const apply = async () => {
    await run({
      cmd: 'tableEdit',
      edit: toTable
        ? { kind: 'fromText', separator, cols: kind === 'paragraph' ? cols : null }
        : { kind: 'toText', separator },
    });
    onClose();
  };
  const kinds: [SepKind, string][] = [
    ['tab', 'table.sepTab'],
    ['comma', 'table.sepComma'],
    ['paragraph', 'table.sepParagraph'],
    ['other', 'table.sepOther'],
  ];
  return (
    <Dialog
      open
      onOpenChange={(o) => !o && onClose()}
      title={t(toTable ? 'table.fromTextTitle' : 'table.toTextTitle')}
      testId="table-convert"
      footer={
        <Buttons
          onCancel={onClose}
          onOk={() => void apply()}
          okTestId="table-convert-ok"
          okDisabled={!separator}
        />
      }
    >
      <div className={styles.formGrid} role="radiogroup" aria-label={t('table.separator')}>
        <span>{t('table.separator')}</span>
        {kinds.map(([k, key]) => (
          <label key={k} className={styles.field}>
            <input
              type="radio"
              name="table-separator"
              data-testid={`sep-${k}`}
              checked={kind === k}
              onChange={() => setKind(k)}
            />
            <span>{t(key)}</span>
          </label>
        ))}
        {kind === 'other' && (
          <input
            aria-label={t('table.sepOther')}
            data-testid="sep-other-text"
            maxLength={4}
            value={other}
            onChange={(e) => setOther(e.target.value)}
          />
        )}
        {toTable && kind === 'paragraph' && (
          <label className={styles.field}>
            <span>{t('table.columns')}</span>
            <input
              type="number"
              min="1"
              max="63"
              data-testid="convert-cols"
              value={cols}
              onChange={(e) => setCols(Math.min(63, Math.max(1, Number(e.target.value) || 1)))}
            />
          </label>
        )}
      </div>
    </Dialog>
  );
}
