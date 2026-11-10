/**
 * Read aloud UI shared by BlueDoc, BlueSheet and BlueSlide (CORE-LANG-003): `useReadAloud` owns
 * a `ReadAloudSession` and renders the floating control bar (play / pause, previous / next unit,
 * stop, speed, voice) with the notices (no voice for the language, nothing to read, platform
 * without speech). Speed and voice are kept per browser / app.
 */
import { useTranslation } from '@blueoffice/i18n';
import { primaryLang, type SpeechProvider, type SpeechVoice } from '@blueoffice/platform';
import { Pause, Play, SkipBack, SkipForward, Square } from 'lucide-react';
import { useCallback, useEffect, useRef, useState, type ReactNode } from 'react';
import styles from './ReadAloudBar.module.css';
import {
  READ_RATES,
  ReadAloudSession,
  type ReadAloudState,
  type ReadPosition,
  type ReadUnit,
} from './readAloud';

/** What one read-aloud run reads and how the editor follows it. */
export interface ReadAloudRequest {
  units: ReadUnit[];
  /** Unit and UTF-16 offset to start at. */
  unit?: number;
  offset?: number;
  /** A word starts: highlight it. */
  onWord?(at: ReadPosition): void;
  /** Reading moved to another unit. */
  onUnit?(unit: number): void;
  /** Reading ended or stopped: clear the highlight. */
  onStop?(): void;
}

/** What the previous / next buttons move by. */
export type ReadUnitKind = 'paragraph' | 'cell' | 'text';

const SETTINGS_KEY = 'bo.readAloud';

interface Kept {
  rate: number;
  voice: string | null;
}

function loadKept(): Kept {
  try {
    const v: unknown = JSON.parse(localStorage.getItem(SETTINGS_KEY) ?? '{}');
    const o = (v && typeof v === 'object' ? v : {}) as Partial<Record<keyof Kept, unknown>>;
    const rate = typeof o.rate === 'number' && o.rate >= 0.1 && o.rate <= 10 ? o.rate : 1;
    const voice = typeof o.voice === 'string' ? o.voice : null;
    return { rate, voice };
  } catch {
    return { rate: 1, voice: null };
  }
}

function saveKept(k: Kept): void {
  try {
    localStorage.setItem(SETTINGS_KEY, JSON.stringify(k));
  } catch {
    // Storage unavailable: the settings last for this session.
  }
}

export interface ReadAloud {
  /** The bar is shown (reading, paused or a notice). */
  active: boolean;
  state: ReadAloudState;
  start(r: ReadAloudRequest): Promise<void>;
  stop(): void;
  togglePause(): void;
  next(): void;
  previous(): void;
  /** The floating bar (render it once; `null` while inactive). */
  bar: ReactNode;
}

