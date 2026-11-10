import type {
  CommentInfo,
  EditCommand,
  EditResult,
  PeopleDirectory,
  ReviewState,
  RevisionItem,
} from '@blueoffice/platform';
import { mentionFields, notifyMentions } from '@blueoffice/platform';
import { useTranslation } from '@blueoffice/i18n';
import {
  CommentTaskLine,
  MentionComposer,
  MentionText,
  type MentionComposerLabels,
  type MentionSubmit,
} from '@blueoffice/ui';
import { useState } from 'react';
import styles from './Editor.module.css';

type Run = (cmd: EditCommand) => Promise<EditResult | null>;

/** Short local date of an `ST_DateTime` stamp. */
export function formatStamp(date: string | null, language: string): string {
  if (!date) return '';
  const d = new Date(date);
  return Number.isNaN(d.getTime())
    ? date
    : d.toLocaleString(language, { dateStyle: 'short', timeStyle: 'short' });
}

function RevisionRow({ r, run, language }: { r: RevisionItem; run: Run; language: string }) {
  const { t } = useTranslation();
  return (
    <li className={styles.reviewItem} data-testid="revision">
      <button
        type="button"
        className={styles.reviewItemMain}
        onClick={() => {
          // Revisions in notes open the note; in headers/footers they are listed only.
          if (r.story?.kind === 'footnote' || r.story?.kind === 'endnote') {
            void run({ cmd: 'editNote', endnote: r.story.kind === 'endnote', id: r.story.id });
          } else if (!r.story) {
            void run({ cmd: 'setSelection', selection: { anchor: r.start, focus: r.end } });
          }
        }}
      >
        <span className={styles.reviewKind}>
          {r.story && (
            <span data-testid="revision-story">{t(`review.story.${r.story.kind}`)} · </span>
          )}
          {t(`review.kind.${r.kind}`)}
        </span>
        <span className={styles.reviewMeta}>
          {r.author} · {formatStamp(r.date, language)}
        </span>
        {r.text && <span className={styles.reviewText}>{r.text}</span>}
      </button>
      <span className={styles.reviewActions}>
        <button
          type="button"
          aria-label={t('commands.review.accept')}
          title={t('commands.review.accept')}
          onClick={() => void run({ cmd: 'acceptRevision', id: r.id })}
        >
          ✓
        </button>
        <button
          type="button"
          aria-label={t('commands.review.reject')}
          title={t('commands.review.reject')}
          onClick={() => void run({ cmd: 'rejectRevision', id: r.id })}
        >
          ✕
        </button>
      </span>
    </li>
  );
}

/** Side pane listing tracked changes (ECMA-376 Part 1, §17.13.5). */
export function ReviewPanel({
  review,
  run,
  language,
  onClose,
}: {
  review: ReviewState | null;
  run: Run;
  language: string;
  onClose: () => void;
}) {
  const { t } = useTranslation();
  const revisions = review?.revisions ?? [];
  return (
    <aside className={styles.reviewPane} aria-label={t('review.title')} data-testid="review-pane">
      <div className={styles.reviewHeader}>
        <strong>{t('review.title')}</strong>
        <button type="button" onClick={onClose} aria-label={t('find.close')}>
          ✕
        </button>
      </div>
      <label className={styles.panelRow}>
        <input
          type="checkbox"
          data-testid="review-track"
          checked={!!review?.trackChanges}
          onChange={(e) => void run({ cmd: 'setTrackChanges', on: e.target.checked })}
        />
        {t('commands.review.trackChanges')}
      </label>
      <label className={styles.panelRow}>
        {t('review.view')}
        <select
          value={review?.view ?? 'markup'}
          onChange={(e) =>
            void run({
              cmd: 'setRevisionView',
              view: e.target.value as ReviewState['view'],
            })
          }
        >
          <option value="markup">{t('commands.review.viewMarkup')}</option>
          <option value="final">{t('commands.review.viewFinal')}</option>
          <option value="original">{t('commands.review.viewOriginal')}</option>
        </select>
      </label>
      <div className={styles.panelMessage} data-testid="revision-count">
        {t('review.count', { count: revisions.length })}
      </div>
      <ul className={styles.reviewList}>
        {revisions.map((r) => (
          <RevisionRow
            key={`${r.story?.kind ?? 'body'}-${r.kind}-${r.id}-${r.start.p}-${r.start.o}`}
            r={r}
            run={run}
            language={language}
          />
        ))}
      </ul>
    </aside>
  );
}

/**
 * Posts a comment (`parent` null) or a reply with its @mentions and task, then
 * notifies the people through the hosted session (CORE-COMMENT-001).
 */
async function postComment(
  run: Run,
  people: PeopleDirectory | undefined,
  parent: number | null,
  v: MentionSubmit,
): Promise<void> {
  const fields = mentionFields(v.mentions, v.assignee);
  const r = await run(
    parent === null
      ? { cmd: 'addComment', text: v.text, ...fields }
      : { cmd: 'replyComment', id: parent, text: v.text, ...fields },
  );
  if (r && typeof r.count === 'number') {
    void notifyMentions(people, {
      commentId: String(r.count),
      text: v.text.trim(),
      mentions: v.mentions,
      assignee: v.assignee,
      app: 'doc',
    });
  }
}

