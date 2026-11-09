import { useTranslation } from '@blueoffice/i18n';
import type {
  EditCommand,
  EditResult,
  ProofEdit,
  ProofIssue,
  ProofOptions,
  ProofReport,
} from '@blueoffice/platform';
import { Dialog } from '@blueoffice/ui';
import { useCallback, useEffect, useRef, useState } from 'react';
import styles from './Editor.module.css';
import { saveIgnoredWord, saveUserWord } from './SpellMenu';

type Run = (cmd: EditCommand) => Promise<EditResult | null>;

/** Proofing languages offered in the pickers (BCP 47 tags, §17.18.48 `ST_Lang`). */
export const PROOF_LANGUAGES = [
  'vi-VN',
  'en-US',
  'en-GB',
  'fr-FR',
  'de-DE',
  'zh-CN',
  'ja-JP',
  'ko-KR',
] as const;

const OPTIONS_KEY = 'bo.spell.options';

/** Proofing options kept by this browser / app (null: never changed → engine defaults). */
export function loadProofOptions(): Partial<ProofOptions> | null {
  try {
    const raw = localStorage.getItem(OPTIONS_KEY);
    const v: unknown = raw ? JSON.parse(raw) : null;
    return v && typeof v === 'object' ? (v as Partial<ProofOptions>) : null;
  } catch {
    return null;
  }
}

export function saveProofOptions(o: Partial<ProofOptions>): void {
  try {
    localStorage.setItem(OPTIONS_KEY, JSON.stringify({ ...loadProofOptions(), ...o }));
  } catch {
    // Storage unavailable (private mode): the options last for this session only.
  }
}

/** Byte length of a string in UTF-8 (engine offsets are UTF-8 byte offsets). */
export const utf8Length = (s: string) => new TextEncoder().encode(s).length;

/** Where the walk through the issues stands: story order, paragraph, byte offset. */
export interface ProofCursor {
  order: number;
  para: number;
  start: number;
}

const issueKey = (i: ProofIssue) => `${i.order}:${i.para}:${i.start}:${i.rule ?? ''}:${i.word}`;

const before = (i: ProofIssue, c: ProofCursor) =>
  i.order !== c.order ? i.order < c.order : i.para !== c.para ? i.para < c.para : i.start < c.start;

/** The first issue at or after `cursor` not skipped yet. */
export function nextIssue(
  issues: readonly ProofIssue[],
  cursor: ProofCursor,
  skipped: ReadonlySet<string>,
): ProofIssue | null {
  return issues.find((i) => !before(i, cursor) && !skipped.has(issueKey(i))) ?? null;
}

const edit = (i: ProofIssue, p: Partial<ProofEdit>): ProofEdit => ({
  story: i.story,
  para: i.para,
  start: i.start,
  end: i.end,
  ...p,
});

interface Counts {
  changed: number;
  ignored: number;
  added: number;
}

/**
 * Whole-document spelling and grammar check (F7, CORE-SPELL-005): walks the issues of every story
 * in order, with the sentence, suggestions and the actions to skip, ignore, add, replace or
 * replace everywhere; a summary and readability statistics at the end.
 */
