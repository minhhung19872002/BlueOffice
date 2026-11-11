/**
 * Controls of the BlueSheet Home tab that are not plain buttons — the font name and size
 * boxes, the number format box — and the small dialogs of the Cells group (column width,
 * row height, insert / delete cells).
 */
import { useTranslation } from '@blueoffice/i18n';
import { Dialog } from '@blueoffice/ui';
import { ChevronDown } from 'lucide-react';
import { useState, type ReactNode } from 'react';
import { SHEET_FONT_SIZES, SHEET_FONTS, numberCategory } from './homeFormat';
import type { NumberPreset } from './FormatCellsDialog';
import styles from './SheetEditor.module.css';

/** An editable box: typing then Enter applies, picking from its list applies at once. */
function ComboBox({
  value,
  list,
  label,
  testId,
  width,
  disabled,
  valid,
  onApply,
  onDone,
}: {
  value: string;
  list: string[];
  label: string;
  testId: string;
  width: number;
  disabled?: boolean;
  valid: (v: string) => boolean;
  onApply: (v: string) => void;
  onDone: () => void;
}) {
  // Text being typed; the active cell's value otherwise.
  const [edit, setEdit] = useState<string | null>(null);
  const text = edit ?? value;
  const listId = `${testId}-list`;
  const apply = (v: string) => {
    setEdit(null);
    if (valid(v) && v !== value) onApply(v.trim());
    onDone();
  };
  return (
    <>
      <input
        className={styles.ribbonBox}
        style={{ width }}
        data-testid={testId}
        aria-label={label}
        title={label}
        list={listId}
        disabled={disabled}
        value={text}
        onFocus={(e) => e.currentTarget.select()}
        onChange={(e) => {
          setEdit(e.target.value);
          if ((e.nativeEvent as InputEvent).inputType === 'insertReplacementText')
            apply(e.target.value);
        }}
        onKeyDown={(e) => {
          if (e.key === 'Enter') {
            e.preventDefault();
            apply(text);
          } else if (e.key === 'Escape') {
            e.preventDefault();
            setEdit(null);
            onDone();
          }
        }}
        onBlur={() => setEdit(null)}
      />
      <datalist id={listId}>
        {list.map((v) => (
          <option key={v} value={v} />
        ))}
      </datalist>
    </>
  );
}

/** Font name of the active cell; any name can be typed (§18.8.22 font name). */
export function FontNameBox(props: {
  value: string;
  disabled?: boolean;
  onApply: (name: string) => void;
  onDone: () => void;
}) {
  const { t } = useTranslation();
  return (
    <ComboBox
      {...props}
      list={SHEET_FONTS}
      label={t('sheetEditor.fontName')}
      testId="sheet-font-name"
      width={104}
      valid={(v) => v.trim().length > 0}
    />
  );
}

/** Font size of the active cell; any size 1–409 pt can be typed (§18.8.38 sz). */
export function FontSizeBox(props: {
  value: number | null;
  disabled?: boolean;
  onApply: (size: number) => void;
  onDone: () => void;
}) {
  const { t } = useTranslation();
  const num = (v: string) => Number(v.replace(',', '.'));
  return (
    <ComboBox
      value={props.value === null ? '' : String(props.value)}
      disabled={props.disabled}
      onDone={props.onDone}
      list={SHEET_FONT_SIZES.map(String)}
      label={t('sheetEditor.fontSizeBox')}
      testId="sheet-font-size"
      width={44}
      valid={(v) => Number.isFinite(num(v)) && num(v) >= 1 && num(v) <= 409}
      onApply={(v) => props.onApply(Math.round(num(v) * 2) / 2)}
    />
  );
}

/**
 * A split button: the main part repeats the last choice (`id`), the narrow arrow opens the
 * choices (`menuId`, anchored under it).
 */
export function SplitButton({
  id,
  menuId,
  label,
  menuLabel,
  icon,
  disabled,
  onApply,
  onMenu,
}: {
  id: string;
  menuId: string;
  label: string;
  menuLabel: string;
  icon: ReactNode;
  disabled?: boolean;
  onApply: () => void;
  onMenu: () => void;
}) {
  return (
    <span className={styles.split}>
      <button
        type="button"
        className={styles.splitMain}
        data-command={id}
        aria-label={label}
        title={label}
        aria-disabled={disabled}
        onMouseDown={(e) => e.preventDefault()}
        onClick={() => !disabled && onApply()}
      >
        {icon}
      </button>
      <button
        type="button"
        className={styles.splitArrow}
        data-command={menuId}
        aria-label={menuLabel}
        title={menuLabel}
        aria-haspopup="menu"
        aria-disabled={disabled}
        onMouseDown={(e) => e.preventDefault()}
        onClick={() => !disabled && onMenu()}
      >
        <ChevronDown size={10} />
      </button>
    </span>
  );
}

