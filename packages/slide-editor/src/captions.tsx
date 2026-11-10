/**
 * Live subtitles in the slide show (SLIDE-REMOTE-001): the presenter's speech through the
 * platform's `DictationProvider` (the BlueOffice server's speech-to-text service when an
 * administrator configured one, docs/deploy/dictation.md), optionally translated through the
 * `TranslationProvider`, shown over the slide. Without a recogniser the option is off and says
 * why. Nothing is written into the deck.
 */
import { useTranslation } from '@blueoffice/i18n';
import {
  DICTATION_LANGUAGES,
  primaryLang,
  type DictationErrorCode,
  type DictationProvider,
  type DictationSession,
  type TranslationLanguage,
  type TranslationProvider,
} from '@blueoffice/platform';
import { Dialog } from '@blueoffice/ui';
import { useEffect, useRef, useState } from 'react';
import styles from './remote.module.css';

export interface CaptionSettings {
  enabled: boolean;
  /** Spoken language (BCP 47). */
  lang: string;
  /** Shown in this language (primary subtag) instead; `null`: as spoken. */
  translateTo: string | null;
  position: 'bottom' | 'top';
  size: 'small' | 'medium' | 'large';
}

export const DEFAULT_CAPTIONS: CaptionSettings = {
  enabled: false,
  lang: 'vi-VN',
  translateTo: null,
  position: 'bottom',
  size: 'medium',
};

/**
 * The last finished phrases (at most `maxChars`, cut at a word) and the phrase being spoken.
 */
export class CaptionTrack {
  private finals: string[] = [];
  interim = '';

  constructor(private readonly maxChars = 140) {}

  addFinal(text: string): void {
    const s = text.trim();
    this.interim = '';
    if (!s) return;
    this.finals.push(s);
    if (this.finals.length > 6) this.finals.shift();
  }

  setInterim(text: string): void {
    this.interim = text.trim();
  }

  clear(): void {
    this.finals = [];
    this.interim = '';
  }

  /** Text of the finished phrases that fits. */
  get text(): string {
    let s = this.finals.join(' ');
    if (s.length <= this.maxChars) return s;
    s = s.slice(s.length - this.maxChars);
    const space = s.indexOf(' ');
    return space >= 0 && space < 20 ? s.slice(space + 1) : s;
  }
}

export type CaptionError = DictationErrorCode | 'translate';

export interface LiveCaptions {
  /** A recogniser is there (`null`: still asking). */
  available: boolean | null;
  /** Languages the translation service offers (empty: no translation). */
  targets: TranslationLanguage[];
  text: string;
  interim: string;
  listening: boolean;
  error: CaptionError | null;
}

/** Restarts after the recogniser stops on its own (silence) while the captions stay on. */
const RESTART_MS = 250;

export function useLiveCaptions({
  dictation,
  translation,
  settings,
  active,
}: {
  dictation: DictationProvider | undefined;
  translation: TranslationProvider | undefined;
  settings: CaptionSettings;
  /** The show runs and the captions are on. */
  active: boolean;
}): LiveCaptions {
  const [available, setAvailable] = useState<boolean | null>(dictation ? null : false);
  const [targets, setTargets] = useState<TranslationLanguage[]>([]);
  const [text, setText] = useState('');
  const [interim, setInterim] = useState('');
  const [listening, setListening] = useState(false);
  const [error, setError] = useState<CaptionError | null>(null);
  const track = useRef(new CaptionTrack());

  useEffect(() => {
    let cancelled = false;
    if (!dictation) return;
    void dictation
      .available()
      .then((a) => !cancelled && setAvailable(a))
      .catch(() => !cancelled && setAvailable(false));
    return () => {
      cancelled = true;
    };
  }, [dictation]);

  useEffect(() => {
    let cancelled = false;
    void (translation?.languages() ?? Promise.resolve([]))
      .then((l) => !cancelled && setTargets(l))
      .catch(() => !cancelled && setTargets([]));
    return () => {
      cancelled = true;
    };
  }, [translation]);

  const { lang, translateTo } = settings;
  const on = active && available === true && !!dictation;
  useEffect(() => {
    if (!on || !dictation) return;
    let stopped = false;
    let session: DictationSession | null = null;
    let timer: ReturnType<typeof setTimeout> | null = null;
    // Translations finish in order of the phrases.
    let queue = Promise.resolve();
    const tr = track.current;
    const from = primaryLang(lang);
    const to = translateTo && translateTo !== from ? translateTo : null;
    const final = (phrase: string) => {
      if (!to || !translation) {
        tr.addFinal(phrase);
        setText(tr.text);
        setInterim('');
        return;
      }
      setInterim('');
      queue = queue.then(async () => {
        let shown = phrase;
        try {
          shown = (await translation.translate([phrase], from, to))[0] ?? phrase;
        } catch {
          if (!stopped) setError('translate');
        }
        if (stopped) return;
        tr.addFinal(shown);
        setText(tr.text);
      });
    };
    const start = () => {
      if (stopped) return;
      session = dictation.start(lang, { interim: true }, (e) => {
        if (stopped) return;
        if (e.type === 'start') setListening(true);
        else if (e.type === 'result') {
          if (e.final) final(e.text);
          // Interim text is only shown untranslated.
          else if (!to) setInterim(e.text);
        } else if (e.type === 'end') {
          setListening(false);
          timer = setTimeout(start, RESTART_MS);
        } else if (e.type === 'error') {
          setListening(false);
          setError(e.code);
        }
      });
    };
    start();
    return () => {
      stopped = true;
      if (timer) clearTimeout(timer);
      session?.abort();
      // The next show starts with empty captions.
      tr.clear();
      setText('');
      setInterim('');
      setError(null);
      setListening(false);
    };
  }, [on, dictation, translation, lang, translateTo]);

  return { available, targets, text, interim, listening, error };
}

