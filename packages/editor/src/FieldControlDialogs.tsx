import { useTranslation } from '@blueoffice/i18n';
import type {
  ControlAppearance,
  ControlInfo,
  ControlListItem,
  ControlProps,
  MergePrompt,
} from '@blueoffice/platform';
import { Dialog } from '@blueoffice/ui';
import { useState } from 'react';
import styles from './Editor.module.css';

/**
 * Ctrl+F9: a new field from its code (ECMA-376 Part 1, §17.16.1 syntax). The selected text, when
 * there is one, starts as the code; the field shows its result once inserted.
 */
export function FieldCodeDialog({
  initial,
  onInsert,
  onClose,
}: {
  initial: string;
  onInsert: (instr: string) => void;
  onClose: () => void;
}) {
  const { t } = useTranslation();
  const [code, setCode] = useState(initial);
  const ok = () => code.trim() !== '' && onInsert(code.trim());
  return (
    <Dialog
      open
      onOpenChange={(o) => !o && onClose()}
      title={t('fieldCode.title')}
      testId="field-code-dialog"
      footer={
        <>
          <button type="button" className={styles.dialogButton} onClick={onClose}>
            {t('security.cancel')}
          </button>
          <button
            type="button"
            className={styles.dialogPrimary}
            data-testid="field-code-ok"
            disabled={code.trim() === ''}
            onClick={ok}
          >
            {t('fieldCode.insert')}
          </button>
        </>
      }
    >
      <label className={styles.field}>
        {t('fieldCode.label')}
        <span className={styles.fieldCodeBraces}>
          <span aria-hidden="true">{'{'}</span>
          <input
            data-testid="field-code-input"
            value={code}
            spellCheck={false}
            autoFocus
            placeholder={t('fieldCode.hint')}
            onChange={(e) => setCode(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === 'Enter') {
                e.preventDefault();
                ok();
              }
            }}
          />
          <span aria-hidden="true">{'}'}</span>
        </span>
      </label>
    </Dialog>
  );
}

/**
 * The questions of ASK / FILLIN fields asked one after the other while the fields update
 * (§17.16.5.4, §17.16.5.20). OK keeps the typed answer; Cancel keeps the default answer.
 */
export function FieldPromptDialog({
  prompts,
  onDone,
}: {
  prompts: MergePrompt[];
  onDone: (answers: Record<string, string>) => void;
}) {
  const { t } = useTranslation();
  const [i, setI] = useState(0);
  const [answers, setAnswers] = useState<Record<string, string>>({});
  const p = prompts[i];
  const [value, setValue] = useState(p?.default ?? '');
  if (!p) return null;
  const next = (answer: string | null) => {
    const a = answer === null ? answers : { ...answers, [p.key]: answer };
    setAnswers(a);
    const n = prompts[i + 1];
    if (!n) {
      onDone(a);
      return;
    }
    setI(i + 1);
    setValue(n.default);
  };
  return (
    <Dialog
      open
      onOpenChange={(o) => !o && next(null)}
      title={t('fieldPrompt.title')}
      testId="field-prompt-dialog"
      footer={
        <>
          <button
            type="button"
            className={styles.dialogButton}
            data-testid="field-prompt-cancel"
            onClick={() => next(null)}
          >
            {t('security.cancel')}
          </button>
          <button
            type="button"
            className={styles.dialogPrimary}
            data-testid="field-prompt-ok"
            onClick={() => next(value)}
          >
            {t('fieldPrompt.ok')}
          </button>
        </>
      }
    >
      <label className={styles.field}>
        <span data-testid="field-prompt-text">{p.prompt || t('fieldPrompt.noText')}</span>
        <textarea
          key={p.key}
          data-testid="field-prompt-input"
          rows={3}
          value={value}
          autoFocus
          onChange={(e) => setValue(e.target.value)}
        />
      </label>
    </Dialog>
  );
}

/** Date display formats offered for each language (§17.5.2 dateFormat; any other can be typed). */
const DATE_FORMATS: Record<string, string[]> = {
  'vi-VN': ['dd/MM/yyyy', 'd/M/yyyy', 'dd-MM-yyyy', 'yyyy-MM-dd', 'MMMM yyyy'],
  'en-US': ['M/d/yyyy', 'MM/dd/yyyy', 'dddd, MMMM d, yyyy', 'MMMM d, yyyy', 'yyyy-MM-dd'],
};

