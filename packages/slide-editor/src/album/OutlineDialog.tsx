import { Dialog } from '@blueoffice/ui';
import { useMemo } from 'react';
import styles from './album.module.css';
import { outlineSlides, type OutlineEntry } from './outlineImport';

type T = (key: string, opts?: Record<string, unknown>) => string;

/**
 * Slides from an outline: shows what the picked file gives (slide titles and
 * their bullets) before the slides are added after the current one.
 */
export function OutlineDialog({
  t,
  name,
  entries,
  onClose,
  onInsert,
}: {
  t: T;
  /** Name of the picked file. */
  name: string;
  /** Its outline; `null`: the file could not be read. */
  entries: OutlineEntry[] | null;
  onClose: () => void;
  onInsert: (entries: OutlineEntry[]) => void;
}) {
  const slides = useMemo(() => (entries ? outlineSlides(entries) : []), [entries]);
  return (
    <Dialog
      open
      onOpenChange={(o) => !o && onClose()}
      title={t('slideEditor.outlineImport.title')}
      testId="outline-import-dialog"
      width={480}
      footer={
        <>
          <button type="button" onClick={onClose}>
            {t('slideEditor.cancel')}
          </button>
          <button
            type="button"
            data-testid="outline-import-insert"
            disabled={slides.length === 0}
            onClick={() => {
              if (entries) onInsert(entries);
              onClose();
            }}
          >
            {t('slideEditor.outlineImport.insert', { count: slides.length })}
          </button>
        </>
      }
    >
      <p className={styles.note}>{t('slideEditor.outlineImport.from', { name })}</p>
      {entries === null ? (
        <p className={styles.error} role="alert">
          {t('slideEditor.outlineImport.unreadable')}
        </p>
      ) : slides.length === 0 ? (
        <p className={styles.note} role="status">
          {t('slideEditor.outlineImport.empty')}
        </p>
      ) : (
        <>
          <p className={styles.note}>{t('slideEditor.outlineImport.hint')}</p>
          <ol
            className={styles.outlinePreview}
            data-testid="outline-import-preview"
            aria-label={t('slideEditor.outlineImport.preview')}
            // Scrollable with the keyboard.
            tabIndex={0}
          >
            {slides.map((s, i) => (
              <li key={i}>
                <div className={styles.outlineTitle}>
                  {i + 1}. {s.title || t('slideEditor.outlineImport.untitled')}
                </div>
                {s.bullets.map((b, k) => (
                  <div
                    key={k}
                    className={styles.outlineBullet}
                    style={{ paddingInlineStart: 16 + b.level * 14 }}
                  >
                    • {b.text}
                  </div>
                ))}
              </li>
            ))}
          </ol>
        </>
      )}
    </Dialog>
  );
}
