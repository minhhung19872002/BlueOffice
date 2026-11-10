import { useEffect, useId, useRef, useState } from 'react';
import {
  insertMention,
  mentionQuery,
  mentionedPeople,
  mentionPieces,
  shiftMentions,
  type MentionDraft,
  type MentionSpan,
  type MentionSuggestion,
} from './mentions';
import styles from './MentionComposer.module.css';

/** UI strings (the caller translates them). */
export interface MentionComposerLabels {
  placeholder: string;
  submit: string;
  /** Checkbox "Assign to" shown once someone is mentioned. */
  assignTo: string;
  /** Picker with no match. */
  noMatches: string;
  /** Accessible name of the picker list. */
  people: string;
}

/** What the composer posts. */
export interface MentionSubmit {
  text: string;
  mentions: MentionSpan[];
  /** Mentioned person the comment assigns a task to. */
  assignee: MentionSuggestion | null;
}

export interface MentionComposerProps {
  labels: MentionComposerLabels;
  /** People matching the text after `@`; absent: no picker (`@` is plain text). */
  search?: (query: string) => Promise<MentionSuggestion[]>;
  onSubmit: (value: MentionSubmit) => void;
  onCancel?: () => void;
  /** `textarea` sent with Ctrl+Enter (default) or one-line `input` sent with Enter. */
  multiline?: boolean;
  autoFocus?: boolean;
  /** `data-testid` of the text box. */
  testId?: string;
  /** `data-testid` of the submit button. */
  buttonTestId?: string;
}

const EMPTY: MentionDraft = { text: '', mentions: [] };