/**
 * Content control properties: title, tag, how it is shown and its colour ([MS-DOCX]
 * w15:appearance, w15:color), removing it once edited (§17.5.2 temporary), the two locks
 * (§17.5.2.1, §17.5.2.42, §17.5.2.23), the items of a list control (add / modify / remove /
 * move) and the display format and language of a date control.
 */
export function ControlPropsDialog({
  control,
  onApply,
  onClose,
}: {
  control: ControlInfo;
  onApply: (props: ControlProps) => void;
  onClose: () => void;
}) {
  const { t } = useTranslation();
  const list = control.kind === 'dropDown' || control.kind === 'comboBox';
  const date = control.kind === 'date';
  const [props, setProps] = useState<ControlProps>({
    title: control.title ?? '',
    tag: control.tag ?? '',
    deleteLocked: control.deleteLocked,
    contentLocked: control.locked,
    appearance: control.appearance ?? 'boundingBox',
    color: control.color ?? '',
    temporary: control.temporary ?? false,
    ...(list
      ? {
          items: control.items.map((d, i) => ({
            displayText: d,
            value: control.values?.[i] ?? d,
          })),
        }
      : {}),
    ...(date
      ? { dateFormat: control.format ?? 'dd/MM/yyyy', dateLid: control.lid ?? 'vi-VN' }
      : {}),
  });
  const set = (p: Partial<ControlProps>) => setProps((old) => ({ ...old, ...p }));
  const [sel, setSel] = useState(0);
  const [display, setDisplay] = useState('');
  const [value, setValue] = useState('');
  const items = props.items ?? [];
  const setItems = (next: ControlListItem[], at: number) => {
    set({ items: next });
    setSel(Math.max(0, Math.min(next.length - 1, at)));
  };
  const pick = (i: number) => {
    setSel(i);
    setDisplay(items[i]?.displayText ?? '');
    setValue(items[i]?.value ?? '');
  };
  const entry = (): ControlListItem => ({
    displayText: display.trim(),
    value: value.trim() || display.trim(),
  });
  const move = (d: number) => {
    const j = sel + d;
    const a = items[sel];
    const b = items[j];
    if (!a || !b) return;
    const next = [...items];
    next[sel] = b;
    next[j] = a;
    setItems(next, j);
  };
  const lid = props.dateLid ?? 'vi-VN';
  return (
    <Dialog
      open
      onOpenChange={(o) => !o && onClose()}
      title={t('controlProps.title')}
      testId="control-props-dialog"
      footer={
        <>
          <button type="button" className={styles.dialogButton} onClick={onClose}>
            {t('security.cancel')}
          </button>
          <button
            type="button"
            className={styles.dialogPrimary}
            data-testid="control-props-ok"
            onClick={() => onApply(props)}
          >
            {t('controlProps.ok')}
          </button>
        </>
      }
    >
      <label className={styles.field}>
        {t('controlProps.name')}
        <input
          data-testid="control-props-title"
          value={props.title}
          onChange={(e) => set({ title: e.target.value })}
        />
      </label>
      <label className={styles.field}>
        {t('controlProps.tag')}
        <input
          data-testid="control-props-tag"
          value={props.tag}
          onChange={(e) => set({ tag: e.target.value })}
        />
      </label>
      <label className={styles.field}>
        {t('controlProps.showAs')}
        <select
          data-testid="control-props-appearance"
          value={props.appearance ?? 'boundingBox'}
          onChange={(e) => set({ appearance: e.target.value as ControlAppearance })}
        >
          <option value="boundingBox">{t('controlProps.showAsBox')}</option>
          <option value="tags">{t('controlProps.showAsTags')}</option>
          <option value="hidden">{t('controlProps.showAsNone')}</option>
        </select>
      </label>
      <div className={styles.field}>
        <span id="cc-color-label">{t('controlProps.color')}</span>
        <span className={styles.ccColorRow}>
          <input
            type="color"
            data-testid="control-props-color"
            aria-labelledby="cc-color-label"
            value={`#${props.color || '5078BE'}`}
            onChange={(e) => set({ color: e.target.value.slice(1).toUpperCase() })}
          />
          <button
            type="button"
            className={styles.dialogButton}
            data-testid="control-props-color-auto"
            aria-pressed={!props.color}
            onClick={() => set({ color: '' })}
          >
            {t('controlProps.colorAuto')}
          </button>
        </span>
      </div>
      <label>
        <input
          type="checkbox"
          data-testid="control-props-temporary"
          checked={props.temporary ?? false}
          onChange={(e) => set({ temporary: e.target.checked })}
        />{' '}
        {t('controlProps.temporary')}
      </label>
      <fieldset className={styles.fieldset}>
        <legend>{t('controlProps.locking')}</legend>
        <label>
          <input
            type="checkbox"
            data-testid="control-props-delete-locked"
            checked={props.deleteLocked}
            onChange={(e) => set({ deleteLocked: e.target.checked })}
          />{' '}
          {t('controlProps.deleteLocked')}
        </label>
        <label>
          <input
            type="checkbox"
            data-testid="control-props-content-locked"
            checked={props.contentLocked}
            onChange={(e) => set({ contentLocked: e.target.checked })}
          />{' '}
          {t('controlProps.contentLocked')}
        </label>
      </fieldset>
      {list && (
        <fieldset className={styles.fieldset}>
          <legend>{t('controlProps.listItems')}</legend>
          <select
            size={5}
            data-testid="control-props-items"
            aria-label={t('controlProps.listItems')}
            className={styles.ccItemList}
            value={String(sel)}
            onChange={(e) => pick(Number(e.target.value))}
          >
            {items.map((it, i) => (
              <option key={`${i}-${it.displayText}`} value={String(i)}>
                {it.displayText === it.value ? it.displayText : `${it.displayText} (${it.value})`}
              </option>
            ))}
          </select>
          <label className={styles.field}>
            {t('controlProps.displayName')}
            <input
              data-testid="control-props-item-display"
              value={display}
              onChange={(e) => setDisplay(e.target.value)}
            />
          </label>
          <label className={styles.field}>
            {t('controlProps.value')}
            <input
              data-testid="control-props-item-value"
              value={value}
              onChange={(e) => setValue(e.target.value)}
            />
          </label>
          <div className={styles.ccItemButtons}>
            <button
              type="button"
              className={styles.dialogButton}
              data-testid="control-props-item-add"
              disabled={display.trim() === ''}
              onClick={() => setItems([...items, entry()], items.length)}
            >
              {t('controlProps.add')}
            </button>
            <button
              type="button"
              className={styles.dialogButton}
              data-testid="control-props-item-modify"
              disabled={display.trim() === '' || !items[sel]}
              onClick={() =>
                setItems(
                  items.map((it, i) => (i === sel ? entry() : it)),
                  sel,
                )
              }
            >
              {t('controlProps.modify')}
            </button>
            <button
              type="button"
              className={styles.dialogButton}
              data-testid="control-props-item-remove"
              disabled={!items[sel]}
              onClick={() =>
                setItems(
                  items.filter((_, i) => i !== sel),
                  sel,
                )
              }
            >
              {t('controlProps.remove')}
            </button>
            <button
              type="button"
              className={styles.dialogButton}
              data-testid="control-props-item-up"
              disabled={sel <= 0}
              onClick={() => move(-1)}
            >
              {t('controlProps.moveUp')}
            </button>
            <button
              type="button"
              className={styles.dialogButton}
              data-testid="control-props-item-down"
              disabled={sel >= items.length - 1}
              onClick={() => move(1)}
            >
              {t('controlProps.moveDown')}
            </button>
          </div>
        </fieldset>
      )}
      {date && (
        <fieldset className={styles.fieldset}>
          <legend>{t('controlProps.dateTitle')}</legend>
          <label className={styles.field}>
            {t('controlProps.locale')}
            <select
              data-testid="control-props-lid"
              value={lid}
              onChange={(e) => {
                const l = e.target.value;
                set({ dateLid: l, dateFormat: DATE_FORMATS[l]?.[0] ?? props.dateFormat });
              }}
            >
              <option value="vi-VN">{t('controlProps.localeVi')}</option>
              <option value="en-US">{t('controlProps.localeEn')}</option>
            </select>
          </label>
          <label className={styles.field}>
            {t('controlProps.dateFormat')}
            <input
              data-testid="control-props-date-format"
              list="cc-date-formats"
              value={props.dateFormat ?? ''}
              onChange={(e) => set({ dateFormat: e.target.value })}
            />
            <datalist id="cc-date-formats">
              {(DATE_FORMATS[lid] ?? []).map((f) => (
                <option key={f} value={f} />
              ))}
            </datalist>
          </label>
        </fieldset>
      )}
    </Dialog>
  );
}