/** Ids of the presets the box lists, in the order of the spreadsheet's list. */
const BOX_PRESETS = [
  'general',
  'number',
  'currencyVnd',
  'accounting',
  'date',
  'dateLong',
  'time',
  'percent',
  'fraction',
  'scientific',
  'text',
];

/**
 * Number format box: the category of the active cell's format; picking one applies its
 * preset, "More number formats…" opens Format Cells.
 */
export function NumberFormatBox({
  code,
  presets,
  disabled,
  onPick,
  onMore,
}: {
  code: string;
  presets: NumberPreset[];
  disabled?: boolean;
  onPick: (code: string) => void;
  onMore: () => void;
}) {
  const { t } = useTranslation();
  const exact = presets.find((p) => p.code === code)?.id;
  const current = exact ?? numberCategory(code);
  const shown = presets.filter((p) => BOX_PRESETS.includes(p.id));
  const label = t('sheetEditor.numberFormatBox');
  return (
    <select
      className={styles.ribbonBox}
      style={{ width: 116 }}
      data-testid="sheet-number-format"
      aria-label={label}
      title={label}
      disabled={disabled}
      value={current}
      onChange={(e) => {
        const id = e.target.value;
        if (id === 'more') {
          onMore();
          return;
        }
        const p = presets.find((x) => x.id === id);
        if (p) onPick(p.code);
      }}
    >
      {!shown.some((p) => p.id === current) && (
        <option value={current}>{t(`sheetEditor.numFmt.${current}`)}</option>
      )}
      {shown.map((p) => (
        <option key={p.id} value={p.id}>
          {t(`sheetEditor.numFmt.${p.id}`)}
        </option>
      ))}
      <option value="more">{t('sheetEditor.numFmtMore')}</option>
    </select>
  );
}

/** Column width (characters) or row height (points) dialog. */
export function SizeDialog({
  rows,
  initial,
  onApply,
  onClose,
}: {
  rows: boolean;
  initial: number;
  onApply: (size: number) => void;
  onClose: () => void;
}) {
  const { t } = useTranslation();
  const [text, setText] = useState(String(initial).replace('.', ','));
  const value = Number(text.trim().replace(',', '.'));
  const max = rows ? 409 : 255;
  const ok = text.trim() !== '' && Number.isFinite(value) && value >= 0 && value <= max;
  const submit = () => {
    if (ok) onApply(value);
  };
  return (
    <Dialog
      open
      onOpenChange={(o) => !o && onClose()}
      title={t(rows ? 'sheetEditor.rowHeightTitle' : 'sheetEditor.colWidthTitle')}
      width={300}
      testId="size-dialog"
      footer={
        <>
          <button type="button" className={styles.btn} onClick={onClose}>
            {t('sheetEditor.dlg.cancel')}
          </button>
          <button
            type="button"
            className={styles.btnPrimary}
            data-testid="size-dialog-ok"
            disabled={!ok}
            onClick={submit}
          >
            {t('sheetEditor.dlg.ok')}
          </button>
        </>
      }
    >
      <form
        className={styles.form}
        onSubmit={(e) => {
          e.preventDefault();
          submit();
        }}
      >
        <label>
          {t(rows ? 'sheetEditor.rowHeightLabel' : 'sheetEditor.colWidthLabel')}
          <input
            data-testid="size-dialog-value"
            inputMode="decimal"
            autoFocus
            value={text}
            onFocus={(e) => e.currentTarget.select()}
            onChange={(e) => setText(e.target.value)}
          />
        </label>
        {!ok && <p className={styles.hint}>{t('sheetEditor.sizeRange', { max })}</p>}
      </form>
    </Dialog>
  );
}

/**
 * Create table (Ctrl+T / Ctrl+L): the range of the data (the current region of a single
 * cell) and whether its first row holds the column names (guessed from the cells).
 */
