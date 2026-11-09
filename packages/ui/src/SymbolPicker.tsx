import {
  useEffect,
  useMemo,
  useRef,
  useState,
  type KeyboardEvent as ReactKeyboardEvent,
} from 'react';
import {
  SPECIAL_CHARACTERS,
  formatCodePoint,
  isInvisible,
  loadRecentSymbols,
  pushRecentSymbol,
  searchSymbols,
  symbolBlocks,
  symbolEntry,
  type SpecialCharacterId,
  type SymbolEntry,
} from './symbols/catalog';
import styles from './SymbolPicker.module.css';

/** UI strings of the symbol picker (the caller translates them). */
export interface SymbolPickerLabels {
  title: string;
  symbolsTab: string;
  specialTab: string;
  search: string;
  block: string;
  font: string;
  recent: string;
  noRecent: string;
  noResults: string;
  grid: string;
  insert: string;
  close: string;
  /** Translated group name by id; `fallback` is the English UCD block name. */
  blockName: (id: string, fallback: string) => string;
  /** Names of the "special characters" entries. */
  special: Record<SpecialCharacterId, string>;
}

export interface SymbolPickerProps {
  labels: SymbolPickerLabels;
  /** Inserts the character (double-click, Enter or the insert button). */
  onInsert: (char: string, font?: string) => void;
  onClose?: () => void;
  /** Font names of the font selector; the glyphs are drawn in the chosen one. */
  fonts?: readonly string[];
  /** Initially chosen font (the document's font at the caret). */
  font?: string;
  /** Characters per grid row (default 16). */
  columns?: number;
}

/** Grid rows moved by PageUp / PageDown. */
const PAGE_ROWS = 6;

/** Grid cell index after a navigation key, or `null` when the key does not move. */
export function moveIndex(
  key: string,
  index: number,
  count: number,
  columns: number,
  ctrl = false,
): number | null {
  if (count === 0) return null;
  const last = count - 1;
  const clamp = (i: number) => Math.max(0, Math.min(last, i));
  switch (key) {
    case 'ArrowRight':
      return clamp(index + 1);
    case 'ArrowLeft':
      return clamp(index - 1);
    case 'ArrowDown':
      return index + columns <= last ? index + columns : index;
    case 'ArrowUp':
      return index - columns >= 0 ? index - columns : index;
    case 'Home':
      return ctrl ? 0 : index - (index % columns);
    case 'End':
      return ctrl ? last : clamp(index - (index % columns) + columns - 1);
    case 'PageDown':
      return clamp(index + columns * PAGE_ROWS);
    case 'PageUp':
      return clamp(index - columns * PAGE_ROWS);
    default:
      return null;
  }
}

/** A character as shown in a cell: spaces and format controls get a placeholder. */
function Glyph({ char }: { char: string }) {
  if (isInvisible(char)) {
    const cp = char.codePointAt(0) ?? 0;
    return (
      <span className={styles.invisible} aria-hidden="true">
        {cp.toString(16).toUpperCase()}
      </span>
    );
  }
  return <span aria-hidden="true">{char}</span>;
}

/**
 * Searchable Unicode symbol picker: characters grouped by Unicode block (and the
 * Vietnamese letters), search by English name / code point / Vietnamese keyword,
 * recently used characters, a font selector, and a tab of special characters
 * (dashes, non-breaking and zero-width spaces…). Keyboard: arrows, Home/End
 * (Ctrl: first/last), PageUp/PageDown in the grid; Enter or double-click inserts.
 */
