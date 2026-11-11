import { useEffect, useMemo, useRef, useState } from 'react';
import {
  flattenRibbon,
  loadRecentCommands,
  pushRecentCommand,
  relatedCommands,
  searchCommands,
  type SearchEntry,
} from './searchRank';
import type { RibbonTab } from './Ribbon';
import styles from './CommandSearch.module.css';

export interface CommandSearchLabels {
  /** Dialog / input accessible name ("Tìm lệnh"). */
  title: string;
  placeholder: string;
  noResults: string;
  /** Heading of the recently run commands (shown before typing); omitted: no recents. */
  recent?: string;
  /** Heading of the commands next to the best match (same ribbon group). */
  related?: string;
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
  const [recentIds, setRecentIds] = useState<string[]>(() => loadRecentCommands());
  // Suggestions: recent commands before typing; related ones after the matches.
  const sections = useMemo(() => {
    if (!query.trim()) {
      const byId = new Map(entries.map((e) => [e.id, e]));
      const recent = recentIds.flatMap((id) => byId.get(id) ?? []).slice(0, 6);
      return labels.recent && recent.length
        ? [{ key: 'recent', title: labels.recent, items: recent }]
        : [];
    }
    const found = searchCommands(entries, query);
    const out: { key: string; title?: string; items: SearchEntry[] }[] = [
      { key: 'results', items: found },
    ];
    const related = labels.related ? relatedCommands(entries, found) : [];
    if (related.length) out.push({ key: 'related', title: labels.related, items: related });
    return out;
  }, [entries, query, recentIds, labels.recent, labels.related]);
  const results = useMemo(() => sections.flatMap((x) => x.items), [sections]);

  useEffect(() => {
    if (open) input.current?.focus();
  }, [open]);

  // Escape closes even when the host moved the focus back to its own input meanwhile.
  const closeRef = useRef(onClose);
  useEffect(() => {
    closeRef.current = onClose;
  }, [onClose]);
  useEffect(() => {
    if (!open) return;
    const f = (e: KeyboardEvent) => {
      if (e.key !== 'Escape') return;
      e.preventDefault();
      e.stopPropagation();
      setQuery('');
      setActive(0);
      closeRef.current();
    };
    window.addEventListener('keydown', f, true);
    return () => window.removeEventListener('keydown', f, true);
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
    setRecentIds(pushRecentCommand(e.id));
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
        {(query.trim() || results.length > 0) && (
          <ul id="bo-command-results" className={styles.list} role="listbox">
            {query.trim() && results.length === 0 && (
              <li className={styles.empty}>{labels.noResults}</li>
            )}
            {sections.map((sec) => {
              const base = results.indexOf(sec.items[0]!);
              return [
                sec.title && sec.items.length > 0 ? (
                  <li
                    key={`h-${sec.key}`}
                    role="presentation"
                    className={styles.heading}
                    data-section={sec.key}
                  >
                    {sec.title}
                  </li>
                ) : null,
                ...sec.items.map((r, k) => {
                  const i = base + k;
                  return (
                    <li
                      key={`${sec.key}-${r.id}`}
                      id={`bo-cmd-${r.id}`}
                      role="option"
                      aria-selected={i === active}
                      aria-disabled={!r.enabled}
                      data-command-result={r.id}
                      data-section={sec.key}
                      className={styles.option}
                      onMouseEnter={() => setActive(i)}
                      onClick={() => run(i)}
                    >
                      <span className={styles.label}>{r.label}</span>
                      <span className={styles.path}>{r.path}</span>
                      {r.shortcut && <kbd className={styles.kbd}>{r.shortcut}</kbd>}
                    </li>
                  );
                }),
              ];
            })}
          </ul>
        )}
      </div>
    </div>
  );
}

/** Ctrl+Q or Alt+Q opens the command search (docs/ui/shortcuts.md); ctrl: false leaves Ctrl+Q
 * to the app (BlueDoc: remove paragraph formatting, as in word processors). */
export function isCommandSearchKey(
  e: Pick<KeyboardEvent, 'ctrlKey' | 'altKey' | 'shiftKey' | 'metaKey' | 'key'>,
  ctrl = true,
): boolean {
  return (
    ((ctrl && e.ctrlKey) || (e.altKey && !e.ctrlKey)) &&
    !e.shiftKey &&
    !e.metaKey &&
    e.key.toLowerCase() === 'q'
  );
}

/** Install the Ctrl+Q / Alt+Q listener for the lifetime of the component. */
export function useCommandSearchShortcut(open: () => void, opts: { ctrl?: boolean } = {}) {
  const ctrl = opts.ctrl ?? true;
  const cb = useRef(open);
  useEffect(() => {
    cb.current = open;
  }, [open]);
  useEffect(() => {
    const f = (e: KeyboardEvent) => {
      if (isCommandSearchKey(e, ctrl)) {
        e.preventDefault();
        cb.current();
      }
    };
    window.addEventListener('keydown', f, true);
    return () => window.removeEventListener('keydown', f, true);
  }, [ctrl]);
}
