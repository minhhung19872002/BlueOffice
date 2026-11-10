import type { SlideReuseSource } from '@blueoffice/platform';
import { useRef, useState } from 'react';
import styles from './album.module.css';

type T = (key: string, opts?: Record<string, unknown>) => string;

/** File types the reuse pane opens. */
export const REUSE_FILES = ['.pptx', '.pptm', '.ppsx', '.ppsm', '.potx', '.potm', '.ppt', '.odp'];

/**
 * Reuse slides (Home › Reuse slides): the slides of another presentation as
 * thumbnails; the chosen ones are inserted after the current slide, with the
 * design of this presentation or — "keep source formatting" — their own.
 *
 * Keyboard: ↑ / ↓ / Home / End move through the slides, Space selects, Enter
 * inserts the selected slides (the focused one when none is selected), Ctrl+A
 * selects all; a double click inserts one slide.
 */
export function ReusePane({
  t,
  source,
  busy,
  error,
  canInsert,
  onOpen,
  onInsert,
  onClose,
  embedded = false,
}: {
  t: T;
  /** The opened presentation, `null` before one is picked. */
  source: SlideReuseSource | null;
  /** A file is being read. */
  busy: boolean;
  /** The picked file could not be read. */
  error: boolean;
  /** A presentation is open and can take slides. */
  canInsert: boolean;
  onOpen: () => void;
  /** Insert slides of the source (ascending indices) after the current slide. */
  onInsert: (indices: number[], keepSource: boolean) => void;
  onClose: () => void;
  /** Shown in a task pane: the host draws the title bar and close button. */
  embedded?: boolean;
}) {
  const [picked, setPicked] = useState<ReadonlySet<number>>(new Set());
  const [focus, setFocus] = useState(0);
  const [keep, setKeep] = useState(false);
  const list = useRef<HTMLUListElement>(null);
  const count = source?.slides.length ?? 0;

  // Another file: nothing is selected yet (state reset while rendering).
  const [shown, setShown] = useState(source);
  if (shown !== source) {
    setShown(source);
    setPicked(new Set());
    setFocus(0);
  }

  const toggle = (i: number) =>
    setPicked((old) => {
      const next = new Set(old);
      if (!next.delete(i)) next.add(i);
      return next;
    });
  const go = (i: number) => {
    const to = Math.max(0, Math.min(count - 1, i));
    setFocus(to);
    list.current?.querySelector<HTMLElement>(`[data-reuse-slide="${to}"]`)?.focus();
  };
  const insert = (indices: number[]) => {
    if (!canInsert || indices.length === 0) return;
    onInsert(
      [...indices].sort((a, b) => a - b),
      keep,
    );
    setPicked(new Set());
  };

  return (
    <div
      className={embedded ? styles.pane : styles.paneStandalone}
      role={embedded ? undefined : 'complementary'}
      aria-label={embedded ? undefined : t('slideEditor.reuse.title')}
      data-testid="reuse-pane"
      onKeyDown={(e) => {
        // Plain keys stay in the pane (they would move or delete shapes); shortcuts
        // with a modifier (save, undo, …) still reach the editor.
        if (e.ctrlKey || e.metaKey || e.altKey) return;
        e.stopPropagation();
        if (e.key === 'Escape') {
          e.preventDefault();
          onClose();
        }
      }}
    >
      {!embedded && (
        <div className={styles.paneHeader}>
          <strong>{t('slideEditor.reuse.title')}</strong>
          <button type="button" aria-label={t('slideEditor.reuse.close')} onClick={onClose}>
            ×
          </button>
        </div>
      )}
      <div className={styles.row}>
        <button type="button" data-testid="reuse-open" disabled={busy} onClick={onOpen}>
          {t('slideEditor.reuse.open')}
        </button>
      </div>
      {busy && (
        <p className={styles.note} role="status">
          {t('slideEditor.reuse.reading')}
        </p>
      )}
      {error && !busy && (
        <p className={styles.error} role="alert">
          {t('slideEditor.reuse.unreadable')}
        </p>
      )}
      {!source && !busy && !error && <p className={styles.note}>{t('slideEditor.reuse.hint')}</p>}
      {source && (
        <>
          <div className={styles.source} data-testid="reuse-source" title={source.name}>
            {t('slideEditor.reuse.source', { name: source.name, count })}
          </div>
          <ul
            className={styles.slides}
            ref={list}
            role="listbox"
            aria-multiselectable
            aria-label={t('slideEditor.reuse.slides')}
            onKeyDown={(e) => {
              if (e.key === 'ArrowDown') go(focus + 1);
              else if (e.key === 'ArrowUp') go(focus - 1);
              else if (e.key === 'Home') go(0);
              else if (e.key === 'End') go(count - 1);
              else if (e.key === ' ') toggle(focus);
              else if (e.key === 'Enter') insert(picked.size ? [...picked] : [focus]);
              else if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 'a')
                setPicked(new Set(source.slides.map((_, i) => i)));
              else return;
              e.preventDefault();
              e.stopPropagation();
            }}
          >
            {source.slides.map((s, i) => (
              <li
                key={i}
                className={styles.slide}
                role="option"
                aria-selected={picked.has(i)}
                tabIndex={i === focus ? 0 : -1}
                data-reuse-slide={i}
                data-testid={`reuse-slide-${i}`}
                onFocus={() => setFocus(i)}
                onClick={() => {
                  setFocus(i);
                  toggle(i);
                }}
                onDoubleClick={() => insert([i])}
              >
                {s.png ? (
                  <img
                    className={styles.slideThumb}
                    src={`data:image/png;base64,${s.png}`}
                    alt=""
                  />
                ) : (
                  <span className={styles.slideThumbEmpty} aria-hidden />
                )}
                <span className={styles.slideLabel}>
                  <span className={styles.slideNumber}>
                    {t('slideEditor.reuse.slideN', { n: i + 1 })}
                  </span>
                  {s.title}
                </span>
              </li>
            ))}
          </ul>
          <label className={styles.row}>
            <input
              type="checkbox"
              data-testid="reuse-keep-source"
              checked={keep}
              onChange={(e) => setKeep(e.currentTarget.checked)}
            />
            {t('slideEditor.reuse.keepSource')}
          </label>
          <div className={styles.row}>
            <button
              type="button"
              data-testid="reuse-insert"
              disabled={!canInsert || picked.size === 0}
              onClick={() => insert([...picked])}
            >
              {t('slideEditor.reuse.insert', { count: picked.size })}
            </button>
            <button
              type="button"
              data-testid="reuse-insert-all"
              disabled={!canInsert || count === 0}
              onClick={() => insert(source.slides.map((_, i) => i))}
            >
              {t('slideEditor.reuse.insertAll')}
            </button>
          </div>
        </>
      )}
    </div>
  );
}
