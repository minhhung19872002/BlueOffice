/**
 * Translation pane shared by BlueDoc, BlueSheet and BlueSlide (CORE-LANG-001): `useTranslate`
 * renders a floating task pane with the source / target languages, the selected text and its
 * translation (editable before use) with Insert / Replace / Copy, and the app's bulk actions
 * (translate the whole document as a copy, the selected cells, the whole deck) with progress and
 * cancellation. Translation goes through the platform's `TranslationProvider` only; without one
 * (nothing configured by the administrator) the pane says so and sends nothing.
 */
import { useTranslation } from '@blueoffice/i18n';
import {
  TranslationError,
  defaultTarget,
  translateTexts,
  translationError,
  type ClipboardAccess,
  type TranslationErrorCode,
  type TranslationProvider,
} from '@blueoffice/platform';
import { ArrowLeftRight, X } from 'lucide-react';
import { useCallback, useEffect, useRef, useState, type ReactNode } from 'react';
import styles from './TranslatePane.module.css';

/** What the pane translates and where the result goes. */
export interface TranslateSource {
  /** The selected text ('' when nothing is selected). */
  text: string;
  /** Insert the translation after the selection. */
  insert?(text: string): Promise<void> | void;
  /** Replace the selection by the translation. */
  replace?(text: string): Promise<void> | void;
}

/** Context of a bulk action run. */
export interface TranslateRun {
  from: string;
  to: string;
  signal: AbortSignal;
  onProgress(done: number, total: number): void;
}

/** A bulk translation (whole document, selected cells, whole deck). */
export interface TranslateAction {
  id: string;
  label: string;
  /** Runs the action; the returned text is shown as the outcome. */
  run(ctx: TranslateRun): Promise<string | void>;
}

export interface TranslateOpen {
  source: TranslateSource;
  actions?: TranslateAction[];
  /** Run this action at once (the commands that translate everything). */
  autoRun?: string;
}

export interface Translate {
  active: boolean;
  open(o: TranslateOpen): void;
  close(): void;
  /** The floating pane (render it once; `null` while closed). */
  pane: ReactNode;
}

const KEPT_KEY = 'bo.translate';

interface Kept {
  from: string;
  to: string | null;
}

function loadKept(): Kept {
  try {
    const v = JSON.parse(localStorage.getItem(KEPT_KEY) ?? '{}') as Partial<
      Record<string, unknown>
    >;
    return {
      from: typeof v.from === 'string' ? v.from : 'auto',
      to: typeof v.to === 'string' ? v.to : null,
    };
  } catch {
    return { from: 'auto', to: null };
  }
}

function saveKept(k: Kept): void {
  try {
    localStorage.setItem(KEPT_KEY, JSON.stringify(k));
  } catch {
    // Storage unavailable: the choice lasts for this session.
  }
}

type Busy = { done: number; total: number } | null;

