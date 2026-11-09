/**
 * Legacy form fields (ECMA-376 Part 1, §17.16.17 ffData) and the "filling in forms" editing
 * restriction (§17.15.1.29 documentProtection): the options dialog of a field, the dialog that
 * starts / stops the restriction, and the bar shown while the caret is in a field.
 */
import { useTranslation } from '@blueoffice/i18n';
import type {
  EditCommand,
  EditResult,
  FormFieldData,
  FormFieldInfo,
  FormTextType,
  ProtectionInfo,
} from '@blueoffice/platform';
import { Dialog } from '@blueoffice/ui';
import { useState } from 'react';
import styles from './Editor.module.css';

type Run = (cmd: EditCommand) => Promise<EditResult | null>;

/** Text field types (§17.18 `ST_FFTextType`). */
export const TEXT_TYPES: FormTextType[] = [
  'regular',
  'number',
  'date',
  'currentDate',
  'currentTime',
  'calculated',
];

/** Format suggestions per text field type (§17.16.20: a case, a numeric or a date picture). */
export const FORMAT_SUGGESTIONS: Record<FormTextType, string[]> = {
  regular: ['UPPERCASE', 'LOWERCASE', 'FIRST CAPITAL', 'TITLE CASE'],
  number: ['0', '0.00', '#,##0', '#,##0.00', '0%'],
  date: ['dd/MM/yyyy', 'd/M/yyyy', 'dd-MM-yyyy', "'ngày' d 'tháng' M 'năm' yyyy"],
  currentDate: ['dd/MM/yyyy', 'd/M/yyyy'],
  currentTime: ['HH:mm', 'HH:mm:ss', 'h:mm am/pm'],
  calculated: ['0', '#,##0', '#,##0.00'],
};

/** i18n key of a form error returned by the engine (`forms:…`), or null. */
export function formErrorKey(error: string): string | null {
  const m = /^forms:(protected|number|date|password)$/.exec(error);
  return m ? `formFields.errors.${m[1]}` : null;
}

const lines = (s: string) =>
  s
    .split('\n')
    .map((l) => l.trim())
    .filter((l) => l !== '');

