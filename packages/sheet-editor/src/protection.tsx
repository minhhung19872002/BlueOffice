/**
 * BlueSheet protection dialogs: protect a sheet (password + what stays
 * allowed, ECMA-376 Part 1, §18.3.1.85 `sheetProtection`), protect the
 * workbook structure (§18.2.29 `workbookProtection`) and the password prompt
 * to remove either. Passwords are hashed by the engine (SHA-512, random salt,
 * 100 000 iterations — [MS-OFFCRYPTO] §2.4.2.4).
 */
import { useTranslation } from '@blueoffice/i18n';
import type { SheetPermissions } from '@blueoffice/platform';
import { Dialog } from '@blueoffice/ui';
import { useState } from 'react';
import styles from './SheetEditor.module.css';

/** Options of the protect dialog, in display order (key = i18n suffix). */
const OPTIONS: (keyof SheetPermissions)[] = [
  'selectLockedCells',
  'selectUnlockedCells',
  'formatCells',
  'formatColumns',
  'formatRows',
  'insertColumns',
  'insertRows',
  'deleteColumns',
  'deleteRows',
  'sort',
  'autoFilter',
  'pivotTables',
  'objects',
];

const DEFAULTS: Partial<SheetPermissions> = { selectLockedCells: true, selectUnlockedCells: true };

export function ProtectSheetDialog({
  workbook,
  onProtect,
  onClose,
}: {
  /** Protect the workbook structure instead of a sheet. */
  workbook: boolean;
  onProtect: (password: string, allow: Partial<SheetPermissions>) => void;
  onClose: () => void;
}) {
  const { t } = useTranslation();
  const [pw, setPw] = useState('');
  const [again, setAgain] = useState('');
  const [allow, setAllow] = useState<Partial<SheetPermissions>>(DEFAULTS);
  const mismatch = pw !== again;
  const ok = () => {
    if (mismatch) return;
    onProtect(pw, allow);
    onClose();
  };
  return (
    <Dialog
      open
      onOpenChange={(o) => !o && onClose()}
      title={t(
        workbook ? 'sheetEditor.protection.workbookTitle' : 'sheetEditor.protection.sheetTitle',
      )}
      testId="protect-dialog"
      footer={
        <>
          <button type="button" className={styles.btn} onClick={onClose}>
            {t('sheetEditor.dlg.cancel')}
          </button>
          <button
            type="button"
            className={styles.btnPrimary}
            data-testid="protect-ok"
            disabled={mismatch}
            onClick={ok}
          >
            {t('sheetEditor.dlg.ok')}
          </button>
        </>
      }
    >
      <div className={styles.form}>
        <label>
          {t('sheetEditor.protection.password')}
          <input
            type="password"
            autoFocus
            data-testid="protect-password"
            value={pw}
            onChange={(e) => setPw(e.target.value)}
          />
        </label>
        <label>
          {t('sheetEditor.protection.confirm')}
          <input
            type="password"
            data-testid="protect-confirm"
            value={again}
            onChange={(e) => setAgain(e.target.value)}
            onKeyDown={(e) => e.key === 'Enter' && ok()}
          />
        </label>
        {mismatch && again && <div role="alert">{t('security.mismatch')}</div>}
        {!workbook && (
          <>
            <div>{t('sheetEditor.protection.allow')}</div>
            {OPTIONS.map((k) => (
              <label key={k} className={styles.check}>
                <input
                  type="checkbox"
                  data-testid={`protect-allow-${k}`}
                  checked={!!allow[k]}
                  onChange={(e) => setAllow({ ...allow, [k]: e.target.checked })}
                />
                {t(`sheetEditor.protection.opt.${k}`)}
              </label>
            ))}
          </>
        )}
      </div>
    </Dialog>
  );
}

/** Password prompt to unprotect a sheet or the workbook. */
export function UnprotectDialog({
  workbook,
  wrong,
  onSubmit,
  onClose,
}: {
  workbook: boolean;
  wrong: boolean;
  onSubmit: (password: string) => void;
  onClose: () => void;
}) {
  const { t } = useTranslation();
  const [pw, setPw] = useState('');
  return (
    <Dialog
      open
      onOpenChange={(o) => !o && onClose()}
      title={t(
        workbook
          ? 'sheetEditor.protection.unprotectWorkbook'
          : 'sheetEditor.protection.unprotectSheet',
      )}
      testId="unprotect-dialog"
      footer={
        <>
          <button type="button" className={styles.btn} onClick={onClose}>
            {t('sheetEditor.dlg.cancel')}
          </button>
          <button
            type="button"
            className={styles.btnPrimary}
            data-testid="unprotect-ok"
            onClick={() => onSubmit(pw)}
          >
            {t('sheetEditor.dlg.ok')}
          </button>
        </>
      }
    >
      <div className={styles.form}>
        <label>
          {t('sheetEditor.protection.password')}
          <input
            type="password"
            autoFocus
            data-testid="unprotect-password"
            value={pw}
            onChange={(e) => setPw(e.target.value)}
            onKeyDown={(e) => e.key === 'Enter' && onSubmit(pw)}
          />
        </label>
        {wrong && (
          <div role="alert" data-testid="unprotect-wrong">
            {t('sheetEditor.protection.wrongPassword')}
          </div>
        )}
      </div>
    </Dialog>
  );
}
