import { useTranslation } from '@blueoffice/i18n';
import { Dialog } from '@blueoffice/ui';
import { useCallback, useEffect, useRef, useState, type ReactNode } from 'react';
import styles from './Editor.module.css';
import { utf8Length } from './SpellCheckDialog';

/**
 * One issue of a spelling walk outside BlueDoc (BlueSheet cells, BlueSlide text, CORE-SPELL-006).
 * `pos` orders the issues (text unit…, byte start); the last element is the byte start in the text.
 */
export interface WalkIssue {
  pos: readonly number[];
  /** Byte range in the text (`pos` ends with `start`). */
  start: number;
  end: number;
  word: string;
  before: string;
  after: string;
  suggestions: string[];
  grammar?: boolean;
  rule?: string;
  /** Where the issue is, as shown to the user ("Sheet1!B3", "Slide 2"). */
  where: string;
}

const keyOf = (i: WalkIssue) => `${i.pos.join(':')}:${i.end}:${i.rule ?? ''}:${i.word}`;

/** `a` comes before `b` (lexicographic). */
export function posBefore(a: readonly number[], b: readonly number[]): boolean {
  for (let k = 0; k < Math.max(a.length, b.length); k++) {
    const x = a[k] ?? -1;
    const y = b[k] ?? -1;
    if (x !== y) return x < y;
  }
  return false;
}

/** The first issue at or after `cursor` not skipped yet. */
export function nextWalkIssue<I extends WalkIssue>(
  issues: readonly I[],
  cursor: readonly number[],
  skipped: ReadonlySet<string>,
): I | null {
  return issues.find((i) => !posBefore(i.pos, cursor) && !skipped.has(keyOf(i))) ?? null;
}

/** What the walk does with the issues; every change is one undoable edit of the host. */
export interface ProofWalkActions<I extends WalkIssue> {
  /** Check everything again (after each action). */
  load: () => Promise<{ issues: I[]; unchecked: string[] }>;
  /** Replace one issue. */
  change: (issue: I, text: string) => Promise<void>;
  /** Replace every occurrence (one undo step). */
  changeAll: (issues: I[], text: string) => Promise<void>;
  /** Ignore the word (or grammar issue) everywhere. */
  ignoreAll: (issue: I) => Promise<void>;
  /** Add the word to the user dictionary. */
  add: (issue: I) => Promise<void>;
  /** Show the issue in the document (select its cell / shape). */
  show?: (issue: I) => void;
}

interface Counts {
  changed: number;
  ignored: number;
  added: number;
}

/**
 * Spelling and grammar check of a workbook or a deck (F7): walks the issues in order, with the
 * sentence, suggestions and the actions to skip, ignore, add, replace or replace everywhere.
 * `settings` holds what the host adds (the proofing language); `reload` changes when the issues
 * must be read again from the start.
 */
