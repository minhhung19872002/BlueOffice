import type { EditCommand, EditResult, MergeState, RefTarget } from '@blueoffice/platform';
import { useTranslation } from '@blueoffice/i18n';
import { useEffect, useState } from 'react';
import styles from './Editor.module.css';

type Run = (cmd: EditCommand) => Promise<EditResult | null>;

/** Insert caption dialog: label (SEQ identifier, §17.16.5.56) and text. */
export function CaptionPanel({ run, onClose }: { run: Run; onClose: () => void }) {
  const { t } = useTranslation();
  const labels = [t('references.figure'), t('references.table'), t('references.equation')];
  const [label, setLabel] = useState(labels[0]!);
  const [text, setText] = useState('');
  const insert = async () => {
    await run({ cmd: 'insertCaption', label, text });
    onClose();
  };
  return (
    <div className={styles.panel} role="dialog" aria-label={t('references.captionTitle')}>
      <strong>{t('references.captionTitle')}</strong>
      <label className={styles.panelRow}>
        {t('references.label')}
        <select
          data-testid="caption-label"
          value={label}
          onChange={(e) => setLabel(e.target.value)}
        >
          {labels.map((l) => (
            <option key={l} value={l}>
              {l}
            </option>
          ))}
        </select>
      </label>
      <div className={styles.panelRow}>
        <input
          autoFocus
          data-testid="caption-text"
          aria-label={t('references.captionText')}
          placeholder={t('references.captionText')}
          value={text}
          onChange={(e) => setText(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === 'Enter') void insert();
            if (e.key === 'Escape') onClose();
          }}
        />
      </div>
      <div className={styles.panelRow}>
        <button type="button" data-testid="caption-insert" onClick={() => void insert()}>
          {t('references.insert')}
        </button>
        <button type="button" onClick={onClose}>
          {t('references.cancel')}
        </button>
      </div>
    </div>
  );
}

/** Mail-merge field picker (MERGEFIELD, ECMA-376 Part 1, §17.16.5.35). */
export function MergeFieldPanel({
  merge,
  run,
  onClose,
}: {
  merge: MergeState | null;
  run: Run;
  onClose: () => void;
}) {
  const { t } = useTranslation();
  return (
    <div className={styles.panel} role="dialog" aria-label={t('merge.title')}>
      <strong>{t('merge.title')}</strong>
      {merge ? (
        <>
          <div className={styles.panelMessage} data-testid="merge-source">
            {t('merge.source', { name: merge.name, count: merge.count })}
          </div>
          <div className={styles.panelRow}>
            {merge.fields.map((f) => (
              <button
                key={f}
                type="button"
                data-testid="merge-field"
                onClick={() => void run({ cmd: 'mergeInsertField', name: f })}
              >
                {f}
              </button>
            ))}
          </div>
        </>
      ) : (
        <div className={styles.panelMessage}>{t('merge.noData')}</div>
      )}
      <div className={styles.panelRow}>
        <button type="button" onClick={onClose}>
          {t('merge.close')}
        </button>
      </div>
    </div>
  );
}

/** Cross-reference dialog: bookmark or caption target, text (REF) or page (PAGEREF). */
export function CrossRefPanel({ run, onClose }: { run: Run; onClose: () => void }) {
  const { t } = useTranslation();
  const [targets, setTargets] = useState<RefTarget[]>([]);
  const [target, setTarget] = useState('');
  const [page, setPage] = useState(false);
  useEffect(() => {
    let live = true;
    void run({ cmd: 'crossReferenceTargets' }).then((r) => {
      if (!live) return;
      const list = r?.targets ?? [];
      setTargets(list);
      setTarget(list[0]?.id ?? '');
    });
    return () => {
      live = false;
    };
  }, [run]);
  const insert = async () => {
    if (!target) return;
    await run({ cmd: 'insertCrossReference', target, page });
    onClose();
  };
  return (
    <div className={styles.panel} role="dialog" aria-label={t('references.crossRefTitle')}>
      <strong>{t('references.crossRefTitle')}</strong>
      {targets.length === 0 ? (
        <div className={styles.panelMessage}>{t('references.noTargets')}</div>
      ) : (
        <label className={styles.panelRow}>
          {t('references.target')}
          <select
            data-testid="crossref-target"
            value={target}
            onChange={(e) => setTarget(e.target.value)}
          >
            {targets.map((x) => (
              <option key={x.id} value={x.id}>
                {x.label}
              </option>
            ))}
          </select>
        </label>
      )}
      <div className={styles.panelRow}>
        <label>
          <input type="radio" checked={!page} onChange={() => setPage(false)} />{' '}
          {t('references.refText')}
        </label>
        <label>
          <input type="radio" checked={page} onChange={() => setPage(true)} />{' '}
          {t('references.refPage')}
        </label>
      </div>
      <div className={styles.panelRow}>
        <button
          type="button"
          data-testid="crossref-insert"
          disabled={!target}
          onClick={() => void insert()}
        >
          {t('references.insert')}
        </button>
        <button type="button" onClick={onClose}>
          {t('references.cancel')}
        </button>
      </div>
    </div>
  );
}
