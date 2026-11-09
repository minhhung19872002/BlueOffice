import { useTranslation } from '@blueoffice/i18n';
import { useState } from 'react';
import styles from './Clipboard.module.css';
import { loadPasteDefaults, savePasteDefaults, type PasteDefaults } from './docPaste';

/** BlueDoc's default paste modes (same document / other sources / plain text), kept per user. */
export function PasteDefaultsForm({ onChange }: { onChange?: (d: PasteDefaults) => void }) {
  const { t } = useTranslation();
  const [d, setD] = useState<PasteDefaults>(loadPasteDefaults);
  const set = (next: PasteDefaults) => {
    setD(next);
    savePasteDefaults(next);
    onChange?.(next);
  };
  const formatted = (key: 'same' | 'other') => (
    <select
      id={`paste-default-${key}`}
      data-testid={`paste-default-${key}`}
      value={d[key]}
      onChange={(e) => set({ ...d, [key]: e.target.value as PasteDefaults[typeof key] })}
    >
      <option value="keep">{t('pasteOptions.keep')}</option>
      <option value="merge">{t('pasteOptions.merge')}</option>
      <option value="text">{t('pasteOptions.text')}</option>
    </select>
  );
  return (
    <div
      className={styles.defaults}
      data-testid="paste-defaults"
      role="group"
      aria-labelledby="paste-defaults-title"
    >
      <strong id="paste-defaults-title">{t('pasteOptions.defaults')}</strong>
      <label htmlFor="paste-default-same">{t('pasteOptions.fromSame')}</label>
      {formatted('same')}
      <label htmlFor="paste-default-other">{t('pasteOptions.fromOther')}</label>
      {formatted('other')}
      <label htmlFor="paste-default-plain">{t('pasteOptions.fromPlain')}</label>
      <select
        id="paste-default-plain"
        data-testid="paste-default-plain"
        value={d.plain}
        onChange={(e) => set({ ...d, plain: e.target.value as PasteDefaults['plain'] })}
      >
        <option value="text">{t('pasteOptions.text')}</option>
        <option value="link">{t('pasteOptions.linkWhenUrl')}</option>
      </select>
    </div>
  );
}
