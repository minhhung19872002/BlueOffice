/**
 * Thesaurus (CORE-THES-001) shared by BlueDoc, BlueSheet and BlueSlide: `useThesaurus` gives
 * the content of the "Từ đồng nghĩa" task pane — a search box, the senses of the word grouped by
 * part of speech (short definition, synonyms, antonyms) and Replace / Insert / Look up for the
 * chosen word; `useThesaurusMenu` + `thesaurusMenuEntry` give the context-menu submenu with the
 * top synonyms of the word under the caret. Replacing keeps the capitalisation of the replaced
 * word and is one undo step (the app's `replace`). Data comes from the platform's
 * `ThesaurusProvider` (loaded on first use); without one the pane says so.
 */
import { useTranslation } from '@blueoffice/i18n';
import {
  matchCase,
  sensesByPos,
  topSynonyms,
  type ThesaurusProvider,
  type ThesaurusResult,
} from '@blueoffice/platform';
import type { MenuEntry, MenuLine } from '@blueoffice/ui';
import { Search } from 'lucide-react';
import { useCallback, useEffect, useRef, useState, type ReactNode } from 'react';
import styles from './ThesaurusPane.module.css';

/** The word the pane works on and how the app changes it. */
export interface ThesaurusTarget {
  /** Word under the caret or selected ('' when none). */
  word: string;
  /**
   * Replaces `word` (under the caret / selected) by `text` as one undo step; `false` when the
   * word is no longer there.
   */
  replace?(text: string, word: string): Promise<boolean | void> | boolean | void;
  /** Inserts `text` at the caret. */
  insert?(text: string): Promise<void> | void;
}

export interface ThesaurusOpen {
  target: ThesaurusTarget;
}

export interface Thesaurus {
  active: boolean;
  open(o: ThesaurusOpen): void;
  close(): void;
  /** Content of the `thesaurus` task pane (`null` while closed). */
  content: ReactNode;
}

/**
 * Holds the editor's thesaurus hook for the commands, which run at event time; the editor keeps
 * one per mount and refreshes it after each render (like `TranslateLink`).
 */
export class ThesaurusLink {
  private hook: Thesaurus | null = null;

  attach(hook: Thesaurus): void {
    this.hook = hook;
  }

  open(o: ThesaurusOpen): void {
    this.hook?.open(o);
  }
}

/** `undefined`: looking up; `null`: not found. */
type Lookup = { query: string; result: ThesaurusResult | null | undefined; failed?: boolean };

/** Words of the context menu's submenu. */
export const MENU_SYNONYMS = 8;

