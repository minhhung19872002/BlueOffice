import type { EditCommand, EditResult, FindMatch, FindOptions } from '@blueoffice/platform';
import { useEffect, useRef, useState } from 'react';
import { useTranslation } from '@blueoffice/i18n';
import styles from './Editor.module.css';

/** Special characters offered by the "Special" menu (`replaceOnly`: only in "Replace with"). */
export const FIND_SPECIALS: { code: string; key: string; replaceOnly?: boolean }[] = [
  { code: '^p', key: 'paragraph' },
  { code: '^t', key: 'tab' },
  { code: '^l', key: 'lineBreak' },
  { code: '^m', key: 'pageBreak' },
  { code: '^#', key: 'digit' },
  { code: '^$', key: 'letter' },
  { code: '^?', key: 'anyChar' },
  { code: '^^', key: 'caret' },
  { code: '^&', key: 'found', replaceOnly: true },
];

/** Find / replace dialog (case, whole word, regex, wildcards, special characters, highlight all). */
export function FindPanel({
  run,
  replace,
  onClose,
  onClearHighlights,
}: {
  run: (cmd: EditCommand) => Promise<EditResult | null>;
  replace: boolean;
  onClose: () => void;
  /** Remove the "highlight all" boxes from the document view. */
  onClearHighlights?: () => void;
}) {
  const { t } = useTranslation();
  const [query, setQuery] = useState('');
  const [with_, setWith] = useState('');
  const [opts, setOpts] = useState<FindOptions>({});
  const [matches, setMatches] = useState<FindMatch[]>([]);
  const [index, setIndex] = useState(-1);
  const [message, setMessage] = useState('');
  const [highlight, setHighlight] = useState(false);
  const [field, setField] = useState<'query' | 'with'>('query');

  // Highlights go away with the panel (the latest callback, whatever the parent passed).
  const clearRef = useRef(onClearHighlights);
  useEffect(() => {
    clearRef.current = onClearHighlights;
  }, [onClearHighlights]);
  useEffect(() => () => clearRef.current?.(), []);

  const search = async (hl = highlight): Promise<FindMatch[]> => {
    const r = await run({ cmd: 'find', query, options: opts, ...(hl ? { highlight: true } : {}) });
    if (r?.error) setMessage(t('find.invalid'));
    const m = r?.matches ?? [];
    setMatches(m);
    if (!r?.error) setMessage(m.length ? t('find.count', { count: m.length }) : t('find.none'));
    return m;
  };

  const toggleHighlight = async () => {
    if (highlight) {
      setHighlight(false);
      onClearHighlights?.();
      return;
    }
    setHighlight(true);
    const m = await search(true);
    if (m.length) setMessage(t('find.highlighted', { count: m.length }));
  };

  const insertSpecial = (code: string) => {
    if (field === 'with') setWith(with_ + code);
    else {
      setQuery(query + code);
      setIndex(-1);
    }
  };

  const next = async () => {
    const m = await search();
    if (!m.length) return;
    const i = (index + 1) % m.length;
    setIndex(i);
    await run({ cmd: 'selectMatch', match: m[i]! });
  };

  const replaceOne = async () => {
    const m = matches[index];
    if (!m) return next();
    await run({ cmd: 'replace', match: m, with: with_, query, options: opts });
    setIndex(index - 1);
    await next();
  };

  const replaceAll = async () => {
    const r = await run({ cmd: 'replaceAll', query, options: opts, with: with_ });
    if (highlight) await search(true);
    setMessage(t('find.replaced', { count: r?.count ?? 0 }));
    setMatches([]);
    setIndex(-1);
  };

  // Regular expressions and wildcards are two ways of reading the query: one at a time.
  const toggle = (k: keyof FindOptions) => {
    const on = !opts[k];
    setOpts({
      ...opts,
      [k]: on,
      ...(on && k === 'regex' ? { wildcards: false } : {}),
      ...(on && k === 'wildcards' ? { regex: false } : {}),
    });
    setIndex(-1);
  };

  return (
    <div
      className={styles.panel}
      role="dialog"
      aria-label={t(replace ? 'find.replaceTitle' : 'find.title')}
    >
      <div className={styles.panelRow}>
        <input
          autoFocus
          data-testid="find-query"
          aria-label={t('find.query')}
          placeholder={t('find.query')}
          value={query}
          onFocus={() => setField('query')}
          onChange={(e) => {
            setQuery(e.target.value);
            setIndex(-1);
          }}
          onKeyDown={(e) => {
            if (e.key === 'Enter') void next();
            if (e.key === 'Escape') onClose();
          }}
        />
        <button type="button" onClick={() => void next()}>
          {t('find.next')}
        </button>
        <button
          type="button"
          data-testid="find-highlight-all"
          aria-pressed={highlight}
          onClick={() => void toggleHighlight()}
        >
          {t('find.highlightAll')}
        </button>
        <button type="button" aria-label={t('host.close')} onClick={onClose}>
          ×
        </button>
      </div>
      {replace && (
        <div className={styles.panelRow}>
          <input
            data-testid="find-replace"
            aria-label={t('find.with')}
            placeholder={t('find.with')}
            value={with_}
            onFocus={() => setField('with')}
            onChange={(e) => setWith(e.target.value)}
          />
          <button type="button" onClick={() => void replaceOne()}>
            {t('find.replace')}
          </button>
          <button type="button" onClick={() => void replaceAll()}>
            {t('find.replaceAll')}
          </button>
        </div>
      )}
      <div className={styles.panelRow}>
        {(['matchCase', 'wholeWord', 'regex', 'wildcards'] as const).map((k) => (
          <label key={k}>
            <input
              type="checkbox"
              data-testid={`find-${k}`}
              checked={!!opts[k]}
              // Wildcard search is always case-sensitive and never whole-word.
              disabled={!!opts.wildcards && (k === 'matchCase' || k === 'wholeWord')}
              onChange={() => toggle(k)}
            />{' '}
            {t(`find.${k}`)}
          </label>
        ))}
        <select
          data-testid="find-special"
          aria-label={t('find.special')}
          value=""
          onChange={(e) => {
            if (e.target.value) insertSpecial(e.target.value);
          }}
        >
          <option value="">{t('find.special')}</option>
          {FIND_SPECIALS.filter((s) => !s.replaceOnly || (replace && field === 'with')).map((s) => (
            <option key={s.code} value={s.code}>
              {t(`find.sp.${s.key}`)}
            </option>
          ))}
        </select>
      </div>
      <div className={styles.panelMessage} data-testid="find-message">
        {message}
      </div>
    </div>
  );
}

