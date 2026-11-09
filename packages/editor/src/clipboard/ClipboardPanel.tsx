import { useTranslation } from '@blueoffice/i18n';
import type { ReactNode } from 'react';
import styles from './Clipboard.module.css';
import {
  clipboardHistory,
  useClipboardHistory,
  type ClipboardHistory,
  type ClipItem,
} from './history';

/** Preview text of an item (first line, shortened). */
export function previewText(text: string, max = 90): string {
  const line = text.replace(/\r\n?/g, '\n').replace(/\t/g, ' ').trim();
  const one = line.split('\n').filter(Boolean).slice(0, 2).join(' ⏎ ');
  return one.length > max ? `${one.slice(0, max - 1)}…` : one;
}

/**
 * Clipboard history panel shared by BlueDoc, BlueSheet and BlueSlide (CORE-PASTE-001):
 * the items copied in this session, newest first; paste or remove one, or clear all.
 * Non-modal: the document keeps its selection. Escape closes.
 */
export function ClipboardPanel({
  onPaste,
  onClose,
  footer,
  history = clipboardHistory,
}: {
  onPaste: (item: ClipItem) => void;
  onClose: () => void;
  /** Extra content under the list (e.g. BlueDoc's default paste settings). */
  footer?: ReactNode;
  history?: ClipboardHistory;
}) {
  const { t } = useTranslation();
  const items = useClipboardHistory(history);
  return (
    <div
      className={styles.panel}
      role="dialog"
      aria-label={t('clipHistory.title')}
      data-testid="clipboard-panel"
      onKeyDown={(e) => {
        e.stopPropagation();
        if (e.key === 'Escape') {
          e.preventDefault();
          onClose();
        }
      }}
    >
      <div className={styles.header}>
        <strong>{t('clipHistory.title')}</strong>
        <button
          type="button"
          data-testid="clipboard-clear"
          disabled={!items.length}
          onClick={() => history.clear()}
        >
          {t('clipHistory.clear')}
        </button>
        <button type="button" aria-label={t('clipHistory.close')} onClick={onClose}>
          ×
        </button>
      </div>
      <div className={styles.count}>{t('clipHistory.count', { count: items.length, max: 24 })}</div>
      {items.length === 0 ? (
        <div className={styles.empty}>{t('clipHistory.empty')}</div>
      ) : (
        <ul className={styles.list} aria-label={t('clipHistory.items')}>
          {items.map((item) => {
            const preview = previewText(item.text);
            const label = preview || t(`clipHistory.kind.${item.kind}`);
            return (
              <li key={item.id} className={styles.item} data-testid="clipboard-item">
                <button
                  type="button"
                  className={styles.itemPaste}
                  title={t('clipHistory.pasteItem')}
                  aria-label={t('clipHistory.pasteLabel', { text: label })}
                  onClick={() => onPaste(item)}
                >
                  <span className={styles.itemMeta}>
                    {t(`clipHistory.app.${item.app}`)} · {t(`clipHistory.kind.${item.kind}`)}
                  </span>
                  {item.png && !preview ? (
                    <img
                      className={styles.thumb}
                      alt=""
                      src={`data:image/png;base64,${item.png}`}
                    />
                  ) : (
                    <span className={styles.itemText}>{preview}</span>
                  )}
                </button>
                <button
                  type="button"
                  className={styles.itemRemove}
                  aria-label={t('clipHistory.remove', { text: label })}
                  onClick={() => history.remove(item.id)}
                >
                  ×
                </button>
              </li>
            );
          })}
        </ul>
      )}
      {footer}
    </div>
  );
}