/** The captions over the show. */
export function CaptionOverlay({
  captions,
  settings,
}: {
  captions: LiveCaptions;
  settings: CaptionSettings;
}) {
  const { t } = useTranslation();
  const pos = settings.position === 'top' ? styles.captionsTop : styles.captionsBottom;
  const size =
    settings.size === 'small'
      ? styles.captionsSmall
      : settings.size === 'large'
        ? styles.captionsLarge
        : styles.captionsMedium;
  const err = captions.error;
  if (!captions.text && !captions.interim && !err) return null;
  return (
    <div
      className={`${styles.captions} ${pos} ${size}`}
      data-testid="show-captions"
      data-position={settings.position}
      data-size={settings.size}
      role="status"
      aria-live="polite"
      aria-label={t('remoteShow.captions')}
    >
      {err && err !== 'translate' ? (
        <span data-testid="captions-error">{t(`dictation.errors.${err}`)}</span>
      ) : (
        <>
          <span data-testid="captions-text">{captions.text}</span>
          {captions.interim && (
            <span className={styles.interim} data-testid="captions-interim">
              {captions.text ? ' ' : ''}
              {captions.interim}
            </span>
          )}
        </>
      )}
    </div>
  );
}

/** Caption settings: on / off, spoken language, translation, position, size. */
export function CaptionDialog({
  settings,
  captions,
  onChange,
  onClose,
}: {
  settings: CaptionSettings;
  captions: LiveCaptions;
  onChange: (s: CaptionSettings) => void;
  onClose: () => void;
}) {
  const { t } = useTranslation();
  const [draft, setDraft] = useState(settings);
  const off = captions.available === false;
  const set = (p: Partial<CaptionSettings>) => setDraft((d) => ({ ...d, ...p }));
  return (
    <Dialog
      open
      onOpenChange={(o) => !o && onClose()}
      title={t('remoteShow.captionsTitle')}
      testId="captions-dialog"
      width={420}
      footer={
        <>
          <button type="button" data-testid="captions-cancel" onClick={onClose}>
            {t('showTools.close')}
          </button>
          <button
            type="button"
            data-testid="captions-apply"
            disabled={off}
            onClick={() => {
              onChange(draft);
              onClose();
            }}
          >
            {t('showTools.save')}
          </button>
        </>
      }
    >
      {off && (
        <p className={styles.warn} role="alert" data-testid="captions-unavailable">
          {t('remoteShow.captionsUnavailable')}
        </p>
      )}
      <div className={styles.form}>
        <label htmlFor="cap-on">{t('remoteShow.captionsOn')}</label>
        <input
          id="cap-on"
          type="checkbox"
          data-testid="captions-on"
          disabled={off}
          checked={draft.enabled && !off}
          onChange={(e) => set({ enabled: e.target.checked })}
        />
        <label htmlFor="cap-lang">{t('remoteShow.captionsSpoken')}</label>
        <select
          id="cap-lang"
          data-testid="captions-lang"
          disabled={off}
          value={draft.lang}
          onChange={(e) => set({ lang: e.target.value })}
        >
          {DICTATION_LANGUAGES.map((l) => (
            <option key={l} value={l}>
              {t(`spell.lang.${l}`, { defaultValue: l })}
            </option>
          ))}
        </select>
        <label htmlFor="cap-tr">{t('remoteShow.captionsTranslate')}</label>
        <select
          id="cap-tr"
          data-testid="captions-translate"
          disabled={off || captions.targets.length === 0}
          value={draft.translateTo ?? ''}
          onChange={(e) => set({ translateTo: e.target.value || null })}
        >
          <option value="">{t('remoteShow.captionsNoTranslate')}</option>
          {captions.targets.map((l) => (
            <option key={l.code} value={l.code}>
              {t(`translate.languages.${l.code}`, { defaultValue: l.code })}
            </option>
          ))}
        </select>
        <label htmlFor="cap-pos">{t('remoteShow.captionsPosition')}</label>
        <select
          id="cap-pos"
          data-testid="captions-position"
          disabled={off}
          value={draft.position}
          onChange={(e) => set({ position: e.target.value === 'top' ? 'top' : 'bottom' })}
        >
          <option value="bottom">{t('remoteShow.bottom')}</option>
          <option value="top">{t('remoteShow.top')}</option>
        </select>
        <label htmlFor="cap-size">{t('remoteShow.captionsSize')}</label>
        <select
          id="cap-size"
          data-testid="captions-size"
          disabled={off}
          value={draft.size}
          onChange={(e) =>
            set({
              size:
                e.target.value === 'small'
                  ? 'small'
                  : e.target.value === 'large'
                    ? 'large'
                    : 'medium',
            })
          }
        >
          <option value="small">{t('remoteShow.small')}</option>
          <option value="medium">{t('remoteShow.medium')}</option>
          <option value="large">{t('remoteShow.large')}</option>
        </select>
      </div>
      {captions.targets.length === 0 && !off && (
        <p className={styles.muted} data-testid="captions-no-translation">
          {t('remoteShow.captionsNoTranslation')}
        </p>
      )}
    </Dialog>
  );
}
