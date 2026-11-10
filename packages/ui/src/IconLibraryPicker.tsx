import {
  memo,
  useEffect,
  useMemo,
  useRef,
  useState,
  type KeyboardEvent as ReactKeyboardEvent,
} from 'react';
import {
  loadIconLibrary,
  searchIcons,
  type IconLibrary,
  type IconLibraryEntry,
  type IconLocale,
} from './iconLibrary/catalog';
import { moveIndex } from './SymbolPicker';
import styles from './IconLibraryPicker.module.css';

/** UI strings of the icon library (the caller translates them). */
export interface IconLibraryLabels {
  title: string;
  search: string;
  category: string;
  allCategories: string;
  grid: string;
  loading: string;
  loadError: string;
  noResults: string;
  /** "3 icons found". */
  found: (count: number) => string;
  /** "2 selected". */
  selected: (count: number) => string;
  clear: string;
  insert: string;
  close: string;
}

/** An icon handed to the document. */
export interface IconPick {
  id: string;
  /** Name in the UI language: the default alternative text of the inserted picture. */
  name: string;
  /** The SVG document. */
  svg: string;
}

export interface IconLibraryPickerProps {
  labels: IconLibraryLabels;
  /** Language of the names shown and handed to `onInsert`. */
  locale: IconLocale;
  /** Inserts the chosen icons, in the order they were selected. */
  onInsert: (icons: IconPick[]) => void | Promise<void>;
  onClose?: () => void;
  /** Icons per grid row (default 8). */
  columns?: number;
  /** Source of the library (tests); the default fetches the lazy chunk. */
  load?: () => Promise<IconLibrary>;
}

const dataUrl = (svg: string) => `data:image/svg+xml;utf8,${encodeURIComponent(svg)}`;

/** The grid cell an event happened in (clicks are handled once, on the grid). */
function cellOf(target: EventTarget): { id: string; index: number } | null {
  const cell = (target as HTMLElement).closest<HTMLElement>('[data-icon]');
  const id = cell?.dataset.icon;
  return cell && id ? { id, index: Number(cell.dataset.index) } : null;
}

/** One icon of the grid; memoised so that selecting or moving redraws two cells, not all. */
const Cell = memo(function Cell({
  id,
  name,
  src,
  index,
  col,
  selected,
  active,
}: {
  id: string;
  name: string;
  src: string | undefined;
  index: number;
  col: number;
  selected: boolean;
  active: boolean;
}) {
  return (
    <div
      role="gridcell"
      aria-colindex={col}
      aria-selected={selected}
      aria-label={name}
      title={name}
      tabIndex={active ? 0 : -1}
      data-index={index}
      data-icon={id}
      className={styles.cell}
    >
      {src && <img src={src} alt="" width={32} height={32} draggable={false} />}
    </div>
  );
});

/**
 * Searchable library of insertable icons (CORE-ICON-001): category filter, search by
 * Vietnamese / English name and keyword (diacritics ignored), a grid with roving focus.
 * Click or Space selects (several icons can be selected), double-click inserts one icon,
 * Enter or the Insert button inserts the selection. Keyboard in the grid: arrows, Home/End
 * (Ctrl: first/last), PageUp/PageDown.
 */
