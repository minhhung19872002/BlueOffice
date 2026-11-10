import type { DocStyleEntry } from '@blueoffice/platform';
import { useTranslation } from '@blueoffice/i18n';
import { useEffect, useMemo, useState } from 'react';
import styles from './StylesPane.module.css';

export interface StylesPaneProps {
  /** Paragraph and character styles of the document (`styleList`, ECMA-376 Part 1, §17.7.4.17). */
  load: () => Promise<DocStyleEntry[] | null>;
  /** Style of the paragraph at the caret (`null`: Normal). */
  current: string | null;
  /** Changes when the list may have changed (document opened, style applied). */
  refreshKey: string;
  apply: (id: string) => void;
}

/** Preview size in the list (points, kept readable). */
const previewSize = (pt: number) => Math.min(18, Math.max(10, pt));

/**
 * Styles task pane (CORE-UI-006): every paragraph / character style of the
 * document — quick styles first — drawn in its own formatting; click applies it.
 */
export function StylesPane({ load, current, refreshKey, apply }: StylesPaneProps) {
  const { t } = useTranslation();
  const [list, setList] = useState<DocStyleEntry[] | null>(null);
  const [filter, setFilter] = useState('');
  const [preview, setPreview] = useState(true);

  useEffect(() => {
    let live = true;
    void load().then((l) => {
      if (live) setList(l ?? []);
    });
    return () => {
      live = false;
    };
  }, [load, refreshKey]);

  const shown = useMemo(() => {
    const q = filter.trim().toLocaleLowerCase();
    return (list ?? []).filter((s) => !q || s.name.toLocaleLowerCase().includes(q));
  }, [list, filter]);
  const active = current ?? 'Normal';

  return (
    <div className={styles.pane} data-testid="styles-pane">
      <div className={styles.tools}>
        <input
          type="search"
          className={styles.filter}
          value={filter}
          placeholder={t('taskPane.styles.filter')}
          aria-label={t('taskPane.styles.filter')}
          data-testid="styles-filter"
          onChange={(e) => setFilter(e.target.value)}
          onKeyDown={(e) => e.stopPropagation()}
        />
        <label className={styles.toggle}>
          <input type="checkbox" checked={preview} onChange={(e) => setPreview(e.target.checked)} />
          {t('taskPane.styles.preview')}
        </label>
      </div>
      {list === null ? (
        <p className={styles.note}>{t('taskPane.styles.loading')}</p>
      ) : shown.length === 0 ? (
        <p className={styles.note}>{t('taskPane.styles.empty')}</p>
      ) : (
        <ul className={styles.list} aria-label={t('taskPane.styles.title')}>
          {shown.map((s) => (
            <li key={s.id}>
              <button
                type="button"
                className={styles.item}
                aria-pressed={s.id === active}
                data-testid="style-item"
                data-style-id={s.id}
                title={t(
                  s.kind === 'paragraph'
                    ? 'taskPane.styles.paragraph'
                    : 'taskPane.styles.character',
                )}
                onClick={() => apply(s.id)}
              >
                <span
                  className={styles.name}
                  style={
                    preview
                      ? {
                          fontFamily: `"${s.preview.font}", sans-serif`,
                          fontSize: `${previewSize(s.preview.size)}pt`,
                          fontWeight: s.preview.bold ? 700 : 400,
                          fontStyle: s.preview.italic ? 'italic' : 'normal',
                          textDecoration: s.preview.underline ? 'underline' : 'none',
                          color: `#${s.preview.color}`,
                        }
                      : undefined
                  }
                >
                  {s.name}
                </span>
                <span className={styles.kind} aria-hidden>
                  {s.kind === 'paragraph' ? '¶' : 'a'}
                </span>
              </button>
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}
