import { useTranslation } from '@blueoffice/i18n';
import { useState } from 'react';
import styles from './Editor.module.css';

/** Password prompt for an encrypted file ([MS-OFFCRYPTO]). */
export function PasswordDialog({
  name,
  wrong,
  onSubmit,
  onCancel,
}: {
  name: string;
  wrong: boolean;
  onSubmit: (password: string) => void;
  onCancel: () => void;
}) {
  const { t } = useTranslation();
  const [pw, setPw] = useState('');
  return (
    <div
      className={styles.panel}
      role="dialog"
      aria-modal="true"
      aria-label={t('security.passwordTitle')}
      data-testid="password-dialog"
      style={{ left: '50%', right: 'auto', top: 80, transform: 'translateX(-50%)' }}
    >
      <strong>{t('security.passwordTitle')}</strong>
      <div>{t('security.passwordPrompt', { name })}</div>
      <div className={styles.panelRow}>
        <input
          autoFocus
          type="password"
          data-testid="password-input"
          aria-label={t('security.password')}
          placeholder={t('security.password')}
          value={pw}
          onChange={(e) => setPw(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === 'Enter' && pw) onSubmit(pw);
            if (e.key === 'Escape') onCancel();
          }}
        />
      </div>
      {wrong && (
        <div className={styles.panelMessage} role="alert" data-testid="password-wrong">
          {t('security.wrongPassword')}
        </div>
      )}
      <div className={styles.panelRow}>
        <button
          type="button"
          data-testid="password-open"
          disabled={!pw}
          onClick={() => onSubmit(pw)}
        >
          {t('security.open')}
        </button>
        <button type="button" onClick={onCancel}>
          {t('security.cancel')}
        </button>
      </div>
    </div>
  );
}

/**
 * Warning shown when a file carries macros ([MS-OVBA] VBA project, XLM macro
 * sheets): they are kept when saving but never run; "View macros" opens the
 * read-only viewer. `inline` places it in the
 * page flow (BlueSheet / BlueSlide) instead of over the document view.
 */
export function MacroBanner({
  onDismiss,
  onView,
  inline,
}: {
  onDismiss: () => void;
  /** Opens the read-only macro viewer (CORE-VBA-002). */
  onView?: () => void;
  inline?: boolean;
}) {
  const { t } = useTranslation();
  return (
    <div
      className={styles.macroBar}
      style={inline ? { position: 'relative' } : undefined}
      role="status"
      data-testid="macro-warning"
    >
      <span>{t('security.macroWarning')}</span>
      {onView && (
        <button type="button" onClick={onView} data-testid="macro-warning-view">
          {t('security.viewMacros')}
        </button>
      )}
      <button type="button" onClick={onDismiss} data-testid="macro-warning-dismiss">
        {t('security.dismiss')}
      </button>
    </div>
  );
}

/** "Save with password": new password twice; empty removes the protection. */
export function SavePasswordPanel({
  onSet,
  onClose,
}: {
  onSet: (password: string) => void;
  onClose: () => void;
}) {
  const { t } = useTranslation();
  const [a, setA] = useState('');
  const [b, setB] = useState('');
  const mismatch = a !== b;
  const set = () => {
    if (mismatch) return;
    onSet(a);
    onClose();
  };
  return (
    <div className={styles.panel} role="dialog" aria-label={t('security.savePasswordTitle')}>
      <strong>{t('security.savePasswordTitle')}</strong>
      <div className={styles.panelMessage}>{t('security.savePasswordHint')}</div>
      <div className={styles.panelRow}>
        <input
          autoFocus
          type="password"
          data-testid="save-password"
          aria-label={t('security.password')}
          placeholder={t('security.password')}
          value={a}
          onChange={(e) => setA(e.target.value)}
        />
      </div>
      <div className={styles.panelRow}>
        <input
          type="password"
          data-testid="save-password-confirm"
          aria-label={t('security.confirm')}
          placeholder={t('security.confirm')}
          value={b}
          onChange={(e) => setB(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === 'Enter') set();
            if (e.key === 'Escape') onClose();
          }}
        />
      </div>
      {mismatch && b && <div className={styles.panelMessage}>{t('security.mismatch')}</div>}
      <div className={styles.panelRow}>
        <button type="button" data-testid="save-password-set" disabled={mismatch} onClick={set}>
          {t('security.set')}
        </button>
        <button type="button" onClick={onClose}>
          {t('security.cancel')}
        </button>
      </div>
    </div>
  );
}