/** Options of the form field at the caret. `onApply` gets the `setFormField` command. */
export function FormFieldDialog({
  info,
  onApply,
  onClose,
}: {
  info: FormFieldInfo;
  onApply: (cmd: EditCommand) => void;
  onClose: () => void;
}) {
  const { t } = useTranslation();
  const [f, setF] = useState<FormFieldData>(info.field);
  const [entries, setEntries] = useState(f.kind === 'dropDown' ? f.entries.join('\n') : '');
  const patch = (p: Partial<FormFieldData>) => setF((x) => ({ ...x, ...p }) as FormFieldData);
  const apply = () => {
    let field: FormFieldData = { ...f, name: f.name?.trim() || null };
    if (field.kind === 'dropDown') {
      const list = lines(entries);
      const keep = (i: number | null) => (i !== null && i < list.length ? i : null);
      field = { ...field, entries: list, result: keep(field.result), default: keep(field.default) };
    }
    onApply({ cmd: 'setFormField', field });
  };
  const help = (key: 'helpText' | 'statusText', label: string, testId: string) => (
    <label className={styles.field}>
      {label}
      <input
        data-testid={testId}
        value={f[key]?.text ?? ''}
        onChange={(e) =>
          patch({ [key]: e.target.value === '' ? null : { autoText: false, text: e.target.value } })
        }
      />
    </label>
  );
  const listId = 'form-format-suggestions';
  return (
    <Dialog
      open
      onOpenChange={(o) => !o && onClose()}
      title={`${t('formFields.title')} · ${t(`formFields.kinds.${f.kind}`)}`}
      testId="form-field-dialog"
      footer={
        <>
          <button type="button" className={styles.dialogButton} onClick={onClose}>
            {t('formFields.cancel')}
          </button>
          <button
            type="button"
            className={styles.dialogPrimary}
            data-testid="form-field-ok"
            onClick={apply}
          >
            {t('formFields.ok')}
          </button>
        </>
      }
    >
      {f.kind === 'text' && (
        <>
          <label className={styles.field}>
            {t('formFields.type')}
            <select
              data-testid="form-text-type"
              value={f.type}
              onChange={(e) => patch({ type: e.target.value as FormTextType })}
            >
              {TEXT_TYPES.map((ty) => (
                <option key={ty} value={ty}>
                  {t(`formFields.types.${ty}`)}
                </option>
              ))}
            </select>
          </label>
          <label className={styles.field}>
            {f.type === 'calculated' ? t('formFields.expression') : t('formFields.default')}
            <input
              data-testid="form-text-default"
              value={f.default}
              placeholder={f.type === 'calculated' ? '=A*B' : ''}
              onChange={(e) => patch({ default: e.target.value })}
            />
          </label>
          <label className={styles.field}>
            {t('formFields.maxLength')}
            <input
              data-testid="form-text-max"
              type="number"
              min={0}
              value={f.maxLength}
              onChange={(e) => patch({ maxLength: Math.max(0, Number(e.target.value) || 0) })}
            />
          </label>
          <label className={styles.field}>
            {t('formFields.format')}
            <input
              data-testid="form-text-format"
              list={listId}
              value={f.format}
              onChange={(e) => patch({ format: e.target.value })}
            />
            <datalist id={listId}>
              {FORMAT_SUGGESTIONS[f.type].map((s) => (
                <option key={s} value={s} />
              ))}
            </datalist>
          </label>
        </>
      )}
      {f.kind === 'checkBox' && (
        <>
          <label className={styles.field}>
            {t('formFields.size')}
            <select
              data-testid="form-check-size-mode"
              value={f.size === null ? 'auto' : 'exact'}
              onChange={(e) => patch({ size: e.target.value === 'auto' ? null : 20 })}
            >
              <option value="auto">{t('formFields.sizeAuto')}</option>
              <option value="exact">{t('formFields.sizeExact')}</option>
            </select>
          </label>
          {f.size !== null && (
            <label className={styles.field}>
              {t('formFields.sizePoints')}
              <input
                data-testid="form-check-size"
                type="number"
                min={1}
                max={1638}
                step={0.5}
                value={f.size / 2}
                onChange={(e) =>
                  patch({ size: Math.max(2, Math.round((Number(e.target.value) || 1) * 2)) })
                }
              />
            </label>
          )}
          <label>
            <input
              type="checkbox"
              data-testid="form-check-default"
              checked={f.default}
              onChange={(e) => patch({ default: e.target.checked })}
            />
            {t('formFields.defaultChecked')}
          </label>
          <label>
            <input
              type="checkbox"
              data-testid="form-check-checked"
              checked={f.checked ?? f.default}
              onChange={(e) => patch({ checked: e.target.checked })}
            />
            {t('formFields.checked')}
          </label>
        </>
      )}
      {f.kind === 'dropDown' && (
        <>
          <label className={styles.field}>
            {t('formFields.entries')}
            <textarea
              data-testid="form-list-entries"
              rows={5}
              value={entries}
              onChange={(e) => setEntries(e.target.value)}
            />
          </label>
          <label className={styles.field}>
            {t('formFields.selected')}
            <select
              data-testid="form-list-selected"
              value={String(f.result ?? f.default ?? 0)}
              onChange={(e) => patch({ result: Number(e.target.value) })}
            >
              {lines(entries).map((v, i) => (
                <option key={`${i}-${v}`} value={String(i)}>
                  {v}
                </option>
              ))}
            </select>
          </label>
        </>
      )}
      <label className={styles.field}>
        {t('formFields.name')}
        <input
          data-testid="form-field-name"
          maxLength={40}
          value={f.name ?? ''}
          onChange={(e) => patch({ name: e.target.value })}
        />
      </label>
      <label>
        <input
          type="checkbox"
          data-testid="form-field-enabled"
          checked={f.enabled}
          onChange={(e) => patch({ enabled: e.target.checked })}
        />
        {t('formFields.enabled')}
      </label>
      <label>
        <input
          type="checkbox"
          data-testid="form-field-calc"
          checked={f.calcOnExit}
          onChange={(e) => patch({ calcOnExit: e.target.checked })}
        />
        {t('formFields.calcOnExit')}
      </label>
      {help('statusText', t('formFields.statusText'), 'form-field-status')}
      {help('helpText', t('formFields.helpText'), 'form-field-help')}
      {(f.entryMacro || f.exitMacro) && (
        <div className={styles.panelMessage} data-testid="form-field-macros">
          {t('formFields.macros', {
            entry: f.entryMacro ?? '—',
            exit: f.exitMacro ?? '—',
          })}
        </div>
      )}
    </Dialog>
  );
}

/**
 * Start the "filling in forms" restriction (optional password, sections left editable) or, when
 * it is on, stop it (its password when it has one).
 */