export function useReadAloud(
  provider: SpeechProvider | undefined,
  kind: ReadUnitKind = 'paragraph',
): ReadAloud {
  const { t, i18n } = useTranslation();
  const [state, setState] = useState<ReadAloudState>('idle');
  const [open, setOpen] = useState(false);
  const [notice, setNotice] = useState<string | null>(null);
  const [voices, setVoices] = useState<SpeechVoice[]>([]);
  const [kept, setKept] = useState<Kept>(loadKept);
  const session = useRef<ReadAloudSession | null>(null);
  const request = useRef<ReadAloudRequest | null>(null);

  const end = useCallback(() => {
    session.current?.dispose();
    session.current = null;
    const r = request.current;
    request.current = null;
    r?.onStop?.();
    setState('idle');
  }, []);

  const stop = useCallback(() => {
    end();
    setOpen(false);
    setNotice(null);
  }, [end]);

  const start = useCallback(
    async (r: ReadAloudRequest) => {
      end();
      setOpen(true);
      if (!provider) {
        setNotice(t('readAloud.unavailable'));
        return;
      }
      if (!r.units.some((u) => u.text.trim())) {
        setNotice(t('readAloud.nothing'));
        return;
      }
      setNotice(null);
      const s: ReadAloudSession = new ReadAloudSession(
        provider,
        r.units,
        { ...kept, pitch: 1, volume: 1, fallbackLang: i18n.language || 'vi-VN' },
        {
          onWord: (at) => r.onWord?.(at),
          onUnit: (u) => r.onUnit?.(u),
          onState: (st) => {
            if (session.current === s) setState(st);
          },
          onNoVoice: (lang) => {
            setNotice(
              primaryLang(lang) === 'vi'
                ? t('readAloud.noVietnameseVoice')
                : t('readAloud.noVoice', { lang }),
            );
            if (session.current === s) end();
          },
          onError: (error) => {
            setNotice(t('readAloud.error', { error }));
            if (session.current === s) end();
          },
          onDone: () => {
            if (session.current !== s) return;
            end();
            setOpen(false);
          },
        },
      );
      session.current = s;
      request.current = r;
      await s.start(r.unit ?? 0, r.offset);
      setVoices(await s.voices());
    },
    [provider, kept, i18n.language, t, end],
  );

  // Voices installed or loaded late.
  useEffect(() => {
    if (!provider?.onVoicesChanged) return;
    return provider.onVoicesChanged(() => {
      void provider.voices().then(setVoices);
    });
  }, [provider]);

  useEffect(() => () => session.current?.dispose(), []);

  const change = useCallback((next: Partial<Kept>) => {
    setKept((k) => {
      const v = { ...k, ...next };
      saveKept(v);
      return v;
    });
    session.current?.update(next);
  }, []);

  const togglePause = useCallback(() => session.current?.toggle(), []);
  const next = useCallback(() => session.current?.next(), []);
  const previous = useCallback(() => session.current?.previous(), []);

  const playing = state === 'playing';
  const idle = state === 'idle';
  const sorted = [...voices].sort(
    (a, b) => a.lang.localeCompare(b.lang) || a.name.localeCompare(b.name),
  );
  const bar = open ? (
    <div
      className={styles.bar}
      role="toolbar"
      aria-label={t('readAloud.bar')}
      data-testid="read-aloud-bar"
      data-state={state}
      onKeyDown={(e) => {
        if (e.key === 'Escape') {
          e.stopPropagation();
          stop();
        }
      }}
    >
      <button
        type="button"
        className={styles.button}
        title={t(`readAloud.previous.${kind}`)}
        aria-label={t(`readAloud.previous.${kind}`)}
        data-testid="read-aloud-previous"
        disabled={idle}
        onClick={previous}
      >
        <SkipBack size={16} />
      </button>
      <button
        type="button"
        className={styles.button}
        title={playing ? t('readAloud.pause') : t('readAloud.resume')}
        aria-label={playing ? t('readAloud.pause') : t('readAloud.resume')}
        aria-pressed={!playing && !idle}
        data-testid="read-aloud-play"
        disabled={idle}
        onClick={togglePause}
      >
        {playing ? <Pause size={16} /> : <Play size={16} />}
      </button>
      <button
        type="button"
        className={styles.button}
        title={t(`readAloud.next.${kind}`)}
        aria-label={t(`readAloud.next.${kind}`)}
        data-testid="read-aloud-next"
        disabled={idle}
        onClick={next}
      >
        <SkipForward size={16} />
      </button>
      <button
        type="button"
        className={styles.button}
        title={t('readAloud.stop')}
        aria-label={t('readAloud.stop')}
        data-testid="read-aloud-stop"
        onClick={stop}
      >
        <Square size={14} />
      </button>
      <label className={styles.label}>
        {t('readAloud.speed')}
        <select
          className={styles.select}
          data-testid="read-aloud-rate"
          value={String(kept.rate)}
          onChange={(e) => change({ rate: Number(e.target.value) })}
        >
          {(READ_RATES.includes(kept.rate as (typeof READ_RATES)[number])
            ? READ_RATES
            : [...READ_RATES, kept.rate].sort((a, b) => a - b)
          ).map((r) => (
            <option key={r} value={String(r)}>
              {t('readAloud.rate', { value: r.toLocaleString(i18n.language) })}
            </option>
          ))}
        </select>
      </label>
      <label className={styles.label}>
        {t('readAloud.voice')}
        <select
          className={styles.select}
          data-testid="read-aloud-voice"
          value={kept.voice ?? ''}
          onChange={(e) => change({ voice: e.target.value || null })}
        >
          <option value="">{t('readAloud.voiceAuto')}</option>
          {sorted.map((v) => (
            <option key={v.id} value={v.id}>
              {`${v.name} (${v.lang})`}
            </option>
          ))}
        </select>
      </label>
      {notice && (
        <div className={styles.notice} role="status" data-testid="read-aloud-notice">
          {notice}
        </div>
      )}
    </div>
  ) : null;

  return { active: open, state, start, stop, togglePause, next, previous, bar };
}