export function CreateTableDialog({
  range,
  headers,
  onApply,
  onClose,
}: {
  range: string;
  headers: boolean;
  onApply: (range: string, headers: boolean) => void;
  onClose: () => void;
}) {
  const { t } = useTranslation();
  const [text, setText] = useState(range);
  const [hasHeaders, setHasHeaders] = useState(headers);
  const ok = /^\$?[A-Za-z]{1,3}\$?\d+(:\$?[A-Za-z]{1,3}\$?\d+)?$/.test(text.trim());
  return (
    <Dialog
      open
      onOpenChange={(o) => !o && onClose()}
      title={t('sheetEditor.createTable.title')}
      width={340}
      testId="create-table"
      footer={
        <>
          <button type="button" className={styles.btn} onClick={onClose}>
            {t('sheetEditor.dlg.cancel')}
          </button>
          <button
            type="button"
            className={styles.btnPrimary}
            data-testid="create-table-ok"
            disabled={!ok}
            onClick={() => onApply(text.trim(), hasHeaders)}
          >
            {t('sheetEditor.dlg.ok')}
          </button>
        </>
      }
    >
      <form
        className={styles.form}
        onSubmit={(e) => {
          e.preventDefault();
          if (ok) onApply(text.trim(), hasHeaders);
        }}
      >
        <label>
          {t('sheetEditor.createTable.range')}
          <input
            data-testid="create-table-range"
            autoFocus
            value={text}
            onFocus={(e) => e.currentTarget.select()}
            onChange={(e) => setText(e.target.value)}
          />
        </label>
        <label className={styles.check}>
          <input
            type="checkbox"
            data-testid="create-table-headers"
            checked={hasHeaders}
            onChange={(e) => setHasHeaders(e.target.checked)}
          />
          {t('sheetEditor.createTable.headers')}
        </label>
      </form>
    </Dialog>
  );
}

/** A question with OK / Cancel (OK has the focus, Enter confirms). */
export function ConfirmDialog({
  title,
  message,
  testId,
  onOk,
  onClose,
}: {
  title: string;
  message: string;
  testId: string;
  onOk: () => void;
  onClose: () => void;
}) {
  const { t } = useTranslation();
  return (
    <Dialog
      open
      onOpenChange={(o) => !o && onClose()}
      title={title}
      width={380}
      testId={testId}
      footer={
        <>
          <button type="button" className={styles.btn} onClick={onClose}>
            {t('sheetEditor.dlg.cancel')}
          </button>
          <button
            type="button"
            className={styles.btnPrimary}
            data-testid={`${testId}-ok`}
            autoFocus
            onClick={onOk}
          >
            {t('sheetEditor.dlg.ok')}
          </button>
        </>
      }
    >
      <p>{message}</p>
    </Dialog>
  );
}

/** What Insert / Delete cells does with the other cells. */
export type CellsShift = 'right' | 'down' | 'left' | 'up' | 'row' | 'col';

/** Insert (shift right / down, entire row / column) or Delete (shift left / up, …) dialog. */
export function CellsDialog({
  insert,
  onApply,
  onClose,
}: {
  insert: boolean;
  onApply: (shift: CellsShift) => void;
  onClose: () => void;
}) {
  const { t } = useTranslation();
  const choices: CellsShift[] = insert
    ? ['right', 'down', 'row', 'col']
    : ['left', 'up', 'row', 'col'];
  // Default: shift down on insert, up on delete.
  const [pick, setPick] = useState<CellsShift>(insert ? 'down' : 'up');
  return (
    <Dialog
      open
      onOpenChange={(o) => !o && onClose()}
      title={t(
        insert ? 'sheetEditor.cellsDialog.insertTitle' : 'sheetEditor.cellsDialog.deleteTitle',
      )}
      width={300}
      testId="cells-dialog"
      footer={
        <>
          <button type="button" className={styles.btn} onClick={onClose}>
            {t('sheetEditor.dlg.cancel')}
          </button>
          <button
            type="button"
            className={styles.btnPrimary}
            data-testid="cells-dialog-ok"
            onClick={() => onApply(pick)}
          >
            {t('sheetEditor.dlg.ok')}
          </button>
        </>
      }
    >
      <div className={styles.form} role="radiogroup">
        {choices.map((c) => (
          <label key={c} className={styles.check}>
            <input
              type="radio"
              name="cells-shift"
              data-shift={c}
              checked={pick === c}
              autoFocus={pick === c}
              onChange={() => setPick(c)}
              onKeyDown={(e) => {
                if (e.key === 'Enter') {
                  e.preventDefault();
                  onApply(pick);
                }
              }}
            />
            {t(`sheetEditor.cellsDialog.${c}`)}
          </label>
        ))}
      </div>
    </Dialog>
  );
}
