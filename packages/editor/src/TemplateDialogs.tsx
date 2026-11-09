import { useTranslation } from '@blueoffice/i18n';
import type { AdminFormatReport, EditCommand, EditResult } from '@blueoffice/platform';
import { Dialog } from '@blueoffice/ui';
import { useCallback, useEffect, useState } from 'react';
import styles from './Editor.module.css';
import tpl from './TemplateDialogs.module.css';

type Run = (cmd: EditCommand) => Promise<EditResult | null>;

/**
 * Built-in templates (`bo_engine::templates::TEMPLATES`): Vietnamese administrative
 * documents of Nghị định 30/2020/NĐ-CP. The engine builds them; the committed copies
 * and their thumbnails (drawn by the BlueOffice renderer) live in `assets/templates/doc/`
 * (`bo-cli templates`).
 */
export const TEMPLATES = [
  'cong-van',
  'quyet-dinh',
  'to-trinh',
  'bien-ban',
  'hop-dong',
  'bao-cao',
  'giay-moi',
  'ke-hoach',
] as const;

export type TemplateId = (typeof TEMPLATES)[number];

/** First-page thumbnails (literal URLs so the bundler emits each file). */
export const TEMPLATE_THUMBNAILS: Record<TemplateId, string> = {
  'cong-van': new URL('../../../assets/templates/doc/cong-van.png', import.meta.url).href,
  'quyet-dinh': new URL('../../../assets/templates/doc/quyet-dinh.png', import.meta.url).href,
  'to-trinh': new URL('../../../assets/templates/doc/to-trinh.png', import.meta.url).href,
  'bien-ban': new URL('../../../assets/templates/doc/bien-ban.png', import.meta.url).href,
  'hop-dong': new URL('../../../assets/templates/doc/hop-dong.png', import.meta.url).href,
  'bao-cao': new URL('../../../assets/templates/doc/bao-cao.png', import.meta.url).href,
  'giay-moi': new URL('../../../assets/templates/doc/giay-moi.png', import.meta.url).href,
  'ke-hoach': new URL('../../../assets/templates/doc/ke-hoach.png', import.meta.url).href,
};

/** "File → New from template": gallery of the built-in templates. */
export function TemplateDialog({
  onCreate,
  onClose,
}: {
  /** Create an untitled document from template `id`, named `name`. */
  onCreate: (id: TemplateId, name: string) => void;
  onClose: () => void;
}) {
  const { t } = useTranslation();
  return (
    <Dialog
      open
      onOpenChange={(o) => !o && onClose()}
      title={t('templates.title')}
      testId="templates"
      width={760}
      footer={
        <button type="button" className={styles.dialogButton} onClick={onClose}>
          {t('security.cancel')}
        </button>
      }
    >
      <p className={tpl.intro}>{t('templates.intro')}</p>
      <div className={tpl.gallery} role="list">
        {TEMPLATES.map((id) => (
          <button
            key={id}
            type="button"
            role="listitem"
            className={tpl.card}
            data-testid={`template-${id}`}
            title={t(`templates.${id}.description`)}
            onClick={() => onCreate(id, t(`templates.${id}.file`))}
          >
            <img src={TEMPLATE_THUMBNAILS[id]} alt="" className={tpl.thumb} loading="lazy" />
            <strong>{t(`templates.${id}.name`)}</strong>
          </button>
        ))}
      </div>
    </Dialog>
  );
}

/** A value of the report: a token (`yes`, `center`, `headerRight`…) or a measurement. */
function useValue() {
  const { t } = useTranslation();
  // Font names and measurements are shown as they are.
  return (v: string) =>
    /^[a-zA-Z]+$/.test(v) ? t(`adminFormat.values.${v}`, { defaultValue: v }) : v;
}

/** Review → Check administrative format (Nghị định 30/2020): the deviations found. */
export function AdminFormatDialog({ run, onClose }: { run: Run; onClose: () => void }) {
  const { t } = useTranslation();
  const value = useValue();
  const [report, setReport] = useState<AdminFormatReport | null>(null);
  const check = useCallback(() => {
    void run({ cmd: 'checkAdminFormat' }).then((r) => setReport(r?.adminFormat ?? null));
  }, [run]);
  useEffect(check, [check]);
  const issues = report?.issues ?? [];
  return (
    <Dialog
      open
      onOpenChange={(o) => !o && onClose()}
      title={t('adminFormat.title')}
      testId="admin-format"
      width={620}
      footer={
        <>
          <button
            type="button"
            className={styles.dialogButton}
            data-testid="admin-format-recheck"
            onClick={check}
          >
            {t('adminFormat.recheck')}
          </button>
          <button type="button" className={styles.dialogButton} onClick={onClose}>
            {t('adminFormat.close')}
          </button>
        </>
      }
    >
      {!report && <p>{t('adminFormat.checking')}</p>}
      {report && (
        <p data-testid="admin-format-summary" className={tpl.intro}>
          {issues.length === 0
            ? t('adminFormat.ok')
            : t('adminFormat.summary', { n: issues.length })}
        </p>
      )}
      {issues.length > 0 && (
        <ul className={tpl.issues} data-testid="admin-format-issues">
          {issues.map((i) => (
            <li key={`${i.id}-${i.section ?? ''}`} data-issue={i.id} data-severity={i.severity}>
              <button
                type="button"
                className={tpl.issue}
                disabled={i.para === undefined}
                onClick={() => {
                  if (i.para === undefined) return;
                  const at = { p: i.para, o: 0 };
                  void run({ cmd: 'setSelection', selection: { anchor: at, focus: at } });
                }}
              >
                <strong>
                  {t(`adminFormat.components.${i.component}`)} —{' '}
                  {t(`adminFormat.props.${i.property}`)}
                  {i.section !== undefined && ` (${t('hf.section', { n: i.section + 1 })})`}
                  {i.count > 1 && ` ×${i.count}`}
                </strong>
                <span>
                  {t('adminFormat.expected', { value: value(i.expected) })} ·{' '}
                  {t('adminFormat.actual', { value: value(i.actual) })}
                </span>
                {i.text && <em>{i.text}</em>}
              </button>
            </li>
          ))}
        </ul>
      )}
    </Dialog>
  );
}