export function SymbolPicker({
  labels,
  onInsert,
  onClose,
  fonts = [],
  font: initialFont,
  columns = 16,
}: SymbolPickerProps) {
  const blocks = symbolBlocks();
  const [tab, setTab] = useState<'symbols' | 'special'>('symbols');
  const [query, setQuery] = useState('');
  const [blockId, setBlockId] = useState(blocks[0]?.id ?? '');
  const [font, setFont] = useState(initialFont ?? fonts[0] ?? '');
  const [recent, setRecent] = useState<string[]>(() => loadRecentSymbols());
  const [active, setActive] = useState(0);
  const [hover, setHover] = useState<SymbolEntry | null>(null);
  const [special, setSpecial] = useState(0);
  const gridRef = useRef<HTMLDivElement>(null);
  const focusCell = useRef(false);

  const items = useMemo<SymbolEntry[]>(
    () =>
      query.trim() ? searchSymbols(query) : (blocks.find((b) => b.id === blockId)?.entries ?? []),
    [query, blockId, blocks],
  );
  const activeIndex = Math.min(active, items.length - 1);
  const current = items[activeIndex];
  const shown = hover ?? current ?? null;
  const fontStyle = font ? { fontFamily: `"${font}", var(--bo-font, sans-serif)` } : undefined;

  // Keyboard navigation moves the DOM focus with the active cell (roving tabindex).
  useEffect(() => {
    if (!focusCell.current) return;
    focusCell.current = false;
    gridRef.current?.querySelector<HTMLElement>(`[data-index="${active}"]`)?.focus();
  }, [active]);

  const insert = (char: string) => {
    setRecent((r) => pushRecentSymbol(char, r));
    onInsert(char, font || undefined);
  };

  const rows: SymbolEntry[][] = [];
  for (let i = 0; i < items.length; i += columns) rows.push(items.slice(i, i + columns));

  const onGridKey = (e: ReactKeyboardEvent) => {
    if (e.key === 'Enter' || e.key === ' ') {
      e.preventDefault();
      if (current) insert(current.char);
      return;
    }
    const next = moveIndex(
      e.key,
      Math.max(0, activeIndex),
      items.length,
      columns,
      e.ctrlKey || e.metaKey,
    );
    if (next === null) return;
    e.preventDefault();
    focusCell.current = true;
    setActive(next);
    setHover(null);
  };

  const onSpecialKey = (e: ReactKeyboardEvent) => {
    const n = SPECIAL_CHARACTERS.length;
    let next: number | null = null;
    if (e.key === 'ArrowDown') next = Math.min(n - 1, special + 1);
    else if (e.key === 'ArrowUp') next = Math.max(0, special - 1);
    else if (e.key === 'Home') next = 0;
    else if (e.key === 'End') next = n - 1;
    else if (e.key === 'Enter') {
      e.preventDefault();
      const s = SPECIAL_CHARACTERS[special];
      if (s) insert(s.char);
      return;
    }
    if (next === null) return;
    e.preventDefault();
    setSpecial(next);
  };

  const tabButton = (id: 'symbols' | 'special', label: string) => (
    <button
      type="button"
      role="tab"
      id={`bo-symbol-tab-${id}`}
      aria-selected={tab === id}
      aria-controls={`bo-symbol-panel-${id}`}
      tabIndex={tab === id ? 0 : -1}
      className={styles.tab}
      onClick={() => setTab(id)}
      onKeyDown={(e) => {
        if (e.key === 'ArrowRight' || e.key === 'ArrowLeft') {
          e.preventDefault();
          setTab(tab === 'symbols' ? 'special' : 'symbols');
        }
      }}
    >
      {label}
    </button>
  );

  const selectedSpecial = SPECIAL_CHARACTERS[special];

  return (
    <div className={styles.picker} data-testid="symbol-picker">
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
      <div role="tablist" className={styles.tabs}>
        {tabButton('symbols', labels.symbolsTab)}
        {tabButton('special', labels.specialTab)}
      </div>

      {tab === 'symbols' ? (
        <div
          role="tabpanel"
          id="bo-symbol-panel-symbols"
          aria-labelledby="bo-symbol-tab-symbols"
          className={styles.body}
        >
          <div className={styles.controls}>
            <input
              type="search"
              className={styles.search}
              aria-label={labels.search}
              placeholder={labels.search}
              data-testid="symbol-search"
              value={query}
              onChange={(e) => {
                setQuery(e.target.value);
                setActive(0);
                setHover(null);
              }}
              onKeyDown={(e) => {
                if (e.key === 'ArrowDown' && items.length > 0) {
                  e.preventDefault();
                  focusCell.current = true;
                  setActive(0);
                  // Same index: the effect does not run, focus now.
                  gridRef.current?.querySelector<HTMLElement>('[data-index="0"]')?.focus();
                } else if (e.key === 'Enter' && current) {
                  e.preventDefault();
                  insert(current.char);
                }
              }}
            />
            <label className={styles.field}>
              <span>{labels.block}</span>
              <select
                value={blockId}
                data-testid="symbol-block"
                disabled={query.trim() !== ''}
                onChange={(e) => {
                  setBlockId(e.target.value);
                  setActive(0);
                  setHover(null);
                }}
              >
                {blocks.map((b) => (
                  <option key={b.id} value={b.id}>
                    {labels.blockName(b.id, b.name)}
                  </option>
                ))}
              </select>
            </label>
            {fonts.length > 0 && (
              <label className={styles.field}>
                <span>{labels.font}</span>
                <select
                  value={font}
                  data-testid="symbol-font"
                  onChange={(e) => setFont(e.target.value)}
                >
                  {!fonts.includes(font) && font && <option value={font}>{font}</option>}
                  {fonts.map((f) => (
                    <option key={f} value={f}>
                      {f}
                    </option>
                  ))}
                </select>
              </label>
            )}
          </div>

          <div className={styles.recentRow}>
            <span className={styles.caption}>{labels.recent}</span>
            <div className={styles.recent} role="toolbar" aria-label={labels.recent}>
              {recent.length === 0 && <span className={styles.caption}>{labels.noRecent}</span>}
              {recent.map((c) => {
                const e = symbolEntry(c.codePointAt(0) ?? -1);
                const name = e
                  ? `${formatCodePoint(e.cp)} ${e.name}`
                  : formatCodePoint(c.codePointAt(0) ?? 0);
                return (
                  <button
                    key={c}
                    type="button"
                    className={styles.recentCell}
                    style={fontStyle}
                    aria-label={name}
                    title={name}
                    data-testid="symbol-recent"
                    onClick={() => insert(c)}
                  >
                    <Glyph char={c} />
                  </button>
                );
              })}
            </div>
          </div>

          <div
            ref={gridRef}
            role="grid"
            aria-label={labels.grid}
            aria-rowcount={rows.length}
            aria-colcount={columns}
            className={styles.grid}
            data-testid="symbol-grid"
            onKeyDown={onGridKey}
            onMouseLeave={() => setHover(null)}
          >
            {items.length === 0 && <div className={styles.empty}>{labels.noResults}</div>}
            {rows.map((row, r) => (
              <div role="row" key={r} className={styles.row} aria-rowindex={r + 1}>
                {row.map((e, c) => {
                  const i = r * columns + c;
                  const selected = i === activeIndex;
                  return (
                    <div
                      key={e.cp}
                      role="gridcell"
                      aria-colindex={c + 1}
                      aria-selected={selected}
                      aria-label={`${formatCodePoint(e.cp)} ${e.name}`}
                      tabIndex={selected ? 0 : -1}
                      data-index={i}
                      data-cp={formatCodePoint(e.cp)}
                      className={styles.cell}
                      style={fontStyle}
                      onClick={() => setActive(i)}
                      onDoubleClick={() => insert(e.char)}
                      onMouseEnter={() => setHover(e)}
                    >
                      <Glyph char={e.char} />
                    </div>
                  );
                })}
              </div>
            ))}
          </div>

          <div className={styles.info} aria-live="polite" data-testid="symbol-info">
            {shown && (
              <>
                <span className={styles.preview} style={fontStyle}>
                  <Glyph char={shown.char} />
                </span>
                <span className={styles.code}>{formatCodePoint(shown.cp)}</span>
                <span className={styles.name}>{shown.name}</span>
              </>
            )}
          </div>
        </div>
      ) : (
        <div
          role="tabpanel"
          id="bo-symbol-panel-special"
          aria-labelledby="bo-symbol-tab-special"
          className={styles.body}
        >
          <div
            role="listbox"
            tabIndex={0}
            aria-label={labels.specialTab}
            aria-activedescendant={`bo-special-${special}`}
            className={styles.specialList}
            data-testid="symbol-special"
            onKeyDown={onSpecialKey}
          >
            {SPECIAL_CHARACTERS.map((s, i) => (
              <div
                key={s.id}
                id={`bo-special-${i}`}
                role="option"
                aria-selected={i === special}
                className={styles.specialItem}
                onClick={() => setSpecial(i)}
                onDoubleClick={() => insert(s.char)}
              >
                <span className={styles.specialGlyph} style={fontStyle}>
                  <Glyph char={s.char} />
                </span>
                <span className={styles.specialName}>{labels.special[s.id]}</span>
                <span className={styles.code}>{formatCodePoint(s.char.codePointAt(0) ?? 0)}</span>
              </div>
            ))}
          </div>
        </div>
      )}

      <div className={styles.footer}>
        <button
          type="button"
          className={styles.primary}
          data-testid="symbol-insert"
          disabled={tab === 'symbols' ? !current : !selectedSpecial}
          onClick={() => {
            const c = tab === 'symbols' ? current?.char : selectedSpecial?.char;
            if (c) insert(c);
          }}
        >
          {labels.insert}
        </button>
      </div>
    </div>
  );
}