export function IconLibraryPicker({
  labels,
  locale,
  onInsert,
  onClose,
  columns = 8,
  load = loadIconLibrary,
}: IconLibraryPickerProps) {
  const [library, setLibrary] = useState<IconLibrary | null>(null);
  const [failed, setFailed] = useState(false);
  const [query, setQuery] = useState('');
  const [category, setCategory] = useState('');
  const [active, setActive] = useState(0);
  const [picked, setPicked] = useState<string[]>([]);
  const [busy, setBusy] = useState(false);
  const gridRef = useRef<HTMLDivElement>(null);
  const focusCell = useRef(false);

  useEffect(() => {
    let alive = true;
    load().then(
      (lib) => alive && setLibrary(lib),
      () => alive && setFailed(true),
    );
    return () => {
      alive = false;
    };
  }, [load]);

  const items = useMemo<IconLibraryEntry[]>(
    () => (library ? searchIcons(library.manifest.icons, query, category || null) : []),
    [library, query, category],
  );
  const sources = useMemo(() => {
    const m = new Map<string, string>();
    for (const i of library?.manifest.icons ?? []) {
      const svg = library?.svg(i.id);
      if (svg) m.set(i.id, dataUrl(svg));
    }
    return m;
  }, [library]);
  const activeIndex = Math.max(0, Math.min(active, items.length - 1));
  const current = items[activeIndex];

  // Keyboard navigation moves the DOM focus with the active cell (roving tabindex).
  useEffect(() => {
    if (!focusCell.current) return;
    focusCell.current = false;
    gridRef.current?.querySelector<HTMLElement>(`[data-index="${active}"]`)?.focus();
  }, [active]);

  const toggle = (id: string) =>
    setPicked((p) => (p.includes(id) ? p.filter((x) => x !== id) : [...p, id]));

  const insert = async (ids: readonly string[]) => {
    if (!library || busy) return;
    const picks: IconPick[] = [];
    for (const id of ids) {
      const entry = library.manifest.icons.find((i) => i.id === id);
      const svg = library.svg(id);
      if (entry && svg) picks.push({ id, name: entry.name[locale], svg });
    }
    if (picks.length === 0) return;
    setBusy(true);
    try {
      await onInsert(picks);
      setPicked([]);
    } finally {
      setBusy(false);
    }
  };
  const insertSelection = () => insert(picked.length > 0 ? picked : current ? [current.id] : []);

  const rows: IconLibraryEntry[][] = [];
  for (let i = 0; i < items.length; i += columns) rows.push(items.slice(i, i + columns));

  const onGridKey = (e: ReactKeyboardEvent) => {
    if (e.key === ' ') {
      e.preventDefault();
      if (current) toggle(current.id);
      return;
    }
    if (e.key === 'Enter') {
      e.preventDefault();
      void insertSelection();
      return;
    }
    const next = moveIndex(e.key, activeIndex, items.length, columns, e.ctrlKey || e.metaKey);
    if (next === null) return;
    e.preventDefault();
    focusCell.current = true;
    setActive(next);
  };

  const reset = () => setActive(0);

  return (
    <div className={styles.picker} data-testid="icon-library">
      <div className={styles.header}>
        <strong>{labels.title}</strong>
        {onClose && (
          <button
            type="button"
            className={styles.close}
            aria-label={labels.close}
            title={labels.close}
            onClick={onClose}
          >
            ×
          </button>
        )}
      </div>

      <div className={styles.controls}>
        <input
          type="search"
          className={styles.search}
          aria-label={labels.search}
          placeholder={labels.search}
          data-testid="icon-search"
          value={query}
          onChange={(e) => {
            setQuery(e.target.value);
            reset();
          }}
          onKeyDown={(e) => {
            if (e.key === 'ArrowDown' && items.length > 0) {
              e.preventDefault();
              setActive(0);
              gridRef.current?.querySelector<HTMLElement>('[data-index="0"]')?.focus();
            }
          }}
        />
        <label className={styles.field}>
          <span>{labels.category}</span>
          <select
            value={category}
            data-testid="icon-category"
            onChange={(e) => {
              setCategory(e.target.value);
              reset();
            }}
          >
            <option value="">{labels.allCategories}</option>
            {library?.manifest.categories.map((c) => (
              <option key={c.id} value={c.id}>
                {c.name[locale]}
              </option>
            ))}
          </select>
        </label>
      </div>

      <div
        ref={gridRef}
        role="grid"
        aria-label={labels.grid}
        aria-multiselectable="true"
        aria-busy={!library && !failed}
        aria-rowcount={rows.length}
        aria-colcount={columns}
        className={styles.grid}
        data-testid="icon-grid"
        onKeyDown={onGridKey}
        onClick={(e) => {
          const hit = cellOf(e.target);
          if (!hit) return;
          setActive(hit.index);
          toggle(hit.id);
        }}
        onDoubleClick={(e) => {
          const hit = cellOf(e.target);
          if (hit) void insert([hit.id]);
        }}
      >
        {!library && !failed && <div className={styles.empty}>{labels.loading}</div>}
        {failed && (
          <div className={styles.empty} role="alert">
            {labels.loadError}
          </div>
        )}
        {library && items.length === 0 && <div className={styles.empty}>{labels.noResults}</div>}
        {rows.map((row, r) => (
          <div role="row" key={r} className={styles.row} aria-rowindex={r + 1}>
            {row.map((e, c) => {
              const i = r * columns + c;
              return (
                <Cell
                  key={e.id}
                  id={e.id}
                  name={e.name[locale]}
                  src={sources.get(e.id)}
                  index={i}
                  col={c + 1}
                  selected={picked.includes(e.id)}
                  active={i === activeIndex}
                />
              );
            })}
          </div>
        ))}
      </div>

      <div className={styles.footer}>
        <span className={styles.status} aria-live="polite" data-testid="icon-status">
          {library &&
            (picked.length > 0 ? labels.selected(picked.length) : labels.found(items.length))}
        </span>
        <button
          type="button"
          className={styles.button}
          disabled={picked.length === 0}
          onClick={() => setPicked([])}
        >
          {labels.clear}
        </button>
        <button
          type="button"
          className={styles.primary}
          data-testid="icon-insert"
          // Not disabled while inserting: a disabled button would drop the keyboard focus
          // (and with it Escape); `insert` ignores a second request meanwhile.
          disabled={picked.length === 0 && !current}
          aria-busy={busy}
          onClick={() => void insertSelection()}
        >
          {labels.insert}
        </button>
      </div>
    </div>
  );
}
