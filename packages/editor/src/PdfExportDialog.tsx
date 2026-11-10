/**
 * PDF export choices (CORE-FMT-002): PDF/A level B conformance (ISO 19005-1/-2/-3;
 * 3b embeds the source file), tagged PDF (ISO 32000-1 §14.8) and bookmarks
 * (§12.3.3) from headings, slide titles or sheet names. Shared by BlueDoc,
 * BlueSlide and BlueSheet.
 */
import { useTranslation } from '@blueoffice/i18n';
import type { PdfAPart, PdfExport } from '@blueoffice/platform';
import { Dialog } from '@blueoffice/ui';
import { useState } from 'react';
import styles from './Editor.module.css';

const PARTS: PdfAPart[] = ['1b', '2b', '3b'];

export interface PdfExportDialogProps {
  /** What the bookmarks come from. */
  app: 'doc' | 'slide' | 'sheet';
  onExport: (choices: PdfExport) => void;
  onClose: () => void;
}

export function PdfExportDialog({ app, onExport, onClose }: PdfExportDialogProps) {
  const { t } = useTranslation();
  const [pdfa, setPdfa] = useState<PdfAPart | ''>('2b');
  const [tagged, setTagged] = useState(true);
  const [bookmarks, setBookmarks] = useState(true);
  const submit = () => {
    onClose();
    onExport({ kind: 'pdf', ...(pdfa ? { pdfa } : {}), tagged, bookmarks });
  };
  const bookmarksKey =
    app === 'doc'
      ? 'pdfExport.bookmarks'
      : app === 'slide'
        ? 'pdfExport.bookmarksSlides'
        : 'pdfExport.bookmarksSheets';
  return (
    <Dialog
      open
      onOpenChange={(o) => !o && onClose()}
      title={t('pdfExport.title')}
      width={460}
      testId="pdf-export-dialog"
      footer={
        <>
          <button
            type="button"
            className={styles.dialogButton}
            data-testid="pdf-export-cancel"
            onClick={onClose}
          >
            {t('pdfExport.cancel')}
          </button>
          <button
            type="button"
            className={styles.dialogPrimary}
            data-testid="pdf-export-ok"
            onClick={submit}
          >
            {t('pdfExport.export')}
          </button>
        </>
      }
    >
      <div className={styles.formGrid}>
        <label className={styles.field}>
          <span>{t('pdfExport.conformance')}</span>
          <select
            data-testid="pdf-export-pdfa"
            value={pdfa}
            onChange={(e) => setPdfa(e.target.value as PdfAPart | '')}
          >
            <option value="">{t('pdfExport.none')}</option>
            {PARTS.map((p) => (
              <option key={p} value={p}>
                {t(`pdfExport.part${p}`)}
              </option>
            ))}
          </select>
        </label>
        <label className={styles.field}>
          <input
            type="checkbox"
            data-testid="pdf-export-tagged"
            checked={tagged}
            onChange={(e) => setTagged(e.target.checked)}
          />
          <span>{t('pdfExport.tagged')}</span>
        </label>
        <label className={styles.field}>
          <input
            type="checkbox"
            data-testid="pdf-export-bookmarks"
            checked={bookmarks}
            onChange={(e) => setBookmarks(e.target.checked)}
          />
          <span>{t(bookmarksKey)}</span>
        </label>
        {pdfa === '1b' && <div className={styles.hint}>{t('pdfExport.note1b')}</div>}
      </div>
    </Dialog>
  );
}