export function useTranslate(
  provider: TranslationProvider | undefined,
  clipboard?: Pick<ClipboardAccess, 'writeText'>,
): Translate {
  const { t, i18n } = useTranslation();
  const [req, setReq] = useState<TranslateOpen | null>(null);
  const [langs, setLangs] = useState<string[] | null>(null);
  const [kept, setKept] = useState<Kept>(loadKept);
  const [result, setResult] = useState('');
  const [busy, setBusy] = useState<Busy>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const abort = useRef<AbortController | null>(null);
  /** Bumped by each open / close: a late language lookup of an older one is ignored. */
  const opening = useRef(0);
  const uiLang = i18n.language || 'vi-VN';
  const to = kept.to ?? defaultTarget(uiLang);

  const name = useCallback(
    (code: string) => {
      const key = `translate.languages.${code}`;
      if (i18n.exists(key)) return t(key);
      try {
        return new Intl.DisplayNames([uiLang], { type: 'language' }).of(code) ?? code;
      } catch {
        return code;
      }
    },
    [t, i18n, uiLang],
  );

  const errorText = useCallback(
    (e: unknown) => {
      const code: TranslationErrorCode = translationError(e).code;
      return t(`translate.errors.${code}`);
    },
    [t],
  );

  const cancel = useCallback(() => {
    abort.current?.abort();
    abort.current = null;
  }, []);

  const close = useCallback(() => {
    opening.current++;
    cancel();
    setReq(null);
    setBusy(null);
    setNotice(null);
    setResult('');
  }, [cancel]);

  useEffect(() => () => abort.current?.abort(), []);

  const change = useCallback((next: Partial<Kept>) => {
    setKept((k) => {
      const v = { ...k, ...next };
      saveKept(v);
      return v;
    });
  }, []);

  /** Runs `work` with a fresh abort controller and the busy state. */
  const guarded = useCallback(
    async (
      work: (signal: AbortSignal, progress: (d: number, n: number) => void) => Promise<void>,
    ) => {
      cancel();
      const ac = new AbortController();
      abort.current = ac;
      setNotice(null);
      setBusy({ done: 0, total: 0 });
      try {
        await work(ac.signal, (done, total) => {
          if (abort.current === ac) setBusy({ done, total });
        });
      } catch (e) {
        // A run replaced by a newer one says nothing.
        if (abort.current === ac || abort.current === null) setNotice(errorText(e));
      } finally {
        if (abort.current === ac || abort.current === null) {
          abort.current = null;
          setBusy(null);
        }
      }
    },
    [cancel, errorText],
  );

  const translateSelection = useCallback(
    (text: string) =>
      guarded(async (signal) => {
        if (!provider) throw new TranslationError('unavailable');
        const [out = ''] = await translateTexts(provider, [text], kept.from, to, { signal });
        setResult(out);
      }),
    [guarded, provider, kept.from, to],
  );

  const runAction = useCallback(
    (a: TranslateAction) =>
      guarded(async (signal, onProgress) => {
        if (!provider) throw new TranslationError('unavailable');
        const msg = await a.run({ from: kept.from, to, signal, onProgress });
        if (msg) setNotice(msg);
      }),
    [guarded, provider, kept.from, to],
  );

  // Opening looks up the provider's languages, then translates the selection or runs the
  // asked action.
  const open = useCallback(
    (o: TranslateOpen) => {
      cancel();
      setBusy(null);
      setNotice(null);
      setResult('');
      setLangs(null);
      setReq(o);
      const gen = ++opening.current;
      void (async () => {
        let codes: string[];
        try {
          codes = provider ? (await provider.languages()).map((x) => x.code) : [];
        } catch {
          codes = [];
        }
        if (opening.current !== gen) return;
        setLangs(codes);
        if (!codes.length) return;
        const auto = o.autoRun && o.actions?.find((a) => a.id === o.autoRun);
        if (auto) await runAction(auto);
        else if (o.source.text.trim()) await translateSelection(o.source.text);
      })();
    },
    [cancel, provider, runAction, translateSelection],
  );

  const available = !!langs?.length;
  const offered = langs ?? [];
  const toChoices = offered.includes(to) || !offered.length ? offered : [...offered, to];
  const source = req?.source;
  const pane = req ? (
    <aside
      className={styles.pane}
      role="dialog"
      aria-label={t('translate.title')}
      data-testid="translate-pane"
      data-busy={busy ? 'true' : 'false'}
      onKeyDown={(e) => {
        if (e.key === 'Escape') {
          e.stopPropagation();
          if (busy) cancel();
          else close();
        }
      }}
    >
      <header className={styles.header}>
        <h2 className={styles.title}>{t('translate.title')}</h2>
        <button
          type="button"
          className={styles.icon}
          title={t('translate.close')}
          aria-label={t('translate.close')}
          data-testid="translate-close"
          onClick={close}
        >
          <X size={16} />
        </button>
      </header>
      {langs !== null && !available ? (
        <p className={styles.notice} role="status" data-testid="translate-unavailable">
          {t('translate.unavailable')}
        </p>
      ) : (
        <>
          <div className={styles.langs}>
            <label className={styles.label}>
              {t('translate.from')}
              <select
                className={styles.select}
                data-testid="translate-from"
                value={kept.from}
                onChange={(e) => change({ from: e.target.value })}
              >
                <option value="auto">{t('translate.detect')}</option>
                {offered.map((c) => (
                  <option key={c} value={c}>
                    {name(c)}
                  </option>
                ))}
              </select>
            </label>
            <button
              type="button"
              className={styles.icon}
              title={t('translate.swap')}
              aria-label={t('translate.swap')}
              disabled={kept.from === 'auto'}
              onClick={() => change({ from: to, to: kept.from })}
            >
              <ArrowLeftRight size={16} />
            </button>
            <label className={styles.label}>
              {t('translate.to')}
              <select
                className={styles.select}
                data-testid="translate-to"
                value={to}
                onChange={(e) => change({ to: e.target.value })}
              >
                {toChoices.map((c) => (
                  <option key={c} value={c}>
                    {name(c)}
                  </option>
                ))}
              </select>
            </label>
          </div>
          <div className={styles.section}>
            <div className={styles.caption}>{t('translate.selection')}</div>
            {source?.text.trim() ? (
              <div className={styles.source} data-testid="translate-source">
                {source.text}
              </div>
            ) : (
              <p className={styles.hint}>{t('translate.noSelection')}</p>
            )}
            <button
              type="button"
              className={styles.primary}
              data-testid="translate-run"
              disabled={!!busy || !source?.text.trim()}
              onClick={() => source && void translateSelection(source.text)}
            >
              {t('translate.translate')}
            </button>
          </div>
          <div className={styles.section}>
            <label className={styles.caption} htmlFor="bo-translate-result">
              {t('translate.result')}
            </label>
            <textarea
              id="bo-translate-result"
              className={styles.result}
              data-testid="translate-result"
              value={result}
              rows={5}
              onChange={(e) => setResult(e.target.value)}
            />
            <div className={styles.row}>
              {source?.insert && (
                <button
                  type="button"
                  className={styles.button}
                  data-testid="translate-insert"
                  disabled={!result || !!busy}
                  onClick={() => void source.insert?.(result)}
                >
                  {t('translate.insert')}
                </button>
              )}
              {source?.replace && (
                <button
                  type="button"
                  className={styles.button}
                  data-testid="translate-replace"
                  disabled={!result || !!busy}
                  onClick={() => void source.replace?.(result)}
                >
                  {t('translate.replace')}
                </button>
              )}
              <button
                type="button"
                className={styles.button}
                data-testid="translate-copy"
                disabled={!result || !clipboard}
                onClick={() => void clipboard?.writeText(result)}
              >
                {t('translate.copy')}
              </button>
            </div>
          </div>
          {!!req.actions?.length && (
            <div className={styles.section}>
              {req.actions.map((a) => (
                <button
                  key={a.id}
                  type="button"
                  className={styles.button}
                  data-testid={`translate-action-${a.id}`}
                  disabled={!!busy}
                  onClick={() => void runAction(a)}
                >
                  {a.label}
                </button>
              ))}
            </div>
          )}
          {busy && (
            <div className={styles.row} role="status" data-testid="translate-progress">
              <progress
                className={styles.progress}
                max={busy.total || 1}
                value={busy.total ? busy.done : undefined}
              />
              <span>
                {busy.total
                  ? t('translate.progress', { done: busy.done, total: busy.total })
                  : t('translate.working')}
              </span>
              <button
                type="button"
                className={styles.button}
                data-testid="translate-cancel"
                onClick={cancel}
              >
                {t('translate.cancel')}
              </button>
            </div>
          )}
        </>
      )}
      {notice && (
        <p className={styles.notice} role="status" data-testid="translate-notice">
          {notice}
        </p>
      )}
      <p className={styles.privacy}>{t('translate.privacy')}</p>
    </aside>
  ) : null;

  return { active: !!req, open, close, pane };
}
