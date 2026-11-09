import { useEffect, useMemo, useRef, useState } from 'react';
import { flattenRibbon, searchCommands } from './searchRank';
import type { RibbonTab } from './Ribbon';
import styles from './CommandSearch.module.css';

export interface CommandSearchLabels {
  /** Dialog / input accessible name ("Tìm lệnh"). */
  title: string;
  placeholder: string;
  noResults: string;
}

export interface CommandSearchProps {
  tabs: RibbonTab[];
  open: boolean;
  onClose: () => void;
  labels: CommandSearchLabels;
}

/**
 * "Find a command" box (UI-FRAME-003): type part of a command name (accents optional), arrows to
 * choose, Enter to run. Commands come from the ribbon so search, ribbon and shortcuts stay in sync.
 */
export function CommandSearch({ tabs, open, onClose, labels }: CommandSearchProps) {
  const [query, setQuery] = useState('');
  const [active, setActive] = useState(0);
  const input = useRef<HTMLInputElement>(null);
  const entries = useMemo(() => flattenRibbon(tabs), [tabs]);
  const results = useMemo(() => searchCommands(entries, query), [entries, query]);

  useEffect(() => {
    if (open) input.current?.focus();
  }, [open]);

  if (!open) return null;
  const close = () => {
    setQuery('');
    setActive(0);
    onClose();
  };
  const run = (i: number) => {
    const e = results[i];
    if (!e || !e.enabled) return;
    close();
    e.onSelect();
  };
  const current = results[active];
  return (
    <div className={styles.backdrop} onMouseDown={close}>
      <div
        className={styles.box}
        role="dialog"
        aria-label={labels.title}
        data-testid="command-search"
        onMouseDown={(e) => e.stopPropagation()}
      >
        <input
          ref={input}
          className={styles.input}
          role="combobox"
          aria-expanded={results.length > 0}
          aria-controls="bo-command-results"
          aria-activedescendant={current ? `bo-cmd-${current.id}` : undefined}
          aria-label={labels.title}
          placeholder={labels.placeholder}
          value={query}
          data-testid="command-search-input"
          onChange={(e) => {
            setQuery(e.target.value);
            setActive(0);
          }}
          onKeyDown={(e) => {
            if (e.key === 'ArrowDown') {
              e.preventDefault();
              setActive((a) => Math.min(a + 1, results.length - 1));
            } else if (e.key === 'ArrowUp') {
              e.preventDefault();
              setActive((a) => Math.max(a - 1, 0));
            } else if (e.key === 'Enter') {
              e.preventDefault();
              run(active);
            } else if (e.key === 'Escape') {
              e.preventDefault();
              close();
            }
          }}
        />
        {query.trim() && (
          <ul id="bo-command-results" className={styles.list} role="listbox">
            {results.length === 0 && <li className={styles.empty}>{labels.noResults}</li>}
            {results.map((r, i) => (
              <li
                key={r.id}
                id={`bo-cmd-${r.id}`}
                role="option"
                aria-selected={i === active}
                aria-disabled={!r.enabled}
                data-command-result={r.id}
                className={styles.option}
                onMouseEnter={() => setActive(i)}
                onClick={() => run(i)}
              >
                <span className={styles.label}>{r.label}</span>
                <span className={styles.path}>{r.path}</span>
                {r.shortcut && <kbd className={styles.kbd}>{r.shortcut}</kbd>}
              </li>
            ))}
          </ul>
        )}
      </div>
    </div>
  );
}

/** Ctrl+Q or Alt+Q opens the command search (docs/ui/shortcuts.md). */
export function isCommandSearchKey(
  e: Pick<KeyboardEvent, 'ctrlKey' | 'altKey' | 'shiftKey' | 'metaKey' | 'key'>,
): boolean {
  return (e.ctrlKey || e.altKey) && !e.shiftKey && !e.metaKey && e.key.toLowerCase() === 'q';
}

/** Install the Ctrl+Q / Alt+Q listener for the lifetime of the component. */
export function useCommandSearchShortcut(open: () => void) {
  const cb = useRef(open);
  useEffect(() => {
    cb.current = open;
  }, [open]);
  useEffect(() => {
    const f = (e: KeyboardEvent) => {
      if (isCommandSearchKey(e)) {
        e.preventDefault();
        cb.current();
      }
    };
    window.addEventListener('keydown', f, true);
    return () => window.removeEventListener('keydown', f, true);
  }, []);
}