/** One comment's text with its mentions, and its task line. */
function CommentBody({ c, done }: { c: CommentInfo; done: boolean }) {
  const { t } = useTranslation();
  return (
    <>
      <span data-testid="comment-text">
        <MentionText text={c.text} mentions={c.mentions} />
      </span>
      {c.task && (
        <CommentTaskLine
          label={t(done ? 'comments.taskDone' : 'comments.assignedTo', {
            name: c.task.assignee.name,
          })}
          done={done}
        />
      )}
    </>
  );
}

function CommentCard({
  c,
  replies,
  run,
  language,
  people,
  labels,
}: {
  c: CommentInfo;
  replies: CommentInfo[];
  run: Run;
  language: string;
  people: PeopleDirectory | undefined;
  labels: (placeholder: string, submit: string) => MentionComposerLabels;
}) {
  const { t } = useTranslation();
  const [replying, setReplying] = useState(false);
  const send = async (v: MentionSubmit) => {
    await postComment(run, people, c.id, v);
    setReplying(false);
  };
  return (
    <li
      className={styles.reviewItem}
      data-testid="comment"
      data-done={c.done ? 'true' : 'false'}
      style={{ flexDirection: 'column', opacity: c.done ? 0.65 : 1 }}
    >
      <button
        type="button"
        className={styles.reviewItemMain}
        onClick={() => void run({ cmd: 'selectComment', id: c.id })}
      >
        <span className={styles.reviewKind}>
          {c.author}
          {c.done ? ` · ${t('comments.resolved')}` : ''}
        </span>
        <span className={styles.reviewMeta}>{formatStamp(c.date, language)}</span>
        {c.quote && <span className={styles.reviewText}>“{c.quote}”</span>}
        <CommentBody c={c} done={c.done} />
      </button>
      {replies.map((r) => (
        <div key={r.id} className={styles.commentReply} data-testid="comment-reply">
          <span className={styles.reviewKind}>{r.author}</span>{' '}
          <span className={styles.reviewMeta}>{formatStamp(r.date, language)}</span>
          <div>
            <CommentBody c={r} done={c.done} />
          </div>
        </div>
      ))}
      {replying && (
        <div className={styles.panelRow}>
          <MentionComposer
            autoFocus
            multiline={false}
            testId="comment-reply-input"
            labels={labels(t('comments.replyPlaceholder'), t('comments.reply'))}
            search={people ? (q) => people.search(q) : undefined}
            onSubmit={(v) => void send(v)}
            onCancel={() => setReplying(false)}
          />
        </div>
      )}
      <span className={styles.commentActions}>
        <button
          type="button"
          data-testid="comment-reply-button"
          aria-pressed={replying}
          onClick={() => setReplying(!replying)}
        >
          {t('comments.reply')}
        </button>
        <button
          type="button"
          data-testid="comment-resolve"
          onClick={() => void run({ cmd: 'resolveComment', id: c.id, done: !c.done })}
        >
          {t(c.done ? 'comments.reopen' : 'comments.resolve')}
        </button>
        <button
          type="button"
          data-testid="comment-delete"
          onClick={() => void run({ cmd: 'deleteComment', id: c.id })}
        >
          {t('comments.delete')}
        </button>
      </span>
    </li>
  );
}

/** Side pane listing comment threads (ECMA-376 Part 1, §17.13.4; [MS-DOCX] replies). */
export function CommentsPanel({
  review,
  run,
  language,
  onClose,
  people,
}: {
  review: ReviewState | null;
  run: Run;
  language: string;
  onClose: () => void;
  /** @mention picker + notifications (hosted sessions; CORE-COMMENT-001). */
  people?: PeopleDirectory | undefined;
}) {
  const { t } = useTranslation();
  const labels = (placeholder: string, submit: string): MentionComposerLabels => ({
    placeholder,
    submit,
    assignTo: t('comments.assignTo'),
    noMatches: t('comments.noMatches'),
    people: t('comments.people'),
  });
  const comments = review?.comments ?? [];
  const byId = new Map(comments.map((c) => [c.id, c]));
  // Thread root of a comment (replies to replies belong to the same thread).
  const root = (c: CommentInfo): number => {
    let cur = c;
    for (let i = 0; i < 64 && cur.parent !== null && byId.has(cur.parent); i++) {
      cur = byId.get(cur.parent)!;
    }
    return cur.id;
  };
  const top = comments.filter((c) => root(c) === c.id);
  const repliesOf = (id: number) => comments.filter((c) => c.id !== id && root(c) === id);

  return (
    <aside
      className={styles.reviewPane}
      aria-label={t('comments.title')}
      data-testid="comments-pane"
    >
      <div className={styles.reviewHeader}>
        <strong>{t('comments.title')}</strong>
        <button type="button" onClick={onClose} aria-label={t('find.close')}>
          ✕
        </button>
      </div>
      <div className={styles.panelRow}>
        <MentionComposer
          autoFocus
          multiline={false}
          testId="comment-input"
          buttonTestId="comment-add"
          labels={labels(t('comments.placeholder'), t('comments.add'))}
          search={people ? (q) => people.search(q) : undefined}
          onSubmit={(v) => void postComment(run, people, null, v)}
          onCancel={onClose}
        />
      </div>
      <div className={styles.panelMessage} data-testid="comment-count">
        {top.length ? t('comments.count', { count: top.length }) : t('comments.none')}
      </div>
      <ul className={styles.reviewList}>
        {top.map((c) => (
          <CommentCard
            key={c.id}
            c={c}
            replies={repliesOf(c.id)}
            run={run}
            language={language}
            people={people}
            labels={labels}
          />
        ))}
      </ul>
    </aside>
  );
}