const SYMBOLS = [
  '₫',
  '€',
  '$',
  '©',
  '®',
  '™',
  '§',
  '¶',
  '°',
  '±',
  '×',
  '÷',
  '≤',
  '≥',
  '≠',
  '≈',
  '∞',
  '√',
  '½',
  '¼',
  '¾',
  '•',
  '…',
  '–',
  '—',
  '“',
  '”',
  '‘',
  '’',
  '«',
  '»',
  '→',
  '←',
  '↑',
  '↓',
  '✓',
  '✗',
  '★',
  '☐',
  '☑',
  'α',
  'β',
  'γ',
  'Δ',
  'Ω',
  'µ',
  'π',
  'Σ',
];

/** Special character picker. */
export function SymbolPanel({
  onPick,
  onClose,
}: {
  onPick: (s: string) => void;
  onClose: () => void;
}) {
  const { t } = useTranslation();
  return (
    <div className={styles.panel} role="dialog" aria-label={t('commands.insert.symbol')}>
      <div className={styles.panelRow}>
        <strong>{t('commands.insert.symbol')}</strong>
        <button type="button" aria-label={t('host.close')} onClick={onClose}>
          ×
        </button>
      </div>
      <div className={styles.symbols}>
        {SYMBOLS.map((s) => (
          <button key={s} type="button" onClick={() => onPick(s)} aria-label={s}>
            {s}
          </button>
        ))}
      </div>
    </div>
  );
}
