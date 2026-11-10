import type {
  CommentInfo,
  EditCommand,
  EditResult,
  MarkKind,
  Pos,
  ReviewState,
  RevisionItem,
  RevisionKind,
} from '@blueoffice/platform';
import { useTranslation } from '@blueoffice/i18n';
import type { CSSProperties } from 'react';
import { formatStamp } from './ReviewPanel';
import { revisionDetail } from './TrackingControls';
import styles from './ReviewingPane.module.css';

type Run = (cmd: EditCommand) => Promise<EditResult | null>;

/** Group of a revision kind in the markup and the counts (mirrors the engine's mapping). */
export function markKindOf(kind: RevisionKind): MarkKind {
  switch (kind) {
    case 'insert':
    case 'moveTo':
    case 'paragraphInsert':
    case 'rowInsert':
    case 'cellInsert':
      return 'insert';
    case 'delete':
    case 'moveFrom':
    case 'paragraphDelete':
    case 'rowDelete':
    case 'cellDelete':
      return 'delete';
    default:
      return 'format';
  }
}

/** One row of the reviewing pane: a revision or a comment thread. */
export type ReviewEntry =
  | { type: 'revision'; group: MarkKind; item: RevisionItem }
  | { type: 'comment'; group: 'comment'; item: CommentInfo; replies: number };

const before = (a: Pos | null | undefined, b: Pos | null | undefined) => {
  if (!a) return b ? 1 : 0;
  if (!b) return -1;
  return a.p - b.p || a.o - b.o;
};

/**
 * Revisions and comment threads in document order (main text first by position;
 * header, footer and note revisions and unanchored comments after).
 */
export function reviewEntries(review: ReviewState | null): ReviewEntry[] {
  if (!review) return [];
  const rev: ReviewEntry[] = review.revisions.map((item) => ({
    type: 'revision',
    group: markKindOf(item.kind),
    item,
  }));
  const com: ReviewEntry[] = review.comments
    .filter((c) => c.parent === null)
    .map((item) => ({
      type: 'comment',
      group: 'comment',
      item,
      replies: review.comments.filter((r) => r.parent === item.id).length,
    }));
  const pos = (e: ReviewEntry): Pos | null =>
    e.type === 'revision' ? (e.item.story ? null : e.item.start) : e.item.start;
  return [...rev, ...com]
    .map((e, i) => ({ e, i }))
    .sort((a, b) => before(pos(a.e), pos(b.e)) || a.i - b.i)
    .map(({ e }) => e);
}

/** Counts per group. */
export function reviewCounts(entries: ReviewEntry[]): Record<MarkKind, number> {
  const c: Record<MarkKind, number> = { insert: 0, delete: 0, format: 0, comment: 0 };
  for (const e of entries) c[e.group] += 1;
  return c;
}

const GROUPS: MarkKind[] = ['insert', 'delete', 'format', 'comment'];

/**
 * Docked list of every tracked change (ECMA-376 Part 1, §17.13.5) and comment thread
 * (§17.13.4) with author, date, type and text; click to go there, accept or reject
 * changes in place.
 */
export function ReviewingPane({
  review,
  run,
  language,
  dock,
  style,
  onDock,
  onClose,
}: {
  review: ReviewState | null;
  run: Run;
  language: string;
  dock: 'left' | 'bottom';
  /** Room left for the docked task panes (CORE-UI-006). */
  style?: CSSProperties;
  onDock: (dock: 'left' | 'bottom') => void;
  onClose: () => void;
}) {
  const { t } = useTranslation();
  const entries = reviewEntries(review);
  const counts = reviewCounts(entries);
  const go = (e: ReviewEntry) => {
    if (e.type === 'comment') {
      void run({ cmd: 'selectComment', id: e.item.id });
      return;
    }
    const r = e.item;
    if (r.story?.kind === 'footnote' || r.story?.kind === 'endnote') {
      void run({ cmd: 'editNote', endnote: r.story.kind === 'endnote', id: r.story.id });
    } else if (!r.story) {
      void run({ cmd: 'setSelection', selection: { anchor: r.start, focus: r.end } });
    }
  };
  return (
    <aside
      className={`${styles.pane} ${dock === 'left' ? styles.left : styles.bottom}`}
      aria-label={t('markup.title')}
      style={style}
      data-testid="reviewing-pane"
      data-dock={dock}
    >
      <div className={styles.header}>
        <strong>{t('markup.title')}</strong>
        <span data-testid="reviewing-summary">
          {t('markup.summary', { count: entries.length })}
        </span>
        <button
          type="button"
          data-testid="reviewing-dock"
          onClick={() => onDock(dock === 'left' ? 'bottom' : 'left')}
        >
          {t(dock === 'left' ? 'markup.dockBottom' : 'markup.dockLeft')}
        </button>
        <button type="button" onClick={onClose} aria-label={t('find.close')}>
          ✕
        </button>
      </div>
      <div className={styles.counts}>
        {GROUPS.map((g) => (
          <span key={g} data-testid={`reviewing-count-${g}`}>
            {t(`markup.counts.${g}`)}: {counts[g]}
          </span>
        ))}
      </div>
      {entries.length === 0 && <div className={styles.meta}>{t('markup.empty')}</div>}
      <ul className={styles.list}>
        {entries.map((e) => {
          const key =
            e.type === 'comment'
              ? `c-${e.item.id}`
              : `r-${e.item.story?.kind ?? 'body'}-${e.item.kind}-${e.item.id}-${e.item.start.p}-${e.item.start.o}`;
          const kind =
            e.type === 'comment' ? t('markup.kind.comment') : t(`review.kind.${e.item.kind}`);
          const detail = e.type === 'revision' ? revisionDetail(t, e.item) : '';
          const text =
            e.type === 'comment'
              ? e.item.text + (e.replies ? ` (+${e.replies})` : '')
              : e.item.text;
          return (
            <li key={key} className={styles.item} data-testid="reviewing-item" data-kind={e.group}>
              <button type="button" className={styles.main} onClick={() => go(e)}>
                <span className={styles.kind}>{kind}</span>
                {detail && (
                  <span className={styles.meta} data-testid="reviewing-detail">
                    {detail}
                  </span>
                )}
                <span className={styles.meta}>
                  {e.item.author}
                  {e.item.date ? ` · ${formatStamp(e.item.date, language)}` : ''}
                </span>
                {text && (
                  <span className={styles.text} data-testid="reviewing-text">
                    {text}
                  </span>
                )}
              </button>
              {e.type === 'revision' && (
                <span className={styles.actions}>
                  <button
                    type="button"
                    data-testid="reviewing-accept"
                    aria-label={t('commands.review.accept')}
                    title={t('commands.review.accept')}
                    onClick={() => void run({ cmd: 'acceptRevision', id: e.item.id })}
                  >
                    ✓
                  </button>
                  <button
                    type="button"
                    data-testid="reviewing-reject"
                    aria-label={t('commands.review.reject')}
                    title={t('commands.review.reject')}
                    onClick={() => void run({ cmd: 'rejectRevision', id: e.item.id })}
                  >
                    ✕
                  </button>
                </span>
              )}
            </li>
          );
        })}
      </ul>
    </aside>
  );
}
