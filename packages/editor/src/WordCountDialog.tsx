/**
 * Word Count (Review tab, click on the status bar; CORE-UI-007): pages, words, characters
 * and paragraphs of the document — and of the selection when there is one — counted by the
 * engine (`wordCount`), optionally with text boxes, footnotes and endnotes.
 */
import { useTranslation } from '@blueoffice/i18n';
import type { EditCommand, EditResult, WordCountInfo, WordCounts } from '@blueoffice/platform';
import { Dialog } from '@blueoffice/ui';
import { useEffect, useState } from 'react';

import styles from './Editor.module.css';

type Run = (cmd: EditCommand) => Promise<EditResult | null>;

const ROWS: readonly (keyof WordCounts)[] = ['words', 'chars', 'charsWithSpaces', 'paragraphs'];

export function WordCountDialog({ query, onClose }: { query: Run; onClose: () => void }) {
  const { t, i18n } = useTranslation();
  const [notes, setNotes] = useState(false);
  const [info, setInfo] = useState<WordCountInfo | null>(null);

  useEffect(() => {
    let live = true;
    void query({ cmd: 'wordCount', notes }).then((r) => {
      if (live && r?.wordCount) setInfo(r.wordCount);
    });
    return () => {
      live = false;
    };
  }, [query, notes]);

  const n = (v: number) => new Intl.NumberFormat(i18n.language).format(v);
  const sel = info?.selection;
  return (
    <Dialog
      open
      onOpenChange={(o) => !o && onClose()}
      title={t('wordCount.title')}
      testId="word-count-dialog"
      width={sel ? 440 : 360}
      footer={
        <button
          type="button"
          className={styles.dialogPrimary}
          data-testid="word-count-close"
          onClick={onClose}
        >
          {t('wordCount.close')}
        </button>
      }
    >
      <table style={{ width: '100%', borderCollapse: 'collapse' }}>
        {sel && (
          <thead>
            <tr>
              <th />
              <th style={{ textAlign: 'right', fontWeight: 600 }}>{t('wordCount.selection')}</th>
              <th style={{ textAlign: 'right', fontWeight: 600 }}>{t('wordCount.document')}</th>
            </tr>
          </thead>
        )}
        <tbody>
          <tr>
            <th scope="row" style={{ textAlign: 'left', fontWeight: 400, padding: '3px 0' }}>
              {t('wordCount.pages')}
            </th>
            {sel && <td />}
            <td style={{ textAlign: 'right' }} data-testid="word-count-pages">
              {info ? n(info.pages) : '…'}
            </td>
          </tr>
          {ROWS.map((k) => (
            <tr key={k}>
              <th scope="row" style={{ textAlign: 'left', fontWeight: 400, padding: '3px 0' }}>
                {t(`wordCount.${k}`)}
              </th>
              {sel && (
                <td style={{ textAlign: 'right' }} data-testid={`word-count-selection-${k}`}>
                  {n(sel[k])}
                </td>
              )}
              <td style={{ textAlign: 'right' }} data-testid={`word-count-${k}`}>
                {info ? n(info.document[k]) : '…'}
              </td>
            </tr>
          ))}
        </tbody>
      </table>
      <label style={{ display: 'block', marginTop: 12 }}>
        <input
          type="checkbox"
          data-testid="word-count-notes"
          checked={notes}
          onChange={(e) => setNotes(e.target.checked)}
        />{' '}
        {t('wordCount.includeNotes')}
      </label>
    </Dialog>
  );
}
