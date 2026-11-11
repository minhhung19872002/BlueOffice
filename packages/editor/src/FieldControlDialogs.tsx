import { useTranslation } from '@blueoffice/i18n';
import type { ControlInfo, ControlProps, MergePrompt } from '@blueoffice/platform';
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

/** Content control properties: title, tag and the two locks (§17.5.2.1, §17.5.2.42, §17.5.2.23). */
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
  const [props, setProps] = useState<ControlProps>({
    title: control.title ?? '',
    tag: control.tag ?? '',
    deleteLocked: control.deleteLocked,
    contentLocked: control.locked,
  });
  const set = (p: Partial<ControlProps>) => setProps({ ...props, ...p });
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
    </Dialog>
  );
}