export function ProofWalkDialog<I extends WalkIssue>({
  actions,
  onClose,
  settings,
  reload,
  langName = (l) => l,
}: {
  actions: ProofWalkActions<I>;
  onClose: () => void;
  settings?: ReactNode;
  reload?: unknown;
  langName?: (lang: string) => string;
}) {
  const { t } = useTranslation();
  const [report, setReport] = useState<{ issues: I[]; unchecked: string[] } | null>(null);
  const [cursor, setCursor] = useState<readonly number[]>([]);
  const [skipped, setSkipped] = useState<ReadonlySet<string>>(new Set());
  const [typed, setTyped] = useState<{ key: string; value: string } | null>(null);
  const [counts, setCounts] = useState<Counts>({ changed: 0, ignored: 0, added: 0 });
  const busy = useRef(false);
  const { load, show } = actions;

  const check = useCallback(async () => setReport(await load()), [load]);

  // A new source (another language) starts the walk again.
  const [source, setSource] = useState({ load, reload });
  if (source.load !== load || source.reload !== reload) {
    setSource({ load, reload });
    setCursor([]);
    setSkipped(new Set());
    setReport(null);
  }
  useEffect(() => {
    let live = true;
    void load().then((r) => {
      if (live) setReport(r);
    });
    return () => {
      live = false;
    };
  }, [load, reload]);

  const current = report ? nextWalkIssue(report.issues, cursor, skipped) : null;
  const currentKey = current ? keyOf(current) : '';
  const replacement =
    typed?.key === currentKey ? typed.value : (current?.suggestions[0] ?? current?.word ?? '');
  const shownRef = useRef('');
  useEffect(() => {
    if (current && shownRef.current !== currentKey) {
      shownRef.current = currentKey;
      show?.(current);
    }
  }, [current, currentKey, show]);

  /** Run an action once at a time, then check again from `at`. */
  const act = async (f: () => Promise<void>, at: readonly number[], count: keyof Counts, n = 1) => {
    if (busy.current) return;
    busy.current = true;
    try {
      await f();
      setCounts((c) => ({ ...c, [count]: c[count] + n }));
      setCursor(at);
      await check();
    } finally {
      busy.current = false;
    }
  };

  /** Just after a replacement of `i`: same text, start moved past the new text. */
  const after = (i: I): number[] => [...i.pos.slice(0, -1), i.start + utf8Length(replacement)];

  const footer = (
    <button
      type="button"
      className={styles.dialogPrimary}
      data-testid="spellcheck-close"
      onClick={onClose}
    >
      {t('spell.close')}
    </button>
  );

  return (
    <Dialog
      open
      onOpenChange={(o) => !o && onClose()}
      title={t('spell.checkTitle')}
      testId="spellcheck-dialog"
      width={560}
      footer={footer}
    >
      {settings}
      {!report && <p>{t('spell.checking')}</p>}
      {report && current && (
        <div data-testid="spellcheck-issue">
          <div style={{ display: 'flex', justifyContent: 'space-between', fontSize: 12 }}>
            <strong data-testid="spellcheck-kind">
              {current.grammar ? t('spell.grammarIssue') : t('spell.notInDictionary')}
            </strong>
            <span data-testid="spellcheck-where">{current.where}</span>
          </div>
          <p
            data-testid="spellcheck-context"
            style={{
              margin: '8px 0',
              padding: 8,
              border: '1px solid var(--bo-border, #c8c8c8)',
              borderRadius: 4,
              minHeight: 40,
            }}
          >
            {current.before}
            <mark
              data-testid="spellcheck-word"
              style={{
                background: 'transparent',
                color: current.grammar ? '#1f5fbf' : '#c42b1c',
                fontWeight: 600,
                textDecoration: 'underline wavy',
              }}
            >
              {current.word}
            </mark>
            {current.after}
          </p>
          {current.grammar && current.rule && (
            <p style={{ margin: '4px 0', fontSize: 12 }} data-testid="spellcheck-rule">
              {t(`grammar.${current.rule}`)}
            </p>
          )}
          <label className={styles.field}>
            <span>{t('spell.changeTo')}</span>
            <input
              data-testid="spellcheck-replacement"
              value={replacement}
              onChange={(e) => setTyped({ key: currentKey, value: e.target.value })}
            />
          </label>
          <div
            role="listbox"
            aria-label={t('spell.suggestions')}
            data-testid="spellcheck-suggestions"
          >
            {current.suggestions.length === 0 && (
              <div style={{ opacity: 0.7 }}>{t('spell.noSuggestions')}</div>
            )}
            {current.suggestions.map((s) => (
              <div
                key={s}
                role="option"
                tabIndex={0}
                aria-selected={s === replacement}
                style={{ cursor: 'pointer', fontWeight: s === replacement ? 600 : 400 }}
                onClick={() => setTyped({ key: currentKey, value: s })}
                onKeyDown={(e) => e.key === 'Enter' && setTyped({ key: currentKey, value: s })}
              >
                {s}
              </div>
            ))}
          </div>
          <div style={{ display: 'flex', flexWrap: 'wrap', gap: 6, marginTop: 8 }}>
            <button
              type="button"
              className={styles.dialogButton}
              data-testid="spellcheck-ignore-once"
              onClick={() => {
                setSkipped((s) => new Set([...s, currentKey]));
                setCounts((c) => ({ ...c, ignored: c.ignored + 1 }));
                setCursor(current.pos);
              }}
            >
              {t('spell.ignoreOnce')}
            </button>
            <button
              type="button"
              className={styles.dialogButton}
              data-testid="spellcheck-ignore-all"
              onClick={() => void act(() => actions.ignoreAll(current), current.pos, 'ignored')}
            >
              {t('spell.ignoreAll')}
            </button>
            {!current.grammar && (
              <button
                type="button"
                className={styles.dialogButton}
                data-testid="spellcheck-add"
                onClick={() => void act(() => actions.add(current), current.pos, 'added')}
              >
                {t('spell.addToDictionary')}
              </button>
            )}
            <button
              type="button"
              className={styles.dialogButton}
              data-testid="spellcheck-change"
              disabled={replacement === current.word}
              onClick={() =>
                void act(() => actions.change(current, replacement), after(current), 'changed')
              }
            >
              {t('spell.change')}
            </button>
            {!current.grammar && (
              <button
                type="button"
                className={styles.dialogButton}
                data-testid="spellcheck-change-all"
                disabled={replacement === current.word}
                onClick={() => {
                  const same = report.issues.filter((x) => !x.grammar && x.word === current.word);
                  void act(
                    () => actions.changeAll(same, replacement),
                    after(current),
                    'changed',
                    same.length,
                  );
                }}
              >
                {t('spell.changeAll')}
              </button>
            )}
          </div>
        </div>
      )}
      {report && !current && (
        <div data-testid="spellcheck-done">
          <p>
            <strong>{t('spell.done')}</strong>
          </p>
          <p data-testid="spellcheck-summary">
            {counts.changed + counts.ignored + counts.added === 0
              ? t('spell.noIssues')
              : t('spell.summary', { ...counts })}
          </p>
          {report.unchecked.length > 0 && (
            <p data-testid="spellcheck-unchecked">
              {t('spell.unchecked', { langs: report.unchecked.map(langName).join(', ') })}
            </p>
          )}
        </div>
      )}
    </Dialog>
  );
}
