/**
 * Dictation UI shared by BlueDoc, BlueSheet and BlueSlide (CORE-LANG-002): `useDictation` owns a
 * `DictationSession` and renders the floating bar — status (listening / paused / error), the
 * interim text (underlined: not written yet), the language picker, pause / resume and stop.
 * The language is kept per browser.
 */
import { useTranslation } from '@blueoffice/i18n';
import {
  DICTATION_LANGUAGES,
  primaryLang,
  type DictationErrorCode,
  type DictationProvider,
} from '@blueoffice/platform';
import { Mic, MicOff, Pause, Square } from 'lucide-react';
import { useCallback, useEffect, useRef, useState, type ReactNode } from 'react';
import styles from './DictationBar.module.css';
import { DictationSession, type DictationState, type DictationTarget } from './dictation';

const LANG_KEY = 'bo.dictation.lang';

function loadLang(uiLang: string): string {
  try {
    const v = localStorage.getItem(LANG_KEY);
    if (v) return v;
  } catch {
    // Storage unavailable.
  }
  return primaryLang(uiLang || 'vi') === 'vi' ? 'vi-VN' : 'en-US';
}

function saveLang(lang: string): void {
  try {
    localStorage.setItem(LANG_KEY, lang);
  } catch {
    // Storage unavailable: kept for this session.
  }
}

/** Display name of a language tag in the UI language. */
function langName(tag: string, uiLang: string): string {
  try {
    const n = new Intl.DisplayNames([uiLang], { type: 'language' }).of(tag);
    return n ? `${n} (${tag})` : tag;
  } catch {
    return tag;
  }
}

/** The editor's dictation hook. */
export interface Dictation {
  /** The bar is shown (listening, paused or a notice). */
  active: boolean;
  state: DictationState;
  lang: string;
  start(): Promise<void>;
  stop(): void;
  /** Start, or stop when running. */
  toggle(): Promise<void>;
  togglePause(): Promise<void>;
  setLang(lang: string): Promise<void>;
  /** Final phrases received so far are written (tests). */
  written(): Promise<void>;
  /** The floating bar (render once; `null` while inactive). */
  bar: ReactNode;
}

type Notice = DictationErrorCode | 'noTarget';

export function useDictation(
  provider: DictationProvider | undefined,
  target: () => DictationTarget | null,
): Dictation {
  const { t, i18n } = useTranslation();
  const [state, setState] = useState<DictationState>('idle');
  const [open, setOpen] = useState(false);
  const [notice, setNotice] = useState<Notice | null>(null);
  const [interim, setInterim] = useState('');
  const [lang, setLangState] = useState(() => loadLang(i18n.language));
  const [langs, setLangs] = useState<string[]>([...DICTATION_LANGUAGES]);
  const session = useRef<DictationSession | null>(null);
  const targetRef = useRef(target);
  useEffect(() => {
    targetRef.current = target;
  });

  const end = useCallback(() => {
    session.current?.dispose();
    session.current = null;
    setState('idle');
    setInterim('');
  }, []);

  const stop = useCallback(() => {
    const s = session.current;
    if (s) {
      s.stop();
      // Results still being recognised are written, then the session goes.
      void s.written().then(() => {
        if (session.current === s) session.current = null;
        s.dispose();
      });
    }
    setState('idle');
    setInterim('');
    setOpen(false);
    setNotice(null);
  }, []);

  const start = useCallback(async () => {
    end();
    setOpen(true);
    setNotice(null);
    if (!provider || !(await provider.available().catch(() => false))) {
      setNotice('unavailable');
      return;
    }
    const tg = targetRef.current();
    if (!tg) {
      setNotice('noTarget');
      return;
    }
    void provider
      .languages()
      .then((l) => {
        if (!l.length) return;
        // Server languages may be primary subtags (`vi`): offer region tags where known.
        const tags = l.map((x) => DICTATION_LANGUAGES.find((d) => primaryLang(d) === x) ?? x);
        setLangs([...new Set(tags)]);
      })
      .catch(() => {});
    const s: DictationSession = new DictationSession(provider, tg, lang, {
      onState: (st) => {
        if (session.current === s) setState(st);
      },
      onError: (code) => {
        if (session.current === s) setNotice(code);
      },
      onInterim: (text) => {
        if (session.current === s) setInterim(text);
      },
      onPhrase: () => {
        if (session.current === s) setNotice(null);
      },
    });
    session.current = s;
    await s.start();
  }, [provider, lang, end]);

  useEffect(() => () => session.current?.dispose(), []);

  const toggle = useCallback(async () => {
    if (open) stop();
    else await start();
  }, [open, start, stop]);

  const togglePause = useCallback(async () => {
    setNotice(null);
    await session.current?.toggle();
  }, []);

  const setLang = useCallback(async (l: string) => {
    setLangState(l);
    saveLang(l);
    await session.current?.setLanguage(l);
  }, []);

  const written = useCallback(() => session.current?.written() ?? Promise.resolve(), []);

  const listening = state === 'listening' || state === 'starting';
  const status = notice
    ? t(`dictation.errors.${notice}`)
    : t(`dictation.status.${state === 'starting' ? 'listening' : state}`);
  const shown = langs.includes(lang) ? langs : [...langs, lang];
  const bar = open ? (
    <div
      className={styles.bar}
      role="toolbar"
      aria-label={t('dictation.bar')}
      data-testid="dictation-bar"
      data-state={state}
      onKeyDown={(e) => {
        if (e.key === 'Escape') {
          e.stopPropagation();
          stop();
        }
      }}
    >
      <span className={listening ? `${styles.mic} ${styles.live}` : styles.mic} aria-hidden="true">
        {notice ? <MicOff size={16} /> : <Mic size={16} />}
      </span>
      <span
        className={notice ? `${styles.status} ${styles.error}` : styles.status}
        role="status"
        data-testid="dictation-status"
      >
        {status}
      </span>
      <button
        type="button"
        className={styles.button}
        title={listening ? t('dictation.pause') : t('dictation.resume')}
        aria-label={listening ? t('dictation.pause') : t('dictation.resume')}
        aria-pressed={!listening}
        data-testid="dictation-pause"
        disabled={state === 'idle' || notice === 'unavailable' || notice === 'noTarget'}
        onClick={() => void togglePause()}
      >
        {listening ? <Pause size={16} /> : <Mic size={16} />}
      </button>
      <button
        type="button"
        className={styles.button}
        title={t('dictation.stop')}
        aria-label={t('dictation.stop')}
        data-testid="dictation-stop"
        onClick={stop}
      >
        <Square size={14} />
      </button>
      <label className={styles.label}>
        {t('dictation.language')}
        <select
          className={styles.select}
          data-testid="dictation-lang"
          value={lang}
          onChange={(e) => void setLang(e.target.value)}
        >
          {shown.map((l) => (
            <option key={l} value={l}>
              {langName(l, i18n.language || 'vi-VN')}
            </option>
          ))}
        </select>
      </label>
      {interim && (
        <div className={styles.interim} data-testid="dictation-interim">
          {interim}
        </div>
      )}
    </div>
  ) : null;

  return { active: open, state, lang, start, stop, toggle, togglePause, setLang, written, bar };
}
