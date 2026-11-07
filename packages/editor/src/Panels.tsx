import type { EditCommand, EditResult, FindMatch, FindOptions } from '@blueoffice/platform';
import { useState } from 'react';
import { useTranslation } from '@blueoffice/i18n';
import styles from './Editor.module.css';

/** Find / replace dialog (case, whole word, regex). */
export function FindPanel({
  run,
  replace,
  onClose,
}: {
  run: (cmd: EditCommand) => Promise<EditResult | null>;
  replace: boolean;
  onClose: () => void;
}) {
  const { t } = useTranslation();
  const [query, setQuery] = useState('');
  const [with_, setWith] = useState('');
  const [opts, setOpts] = useState<FindOptions>({});
  const [matches, setMatches] = useState<FindMatch[]>([]);
  const [index, setIndex] = useState(-1);
  const [message, setMessage] = useState('');

  const search = async (): Promise<FindMatch[]> => {
    const r = await run({ cmd: 'find', query, options: opts });
    if (r?.error) setMessage(t('find.invalid'));
    const m = r?.matches ?? [];
    setMatches(m);
    if (!r?.error) setMessage(m.length ? t('find.count', { count: m.length }) : t('find.none'));
    return m;
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
    await run({ cmd: 'replace', match: m, with: with_ });
    setIndex(index - 1);
    await next();
  };

  const replaceAll = async () => {
    const r = await run({ cmd: 'replaceAll', query, options: opts, with: with_ });
    setMessage(t('find.replaced', { count: r?.count ?? 0 }));
    setMatches([]);
    setIndex(-1);
  };

  const toggle = (k: keyof FindOptions) => setOpts({ ...opts, [k]: !opts[k] });

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
        {(['matchCase', 'wholeWord', 'regex'] as const).map((k) => (
          <label key={k}>
            <input type="checkbox" checked={!!opts[k]} onChange={() => toggle(k)} />{' '}
            {t(`find.${k}`)}
          </label>
        ))}
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