export function ProtectFormsDialog({
  protection,
  sections,
  onApply,
  onClose,
}: {
  protection: ProtectionInfo | null;
  /** Number of sections of the document. */
  sections: number;
  onApply: (cmd: EditCommand) => void;
  onClose: () => void;
}) {
  const { t } = useTranslation();
  const on = !!protection?.enforced;
  const [password, setPassword] = useState('');
  const [confirm, setConfirm] = useState('');
  const [free, setFree] = useState<number[]>(protection?.unprotectedSections ?? []);
  const mismatch = !on && password !== confirm;
  const submit = () => {
    if (on) onApply({ cmd: 'unprotectDocument', password: password || null });
    else
      onApply({
        cmd: 'protectDocument',
        edit: 'forms',
        password: password || null,
        unprotectedSections: free,
      });
  };
  const toggle = (i: number, keep: boolean) =>
    setFree((f) => (keep ? [...f, i].sort((a, b) => a - b) : f.filter((x) => x !== i)));
  return (
    <Dialog
      open
      onOpenChange={(o) => !o && onClose()}
      title={on ? t('formFields.protect.stopTitle') : t('formFields.protect.title')}
      testId="protect-forms-dialog"
      footer={
        <>
          <button type="button" className={styles.dialogButton} onClick={onClose}>
            {t('formFields.cancel')}
          </button>
          <button
            type="button"
            className={styles.dialogPrimary}
            data-testid="protect-forms-ok"
            disabled={mismatch}
            onClick={submit}
          >
            {on ? t('formFields.protect.stop') : t('formFields.protect.start')}
          </button>
        </>
      }
    >
      <div className={styles.panelMessage}>
        {on ? t('formFields.protect.stopHint') : t('formFields.protect.hint')}
      </div>
      {(!on || protection?.hasPassword) && (
        <label className={styles.field}>
          {t('formFields.protect.password')}
          <input
            type="password"
            data-testid="protect-forms-password"
            value={password}
            onChange={(e) => setPassword(e.target.value)}
          />
        </label>
      )}
      {!on && (
        <label className={styles.field}>
          {t('formFields.protect.confirm')}
          <input
            type="password"
            data-testid="protect-forms-confirm"
            value={confirm}
            onChange={(e) => setConfirm(e.target.value)}
          />
        </label>
      )}
      {mismatch && confirm !== '' && (
        <div className={styles.panelMessage} role="alert">
          {t('formFields.protect.mismatch')}
        </div>
      )}
      {!on && sections > 1 && (
        <fieldset data-testid="protect-forms-sections">
          <legend>{t('formFields.protect.sections')}</legend>
          {Array.from({ length: sections }, (_, i) => (
            <label key={i}>
              <input
                type="checkbox"
                data-testid={`protect-forms-section-${i}`}
                checked={free.includes(i)}
                onChange={(e) => toggle(i, e.target.checked)}
              />
              {t('formFields.protect.section', { n: i + 1 })}
            </label>
          ))}
        </fieldset>
      )}
    </Dialog>
  );
}

/** The form field at the caret: its status text, a list to pick from, a box to tick, Tab moves. */
export function FormFieldBar({
  info,
  protection,
  run,
}: {
  info: FormFieldInfo;
  protection: ProtectionInfo | null;
  run: Run;
}) {
  const { t } = useTranslation();
  const f = info.field;
  const filling = !!protection?.enforced && protection.edit === 'forms';
  const set = (value: string) => void run({ cmd: 'setFormFieldValue', value });
  const status = f.statusText?.text || f.helpText?.text || '';
  return (
    <div
      className={`${styles.hfBar} ${styles.ccBar}`}
      role="toolbar"
      data-testid="form-field-bar"
      aria-label={t('formFields.bar.label')}
    >
      <strong data-testid="form-field-kind">
        {t(`formFields.kinds.${f.kind}`)}
        {f.name ? ` · ${f.name}` : ''}
      </strong>
      {!info.fillable && <span>{t('formFields.bar.readOnly')}</span>}
      {info.fillable && f.kind === 'checkBox' && (
        <button
          type="button"
          data-testid="form-field-toggle"
          aria-pressed={info.value === '☒'}
          onMouseDown={(e) => e.preventDefault()}
          onClick={() => set('')}
        >
          {info.value} {t('formFields.bar.toggle')}
        </button>
      )}
      {info.fillable && f.kind === 'dropDown' && (
        <select
          data-testid="form-field-choose"
          aria-label={t('formFields.bar.choose')}
          value={String(f.result ?? f.default ?? 0)}
          onChange={(e) => set(e.target.value)}
        >
          {f.entries.map((v, i) => (
            <option key={`${i}-${v}`} value={String(i)}>
              {v}
            </option>
          ))}
        </select>
      )}
      {filling && (
        <>
          <button
            type="button"
            data-testid="form-field-previous"
            onMouseDown={(e) => e.preventDefault()}
            onClick={() => void run({ cmd: 'nextFormField', back: true })}
          >
            {t('formFields.bar.previous')}
          </button>
          <button
            type="button"
            data-testid="form-field-next"
            onMouseDown={(e) => e.preventDefault()}
            onClick={() => void run({ cmd: 'nextFormField' })}
          >
            {t('formFields.bar.next')}
          </button>
        </>
      )}
      {status && <span data-testid="form-field-status-text">{status}</span>}
    </div>
  );
}