/** Comment text box with an @mention people picker and the "assign to" checkbox. */
export function MentionComposer(props: MentionComposerProps) {
  const { labels, search } = props;
  const [draft, setDraft] = useState<MentionDraft>(EMPTY);
  const [query, setQuery] = useState<{ at: number; query: string } | null>(null);
  // Answer of the last search, for the query it was asked with.
  const [found, setFound] = useState<{ query: string; list: MentionSuggestion[] } | null>(null);
  const [active, setActive] = useState(0);
  const [assign, setAssign] = useState(false);
  const [assigneeKey, setAssigneeKey] = useState('');
  const box = useRef<HTMLTextAreaElement & HTMLInputElement>(null);
  const caretAfter = useRef<number | null>(null);
  const listId = useId();
  const multiline = props.multiline ?? true;
  const options = search && query && found?.query === query.query ? found.list : null;

  // Search the people for the `@query` being typed (debounced; the last answer wins).
  useEffect(() => {
    if (!search || query === null) return;
    let live = true;
    const q = query.query;
    const timer = setTimeout(() => {
      search(q)
        .then((list) => {
          if (live) {
            setFound({ query: q, list });
            setActive(0);
          }
        })
        .catch(() => {
          if (live) setFound({ query: q, list: [] });
        });
    }, 120);
    return () => {
      live = false;
      clearTimeout(timer);
    };
  }, [search, query]);

  useEffect(() => {
    if (caretAfter.current !== null && box.current) {
      box.current.setSelectionRange(caretAfter.current, caretAfter.current);
      caretAfter.current = null;
    }
  });

  const people = mentionedPeople(draft.mentions);
  const keyOf = (p: MentionSuggestion) => p.id || p.email || p.name;
  const assignee = assign
    ? (people.find((p) => keyOf(p) === assigneeKey) ?? people[0] ?? null)
    : null;

  const track = (text: string, caret: number) => {
    setDraft((d) => ({ text, mentions: shiftMentions(d.text, text, d.mentions, caret) }));
    setQuery(search ? mentionQuery(text, caret) : null);
  };

  const pick = (p: MentionSuggestion) => {
    if (!query) return;
    const caret = box.current?.selectionStart ?? draft.text.length;
    const next = insertMention(draft, query.at, caret, p);
    setDraft({ text: next.text, mentions: next.mentions });
    caretAfter.current = next.caret;
    setQuery(null);
    box.current?.focus();
  };

  const send = () => {
    const text = draft.text.trim();
    if (!text) return;
    // Mentions index the text as typed; the engines trim and shift them.
    props.onSubmit({ text: draft.text, mentions: draft.mentions, assignee });
    setDraft(EMPTY);
    setAssign(false);
    setQuery(null);
  };

  const open = query !== null && options !== null;
  const onKeyDown = (e: React.KeyboardEvent) => {
    // Keys stay in the text box (no editor / grid shortcuts).
    e.stopPropagation();
    if (e.nativeEvent.isComposing) return;
    if (open && options.length > 0) {
      if (e.key === 'ArrowDown' || e.key === 'ArrowUp') {
        e.preventDefault();
        const n = options.length;
        setActive((i) => (i + (e.key === 'ArrowDown' ? 1 : n - 1)) % n);
        return;
      }
      if (e.key === 'Enter' || e.key === 'Tab') {
        e.preventDefault();
        const p = options[active];
        if (p) pick(p);
        return;
      }
    }
    if (e.key === 'Escape') {
      if (open) {
        e.preventDefault();
        setQuery(null);
      } else {
        props.onCancel?.();
      }
      return;
    }
    if (e.key === 'Enter' && (!multiline || e.ctrlKey || e.metaKey)) {
      e.preventDefault();
      send();
    }
  };

  const common = {
    ref: box,
    value: draft.text,
    placeholder: labels.placeholder,
    'aria-label': labels.placeholder,
    'data-testid': props.testId,
    autoFocus: props.autoFocus,
    role: 'combobox',
    'aria-expanded': open,
    'aria-controls': listId,
    'aria-autocomplete': 'list' as const,
    onChange: (e: React.ChangeEvent<HTMLTextAreaElement | HTMLInputElement>) =>
      track(e.target.value, e.target.selectionStart ?? e.target.value.length),
    onSelect: (e: React.SyntheticEvent<HTMLTextAreaElement | HTMLInputElement>) => {
      const t = e.currentTarget;
      if (search) setQuery(mentionQuery(t.value, t.selectionStart ?? t.value.length));
    },
    onKeyDown,
    onBlur: () => setQuery(null),
  };

  return (
    <div className={styles.composer}>
      <div className={styles.field}>
        {multiline ? <textarea rows={2} {...common} /> : <input {...common} />}
        {open && (
          <ul
            id={listId}
            role="listbox"
            aria-label={labels.people}
            className={styles.picker}
            data-testid="mention-picker"
          >
            {options.length === 0 && <li className={styles.none}>{labels.noMatches}</li>}
            {options.map((p, i) => (
              <li
                key={keyOf(p)}
                role="option"
                aria-selected={i === active}
                className={i === active ? `${styles.option} ${styles.active}` : styles.option}
                data-testid="mention-option"
                // Keep the focus (and the caret) in the text box.
                onMouseDown={(e) => {
                  e.preventDefault();
                  pick(p);
                }}
              >
                <span className={styles.name}>{p.name}</span>
                {p.email && <span className={styles.email}>{p.email}</span>}
              </li>
            ))}
          </ul>
        )}
      </div>
      {people.length > 0 && (
        <label className={styles.assign}>
          <input
            type="checkbox"
            checked={assign}
            data-testid="mention-assign"
            onChange={(e) => setAssign(e.target.checked)}
          />
          <span>{labels.assignTo}</span>
          {people.length > 1 ? (
            <select
              value={assignee ? keyOf(assignee) : keyOf(people[0]!)}
              aria-label={labels.assignTo}
              onChange={(e) => {
                setAssigneeKey(e.target.value);
                setAssign(true);
              }}
            >
              {people.map((p) => (
                <option key={keyOf(p)} value={keyOf(p)}>
                  @{p.name}
                </option>
              ))}
            </select>
          ) : (
            <strong>@{people[0]!.name}</strong>
          )}
        </label>
      )}
      <button
        type="button"
        onClick={send}
        disabled={!draft.text.trim()}
        data-testid={props.buttonTestId}
      >
        {labels.submit}
      </button>
    </div>
  );
}

/** Comment text with its mentions highlighted. */
export function MentionText(props: { text: string; mentions?: MentionSpan[] | undefined }) {
  return (
    <>
      {mentionPieces(props.text, props.mentions).map((p, i) =>
        p.mention ? (
          <span
            key={i}
            className={styles.mention}
            title={p.mention.email ?? undefined}
            data-testid="comment-mention"
          >
            {p.text}
          </span>
        ) : (
          <span key={i}>{p.text}</span>
        ),
      )}
    </>
  );
}

/** "Assigned to X" line of a comment (the caller formats the label). */
export function CommentTaskLine(props: { label: string; done: boolean }) {
  return (
    <span
      className={styles.task}
      data-testid="comment-task"
      data-done={props.done ? 'true' : 'false'}
    >
      {props.label}
    </span>
  );
}
