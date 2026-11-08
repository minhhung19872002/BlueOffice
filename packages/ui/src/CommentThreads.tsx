import { useState } from 'react';
import styles from './CommentThreads.module.css';

/** One comment of a thread (the first is the thread's root). */
export interface ThreadComment {
  id: string;
  author: string;
  /** ISO 8601 date-time, or empty. */
  date: string;
  text: string;
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
}

export interface CommentThreadsProps {
  threads: CommentThread[];
  labels: CommentThreadsLabels;
  /** Anchor of a new thread (current cell / slide); `null` disables adding. */
  newAnchor: string | null;
  onAdd: (text: string) => void;
  onReply: (threadId: string, text: string) => void;
  onResolve: (threadId: string, done: boolean) => void;
  onDelete: (threadId: string) => void;
  onSelect?: (threadId: string) => void;
  onClose: () => void;
}

function Composer(props: { placeholder: string; submit: string; onSubmit: (t: string) => void }) {
  const [text, setText] = useState('');
  const send = () => {
    const v = text.trim();
    if (!v) return;
    props.onSubmit(v);
    setText('');
  };
  return (
    <div className={styles.composer}>
      <textarea
        value={text}
        rows={2}
        placeholder={props.placeholder}
        aria-label={props.placeholder}
        onChange={(e) => setText(e.target.value)}
        onKeyDown={(e) => {
          // Ctrl+Enter sends; plain keys stay in the text box (no grid shortcuts).
          e.stopPropagation();
          if (e.key === 'Enter' && (e.ctrlKey || e.metaKey) && !e.nativeEvent.isComposing) {
            e.preventDefault();
            send();
          }
        }}
      />
      <button type="button" onClick={send} disabled={!text.trim()}>
        {props.submit}
      </button>
    </div>
  );
}

const when = (iso: string) => {
  const d = iso ? new Date(iso) : null;
  return d && !Number.isNaN(d.getTime()) ? d.toLocaleString() : '';
};

/** Side panel listing comment threads with reply / resolve / delete. */
export function CommentThreads(props: CommentThreadsProps) {
  const { labels } = props;
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
          <Composer
            placeholder={labels.newPlaceholder}
            submit={labels.newComment}
            onSubmit={props.onAdd}
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
              <div className={styles.text}>{c.text}</div>
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
            <Composer
              placeholder={labels.replyPlaceholder}
              submit={labels.reply}
              onSubmit={(t) => props.onReply(th.id, t)}
            />
          )}
        </section>
      ))}
    </aside>
  );
}