export function useThesaurus(provider: ThesaurusProvider | undefined): Thesaurus {
  const { t } = useTranslation();
  const [req, setReq] = useState<ThesaurusOpen | null>(null);
  const [query, setQuery] = useState('');
  const [found, setFound] = useState<Lookup | null>(null);
  const [chosen, setChosen] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  /** The word replacements apply to (follows each replacement). */
  const [targetWord, setTargetWord] = useState('');
  const gen = useRef(0);
  const searchRef = useRef<HTMLInputElement>(null);

  const lookup = useCallback(
    (word: string) => {
      const q = word.trim();
      const g = ++gen.current;
      setChosen(null);
      setNotice(null);
      if (!q || !provider) {
        setFound(q ? { query: q, result: null } : null);
        return;
      }
      setFound({ query: q, result: undefined });
      provider
        .lookup(q)
        .then((result) => {
          if (gen.current === g) setFound({ query: q, result });
        })
        .catch(() => {
          if (gen.current === g) setFound({ query: q, result: null, failed: true });
        });
    },
    [provider],
  );

  const open = useCallback(
    (o: ThesaurusOpen) => {
      setReq(o);
      setQuery(o.target.word);
      setTargetWord(o.target.word);
      lookup(o.target.word);
      // Typing a word is the next step when there is none.
      if (!o.target.word) setTimeout(() => searchRef.current?.focus(), 0);
    },
    [lookup],
  );

  const close = useCallback(() => {
    gen.current++;
    setReq(null);
    setFound(null);
    setChosen(null);
    setNotice(null);
  }, []);

  useEffect(() => () => void gen.current++, []);

  const target = req?.target;
  const replaceWith = async (w: string) => {
    if (!target?.replace) return;
    const text = matchCase(targetWord, w);
    const ok = await target.replace(text, targetWord);
    if (ok === false) setNotice(t('thesaurus.wordGone', { word: targetWord }));
    else {
      setTargetWord(text);
      setNotice(t('thesaurus.replaced', { from: targetWord, to: text }));
    }
  };

  const result = found?.result;
  const groups = sensesByPos(result ?? null);
  const base = result?.lemmas.filter((l) => l !== found?.query.toLowerCase()) ?? [];
  const chip = (w: string, kind: 'synonym' | 'antonym') => (
    <button
      key={`${kind}:${w}`}
      type="button"
      className={`${styles.chip} ${chosen === w ? styles.chosen : ''}`}
      data-testid={`thesaurus-${kind}`}
      data-word={w}
      aria-pressed={chosen === w}
      onClick={() => setChosen(w)}
      onDoubleClick={() => {
        setQuery(w);
        lookup(w);
      }}
    >
      {w}
    </button>
  );

  const content = req ? (
    <div className={styles.pane} data-testid="thesaurus-pane">
      <form
        className={styles.search}
        role="search"
        onSubmit={(e) => {
          e.preventDefault();
          lookup(query);
        }}
      >
        <input
          ref={searchRef}
          type="search"
          className={styles.input}
          data-testid="thesaurus-search"
          aria-label={t('thesaurus.search')}
          placeholder={t('thesaurus.searchPlaceholder')}
          value={query}
          onChange={(e) => setQuery(e.target.value)}
        />
        <button
          type="submit"
          className={styles.icon}
          title={t('thesaurus.lookUp')}
          aria-label={t('thesaurus.lookUp')}
          data-testid="thesaurus-go"
        >
          <Search size={14} aria-hidden />
        </button>
      </form>
      {!provider ? (
        <p className={styles.notice} role="status" data-testid="thesaurus-unavailable">
          {t('thesaurus.unavailable')}
        </p>
      ) : !found ? (
        <p className={styles.hint}>{t('thesaurus.noWord')}</p>
      ) : result === undefined ? (
        <p className={styles.hint} role="status" data-testid="thesaurus-loading">
          {provider.loaded() ? t('thesaurus.searching') : t('thesaurus.loading')}
        </p>
      ) : !result || !groups.length ? (
        <p className={styles.notice} role="status" data-testid="thesaurus-none">
          {found.failed
            ? t('thesaurus.loadFailed')
            : t('thesaurus.notFound', { word: found.query })}
        </p>
      ) : (
        <div className={styles.results} data-testid="thesaurus-results">
          {base.length > 0 && (
            <p className={styles.hint} data-testid="thesaurus-lemma">
              {t('thesaurus.baseForm', { word: base.join(', ') })}
            </p>
          )}
          {groups.map((g) => (
            <section
              key={g.pos}
              className={styles.group}
              data-testid="thesaurus-group"
              data-pos={g.pos}
            >
              <h3 className={styles.pos}>{t(`thesaurus.pos.${g.pos}`)}</h3>
              {g.senses.map((s, i) => (
                <div key={i} className={styles.sense} data-testid="thesaurus-sense">
                  <div className={styles.gloss}>{s.gloss}</div>
                  {s.synonyms.length > 0 && (
                    <div className={styles.chips}>{s.synonyms.map((w) => chip(w, 'synonym'))}</div>
                  )}
                  {s.antonyms.length > 0 && (
                    <div className={styles.chips}>
                      <span className={styles.caption}>{t('thesaurus.antonyms')}</span>
                      {s.antonyms.map((w) => chip(w, 'antonym'))}
                    </div>
                  )}
                </div>
              ))}
            </section>
          ))}
        </div>
      )}
      <div className={styles.actions}>
        {target?.replace && (
          <button
            type="button"
            className={styles.primary}
            data-testid="thesaurus-replace"
            disabled={!chosen || !targetWord}
            onClick={() => chosen && void replaceWith(chosen)}
          >
            {t('thesaurus.replace')}
          </button>
        )}
        {target?.insert && (
          <button
            type="button"
            className={styles.button}
            data-testid="thesaurus-insert"
            disabled={!chosen}
            onClick={() => chosen && void target.insert?.(chosen)}
          >
            {t('thesaurus.insert')}
          </button>
        )}
        <button
          type="button"
          className={styles.button}
          data-testid="thesaurus-lookup-chosen"
          disabled={!chosen}
          onClick={() => {
            if (!chosen) return;
            setQuery(chosen);
            lookup(chosen);
          }}
        >
          {t('thesaurus.lookUpChosen')}
        </button>
      </div>
      {notice && (
        <p className={styles.notice} role="status" data-testid="thesaurus-notice">
          {notice}
        </p>
      )}
      {result && (
        <p className={styles.credit} data-testid="thesaurus-credit">
          {t('thesaurus.credit')}
        </p>
      )}
    </div>
  ) : null;

  return { active: !!req, open, close, content };
}