export function SpellCheckDialog({
  run,
  onClose,
  onShow,
}: {
  run: Run;
  onClose: () => void;
  /** Show the issue in the document (select the word of the main text). */
  onShow?: (issue: ProofIssue) => void;
}) {
  const { t } = useTranslation();
  const [report, setReport] = useState<ProofReport | null>(null);
  const [cursor, setCursor] = useState<ProofCursor>({ order: 0, para: 0, start: 0 });
  const [skipped, setSkipped] = useState<ReadonlySet<string>>(new Set());
  const [typed, setTyped] = useState<{ key: string; value: string } | null>(null);
  const [explained, setExplained] = useState('');
  const [counts, setCounts] = useState<Counts>({ changed: 0, ignored: 0, added: 0 });
  const [options, setOptions] = useState<ProofOptions | null>(null);
  const busy = useRef(false);

  const check = useCallback(async () => {
    const r = await run({ cmd: 'proofCheck' });
    if (r?.proof) setReport(r.proof);
  }, [run]);

  useEffect(() => {
    let live = true;
    void run({ cmd: 'proofOptions' }).then((r) => {
      if (live && r?.proofOptions) setOptions(r.proofOptions);
    });
    void run({ cmd: 'proofCheck' }).then((r) => {
      if (live && r?.proof) setReport(r.proof);
    });
    return () => {
      live = false;
    };
  }, [run]);

  const current = report ? nextIssue(report.issues, cursor, skipped) : null;
  const currentKey = current ? issueKey(current) : '';
  // Typed replacement and explanation belong to the issue they were made for.
  const replacement =
    typed?.key === currentKey ? typed.value : (current?.suggestions[0] ?? current?.word ?? '');
  const setReplacement = (value: string) => setTyped({ key: currentKey, value });
  const explain = explained === currentKey;
  const setExplain = (on: boolean) => setExplained(on ? currentKey : '');
  const shownRef = useRef('');
  useEffect(() => {
    if (current && shownRef.current !== currentKey) {
      shownRef.current = currentKey;
      onShow?.(current);
    }
  }, [current, currentKey, onShow]);

  /** Run an action once at a time, then check again from `at`. */
  const act = async (f: () => Promise<void>, at: ProofCursor, count?: keyof Counts, n = 1) => {
    if (busy.current) return;
    busy.current = true;
    try {
      await f();
      if (count) setCounts((c) => ({ ...c, [count]: c[count] + n }));
      setCursor(at);
      await check();
    } finally {
      busy.current = false;
    }
  };

  const here = (i: ProofIssue): ProofCursor => ({ order: i.order, para: i.para, start: i.start });

  const skipOnce = (i: ProofIssue) => {
    setSkipped((s) => new Set([...s, issueKey(i)]));
    setCounts((c) => ({ ...c, ignored: c.ignored + 1 }));
    setCursor(here(i));
  };

  const ignoreAll = (i: ProofIssue) =>
    act(
      async () => {
        if (i.rule) {
          await run({ cmd: 'grammarIgnore', rule: i.rule, text: i.word });
        } else {
          await run({ cmd: 'spellIgnore', word: i.word });
          saveIgnoredWord(i.word);
        }
      },
      here(i),
      'ignored',
    );

  const addWord = (i: ProofIssue) =>
    act(
      async () => {
        await run({ cmd: 'spellAddWord', word: i.word });
        saveUserWord(i.word);
      },
      here(i),
      'added',
    );

  const change = (i: ProofIssue) =>
    act(
      async () => {
        await run({ cmd: 'proofEdit', edits: [edit(i, { text: replacement })] });
      },
      { ...here(i), start: i.start + utf8Length(replacement) },
      'changed',
    );

  const changeAll = (i: ProofIssue) => {
    const same = (report?.issues ?? []).filter((x) => !x.grammar && x.word === i.word);
    return act(
      async () => {
        await run({ cmd: 'proofEdit', edits: same.map((x) => edit(x, { text: replacement })) });
      },
      { ...here(i), start: i.start + utf8Length(replacement) },
      'changed',
      same.length,
    );
  };

  const setLanguage = (i: ProofIssue, lang: string) =>
    act(async () => {
      await run({ cmd: 'proofEdit', edits: [edit(i, { lang })] });
    }, here(i));

  const setOption = (o: Partial<ProofOptions>) =>
    act(async () => {
      const r = await run({ cmd: 'proofOptions', ...o });
      if (r?.proofOptions) setOptions(r.proofOptions);
      saveProofOptions(o);
    }, cursor);

  const langName = (l: string) =>
    (PROOF_LANGUAGES as readonly string[]).includes(l) ? t(`spell.lang.${l}`) : l;

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
      {!report && <p>{t('spell.checking')}</p>}
      {report && current && (
        <div data-testid="spellcheck-issue">
          <div style={{ display: 'flex', justifyContent: 'space-between', fontSize: 12 }}>
            <strong data-testid="spellcheck-kind">
              {current.grammar ? t('spell.grammarIssue') : t('spell.notInDictionary')}
            </strong>
            <span data-testid="spellcheck-where">
              {t('spell.where', { story: t(`spell.story.${current.story.kind}`) })}
            </span>
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
          {explain && current.rule && (
            <p style={{ margin: '4px 0', fontSize: 12 }} data-testid="spellcheck-explain">
              {t(`spell.why.${current.rule}`)}
            </p>
          )}
          <label className={styles.field}>
            <span>{t('spell.changeTo')}</span>
            <input
              data-testid="spellcheck-replacement"
              value={replacement}
              onChange={(e) => setReplacement(e.target.value)}
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
                onClick={() => setReplacement(s)}
                onDoubleClick={() => void change(current)}
                onKeyDown={(e) => e.key === 'Enter' && setReplacement(s)}
              >
                {s}
              </div>
            ))}
          </div>
          {!current.grammar && (
            <label className={styles.field}>
              <span>{t('spell.language')}</span>
              <select
                data-testid="spellcheck-language"
                value={current.lang ?? ''}
                onChange={(e) => void setLanguage(current, e.target.value)}
              >
                {!current.lang && <option value="">{t('spell.lang.none')}</option>}
                {PROOF_LANGUAGES.map((l) => (
                  <option key={l} value={l}>
                    {langName(l)}
                  </option>
                ))}
              </select>
            </label>
          )}
          <div style={{ display: 'flex', flexWrap: 'wrap', gap: 6, marginTop: 8 }}>
            <button
              type="button"
              className={styles.dialogButton}
              data-testid="spellcheck-ignore-once"
              onClick={() => skipOnce(current)}
            >
              {t('spell.ignoreOnce')}
            </button>
            <button
              type="button"
              className={styles.dialogButton}
              data-testid="spellcheck-ignore-all"
              onClick={() => void ignoreAll(current)}
            >
              {t('spell.ignoreAll')}
            </button>
            {!current.grammar && (
              <button
                type="button"
                className={styles.dialogButton}
                data-testid="spellcheck-add"
                onClick={() => void addWord(current)}
              >
                {t('spell.addToDictionary')}
              </button>
            )}
            <button
              type="button"
              className={styles.dialogButton}
              data-testid="spellcheck-change"
              disabled={replacement === current.word}
              onClick={() => void change(current)}
            >
              {t('spell.change')}
            </button>
            {!current.grammar && (
              <button
                type="button"
                className={styles.dialogButton}
                data-testid="spellcheck-change-all"
                disabled={replacement === current.word}
                onClick={() => void changeAll(current)}
              >
                {t('spell.changeAll')}
              </button>
            )}
            {current.grammar && current.rule && (
              <button
                type="button"
                className={styles.dialogButton}
                data-testid="spellcheck-explain-button"
                aria-pressed={explain}
                onClick={() => setExplain(!explain)}
              >
                {t('spell.explain')}
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
          {counts.changed + counts.ignored + counts.added === 0 ? (
            <p data-testid="spellcheck-summary">{t('spell.noIssues')}</p>
          ) : (
            <p data-testid="spellcheck-summary">{t('spell.summary', { ...counts })}</p>
          )}
          {report.unchecked.length > 0 && (
            <p data-testid="spellcheck-unchecked">
              {t('spell.unchecked', { langs: report.unchecked.map(langName).join(', ') })}
            </p>
          )}
          <table data-testid="spellcheck-stats" style={{ fontSize: 13 }}>
            <caption style={{ textAlign: 'left', fontWeight: 600 }}>{t('spell.stats')}</caption>
            <tbody>
              {(
                [
                  ['statWords', report.stats.words],
                  ['statCharacters', report.stats.characters],
                  ['statParagraphs', report.stats.paragraphs],
                  ['statSentences', report.stats.sentences],
                  ['statSentencesPerParagraph', report.stats.sentencesPerParagraph.toFixed(1)],
                  ['statWordsPerSentence', report.stats.wordsPerSentence.toFixed(1)],
                  ['statCharactersPerWord', report.stats.charactersPerWord.toFixed(1)],
                ] as const
              ).map(([k, v]) => (
                <tr key={k}>
                  <th style={{ textAlign: 'left', fontWeight: 400, paddingRight: 16 }}>
                    {t(`spell.${k}`)}
                  </th>
                  <td data-testid={`spellcheck-${k}`}>{v}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}
      {options && (
        <fieldset style={{ marginTop: 12, fontSize: 12 }}>
          <legend>{t('spell.options')}</legend>
          <label style={{ display: 'block' }}>
            <input
              type="checkbox"
              data-testid="spellcheck-ignore-caps"
              checked={options.ignoreCaps}
              onChange={(e) => void setOption({ ignoreCaps: e.target.checked })}
            />{' '}
            {t('spell.ignoreCaps')}
          </label>
          <label style={{ display: 'block' }}>
            <input
              type="checkbox"
              data-testid="spellcheck-ignore-digits"
              checked={options.ignoreDigits}
              onChange={(e) => void setOption({ ignoreDigits: e.target.checked })}
            />{' '}
            {t('spell.ignoreDigits')}
          </label>
        </fieldset>
      )}
    </Dialog>
  );
}

/**
 * Proofing language of the selection (status bar / Review tab): the language to check it in, "do
 * not check" and the automatic language option. Both changes are undoable edits.
 */
export function LanguageDialog({
  run,
  lang,
  noProof,
  onClose,
}: {
  run: Run;
  /** Current language of the selection (null: not set). */
  lang: string | null;
  noProof: boolean;
  onClose: () => void;
}) {
  const { t } = useTranslation();
  const [value, setValue] = useState(lang ?? '');
  const [skip, setSkip] = useState(noProof);
  const [auto, setAuto] = useState<boolean | null>(null);

  useEffect(() => {
    let live = true;
    void run({ cmd: 'proofOptions' }).then((r) => {
      if (live && r?.proofOptions) setAuto(r.proofOptions.autoLanguage);
    });
    return () => {
      live = false;
    };
  }, [run]);

  const ok = async () => {
    if (value !== (lang ?? ''))
      await run({ cmd: 'formatText', format: { kind: 'language', lang: value || null } });
    if (skip !== noProof) await run({ cmd: 'formatText', format: { kind: 'noProof', on: skip } });
    if (auto !== null) {
      await run({ cmd: 'proofOptions', autoLanguage: auto });
      saveProofOptions({ autoLanguage: auto });
    }
    onClose();
  };

  const options = (PROOF_LANGUAGES as readonly string[]).includes(value)
    ? PROOF_LANGUAGES
    : [...PROOF_LANGUAGES, value].filter(Boolean);

  return (
    <Dialog
      open
      onOpenChange={(o) => !o && onClose()}
      title={t('spell.languageTitle')}
      testId="language-dialog"
      footer={
        <>
          <button type="button" className={styles.dialogButton} onClick={onClose}>
            {t('spell.cancel')}
          </button>
          <button
            type="button"
            className={styles.dialogPrimary}
            data-testid="language-ok"
            onClick={() => void ok()}
          >
            {t('spell.ok')}
          </button>
        </>
      }
    >
      <label className={styles.field}>
        <span>{t('spell.languageLabel')}</span>
        <select
          data-testid="language-select"
          value={value}
          onChange={(e) => setValue(e.target.value)}
        >
          <option value="">{t('spell.lang.none')}</option>
          {options.map((l) => (
            <option key={l} value={l}>
              {(PROOF_LANGUAGES as readonly string[]).includes(l) ? t(`spell.lang.${l}`) : l}
            </option>
          ))}
        </select>
      </label>
      <label style={{ display: 'block', marginTop: 8 }}>
        <input
          type="checkbox"
          data-testid="language-noproof"
          checked={skip}
          onChange={(e) => setSkip(e.target.checked)}
        />{' '}
        {t('spell.noProof')}
      </label>
      <label style={{ display: 'block', marginTop: 4 }}>
        <input
          type="checkbox"
          data-testid="language-auto"
          checked={auto ?? false}
          disabled={auto === null}
          onChange={(e) => setAuto(e.target.checked)}
        />{' '}
        {t('spell.autoLanguage')}
      </label>
    </Dialog>
  );
}
