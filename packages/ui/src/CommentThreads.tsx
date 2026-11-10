import styles from './CommentThreads.module.css';
import {
  CommentTaskLine,
  MentionComposer,
  MentionText,
  type MentionSubmit,
} from './MentionComposer';
import type { MentionSpan, MentionSuggestion } from './mentions';

/** One comment of a thread (the first is the thread's root). */
export interface ThreadComment {
  id: string;
  author: string;
  /** ISO 8601 date-time, or empty. */
  date: string;
  text: string;
  /** @mentions in `text`. */
  mentions?: MentionSpan[] | undefined;
  /** Task assigned in the comment. */
  task?: { assignee: { name: string } } | null | undefined;
}

/** A comment thread anchored somewhere in the document (a cell, a slide). */
export interface CommentThread {
  id: string;
  /** Where the thread is anchored, shown above it ("B4", "Slide 2"). */
  anchor: string;
  done: boolean;
  comments: ThreadComment[];
}

/** UI strings (the caller translates them). */
export interface CommentThreadsLabels {
  title: string;
  empty: string;
  newComment: string;
  newPlaceholder: string;
  reply: string;
  replyPlaceholder: string;
  resolve: string;
  reopen: string;
  delete: string;
  close: string;
  resolved: string;
  /** Composer checkbox "Assign to". */
  assignTo: string;
  /** Picker without a match. */
  noMatches: string;
  /** Accessible name of the picker. */
  people: string;
  /** "Assigned to {name}" (`{name}` is replaced). */
  assignedTo: string;
  /** "Assigned to {name} · done" for a resolved thread. */
  taskDone: string;
}

export interface CommentThreadsProps {
  threads: CommentThread[];
  labels: CommentThreadsLabels;
  /** Anchor of a new thread (current cell / slide); `null` disables adding. */
  newAnchor: string | null;
  onAdd: (text: string, extra: MentionSubmit) => void;
  onReply: (threadId: string, text: string, extra: MentionSubmit) => void;
  /** People for the @mention picker (hosted sessions); absent: no picker. */
  searchPeople?: ((query: string) => Promise<MentionSuggestion[]>) | undefined;
  onResolve: (threadId: string, done: boolean) => void;
  onDelete: (threadId: string) => void;
  onSelect?: (threadId: string) => void;
  onClose: () => void;
}

const when = (iso: string) => {
  const d = iso ? new Date(iso) : null;
  return d && !Number.isNaN(d.getTime()) ? d.toLocaleString() : '';
};

/** Side panel listing comment threads with reply / resolve / delete. */
export function CommentThreads(props: CommentThreadsProps) {
  const { labels } = props;
  const composerLabels = (placeholder: string, submit: string) => ({
    placeholder,
    submit,
    assignTo: labels.assignTo,
    noMatches: labels.noMatches,
    people: labels.people,
  });
  return (
    <aside className={styles.panel} aria-label={labels.title} data-testid="comment-threads">
      <div className={styles.head}>
        <strong>{labels.title}</strong>
        <button type="button" onClick={props.onClose} aria-label={labels.close}>
          ×
        </button>
      </div>
      {props.newAnchor !== null && (
        <div className={styles.thread}>
          <div className={styles.anchor}>
            {labels.newComment} · {props.newAnchor}
          </div>
          <MentionComposer
            labels={composerLabels(labels.newPlaceholder, labels.newComment)}
            search={props.searchPeople}
            onSubmit={(v) => props.onAdd(v.text, v)}
          />
        </div>
      )}
      {props.threads.length === 0 && <p className={styles.empty}>{labels.empty}</p>}
      {props.threads.map((th) => (
        <section
          key={th.id}
          className={th.done ? `${styles.thread} ${styles.done}` : styles.thread}
          data-testid="comment-thread"
          onClick={() => props.onSelect?.(th.id)}
        >
          <div className={styles.anchor}>
            {th.anchor}
            {th.done && <span> · {labels.resolved}</span>}
          </div>
          {th.comments.map((c) => (
            <div key={c.id} className={styles.comment}>
              <div className={styles.meta}>
                <span className={styles.author}>{c.author}</span>
                <span>{when(c.date)}</span>
              </div>
              <div className={styles.text}>
                <MentionText text={c.text} mentions={c.mentions} />
              </div>
              {c.task && (
                <CommentTaskLine
                  label={(th.done ? labels.taskDone : labels.assignedTo).replace(
                    '{name}',
                    c.task.assignee.name,
                  )}
                  done={th.done}
                />
              )}
            </div>
          ))}
          <div className={styles.actions}>
            <button type="button" onClick={() => props.onResolve(th.id, !th.done)}>
              {th.done ? labels.reopen : labels.resolve}
            </button>
            <button type="button" onClick={() => props.onDelete(th.id)}>
              {labels.delete}
            </button>
          </div>
          {!th.done && (
            <MentionComposer
              labels={composerLabels(labels.replyPlaceholder, labels.reply)}
              search={props.searchPeople}
              onSubmit={(v) => props.onReply(th.id, v.text, v)}
            />
          )}
        </section>
      ))}
    </aside>
  );
}