/** The context menu's thesaurus lookup: the word under the caret and its result. */
export interface ThesaurusMenuState {
  word: string;
  /** `undefined`: looking up; `null`: nothing found. */
  result: ThesaurusResult | null | undefined;
}

/** Looks the word of a context menu up (a newer menu wins over an older answer). */
export function useThesaurusMenu(provider: ThesaurusProvider | undefined) {
  const [state, setState] = useState<ThesaurusMenuState | null>(null);
  const gen = useRef(0);
  const prepare = useCallback(
    (word: string | null | undefined) => {
      const g = ++gen.current;
      const w = word?.trim() ?? '';
      if (!w || !provider) {
        setState(null);
        return;
      }
      setState({ word: w, result: undefined });
      provider
        .lookup(w)
        .then((result) => {
          if (gen.current === g) setState({ word: w, result });
        })
        .catch(() => {
          if (gen.current === g) setState({ word: w, result: null });
        });
    },
    [provider],
  );
  return { state, prepare };
}

/**
 * The "Từ đồng nghĩa" submenu entry: the top synonyms of the word (in its capitalisation; one
 * picks the replacement), then the pane. `null` when there is no word.
 */
export function thesaurusMenuEntry(
  state: ThesaurusMenuState | null,
  t: (key: string, o?: Record<string, unknown>) => string,
  onPick: (text: string, word: string) => void,
  openPane: () => void,
): MenuEntry | null {
  if (!state) return null;
  const lines: MenuLine[] = [];
  if (state.result === undefined) lines.push({ heading: t('thesaurus.searching') });
  else {
    const words = topSynonyms(state.result, MENU_SYNONYMS);
    if (!words.length)
      lines.push({ heading: t('thesaurus.noSynonyms'), testId: 'thesaurus-menu-none' });
    for (const w of words) {
      const text = matchCase(state.word, w);
      lines.push({
        id: `thesaurus.pick.${w}`,
        label: text,
        emphasis: true,
        onSelect: () => onPick(text, state.word),
      });
    }
  }
  lines.push({ separator: true });
  lines.push({ id: 'review.thesaurus', label: t('thesaurus.openPane'), onSelect: openPane });
  return {
    id: 'thesaurus.menu',
    label: t('thesaurus.menu'),
    submenu: lines,
    submenuTestId: 'thesaurus-menu',
    onSelect: openPane,
  };
}
