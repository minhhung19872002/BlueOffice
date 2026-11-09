import { useTranslation } from '@blueoffice/i18n';
import type {
  DisplayCapabilities,
  DisplayScreen,
  DisplayTarget,
  SlideCustomShow,
  SlideShowSetup,
  SlideShowType,
} from '@blueoffice/platform';
import { Dialog } from '@blueoffice/ui';
import { useState } from 'react';
import styles from './showTools.module.css';

/** Mounted while open: the fields start from `setup` and `target` each time. */
export interface SetupShowDialogProps {
  open: boolean;
  setup: SlideShowSetup;
  customShows: SlideCustomShow[];
  count: number;
  /** What the host can do for a second screen (absent: no choice). */
  displays?: DisplayCapabilities;
  /** List the screens (may ask for permission). */
  findScreens?: () => Promise<DisplayScreen[]>;
  target: DisplayTarget;
  onClose: () => void;
  onApply: (setup: SlideShowSetup, target: DisplayTarget) => void;
}

const targetKey = (t: DisplayTarget) => (t.kind === 'screen' ? `screen:${t.id}` : t.kind);

function targetOf(key: string): DisplayTarget {
  if (key.startsWith('screen:')) return { kind: 'screen', id: key.slice(7) };
  if (key === 'presentation') return { kind: 'presentation' };
  return { kind: 'window' };
}

/**
 * Show setup (§19.2.1.30 showPr): how the show is presented, which slides,
 * looping, narration / animation, timings, pen colour — and (not saved in the
 * file) on which screen the audience sees it.
 */
export function SetupShowDialog({
  open,
  setup,
  customShows,
  count,
  displays,
  findScreens,
  target,
  onClose,
  onApply,
}: SetupShowDialogProps) {
  const { t } = useTranslation();
  const [s, setS] = useState<SlideShowSetup>(setup);
  const [where, setWhere] = useState(targetKey(target));
  const [screens, setScreens] = useState<DisplayScreen[]>([]);
  const [busy, setBusy] = useState(false);

  const kiosk = s.showType === 'kiosk';
  const type = (v: SlideShowType) => (
    <label className={styles.row}>
      <input
        type="radio"
        name="setup-type"
        data-testid={`setup-type-${v}`}
        checked={s.showType === v}
        onChange={() => setS({ ...s, showType: v })}
      />
      {t(`showTools.type.${v}`)}
    </label>
  );
  const range = s.range;
  const from = range.kind === 'range' ? range.from : 1;
  const to = range.kind === 'range' ? range.to : Math.max(1, count);
  const firstShow = customShows[0]?.id;
  const num = (v: string, lo: number, hi: number) =>
    Math.max(lo, Math.min(hi, Math.round(Number(v) || lo)));

  return (
    <Dialog
      open={open}
      onOpenChange={(o) => !o && onClose()}
      title={t('showTools.setupTitle')}
      testId="setup-show-dialog"
      width={520}
      footer={
        <>
          <button type="button" onClick={onClose}>
            {t('showTools.cancel')}
          </button>
          <button
            type="button"
            data-testid="setup-ok"
            onClick={() => onApply(kiosk ? { ...s, loop: true } : s, targetOf(where))}
          >
            {t('showTools.ok')}
          </button>
        </>
      }
    >
      <fieldset className={styles.form}>
        <legend>{t('showTools.showType')}</legend>
        {type('speaker')}
        {type('browse')}
        {s.showType === 'browse' && (
          <label className={styles.row}>
            <input
              type="checkbox"
              data-testid="setup-scrollbar"
              checked={s.scrollbar}
              onChange={(e) => setS({ ...s, scrollbar: e.target.checked })}
            />
            {t('showTools.scrollbar')}
          </label>
        )}
        {type('kiosk')}
        {kiosk && (
          <label className={styles.row}>
            {t('showTools.restartAfter')}
            <input
              type="number"
              min={1}
              max={1440}
              data-testid="setup-restart"
              value={Math.round(s.restartMs / 1000)}
              onChange={(e) => setS({ ...s, restartMs: num(e.target.value, 1, 86_400) * 1000 })}
            />
            {t('showTools.seconds')}
          </label>
        )}
      </fieldset>
      <fieldset className={styles.form}>
        <legend>{t('showTools.options')}</legend>
        <label className={styles.row}>
          <input
            type="checkbox"
            data-testid="setup-loop"
            checked={kiosk || s.loop}
            disabled={kiosk}
            onChange={(e) => setS({ ...s, loop: e.target.checked })}
          />
          {t('showTools.loop')}
        </label>
        <label className={styles.row}>
          <input
            type="checkbox"
            data-testid="setup-no-narration"
            checked={!s.narration}
            onChange={(e) => setS({ ...s, narration: !e.target.checked })}
          />
          {t('showTools.noNarration')}
        </label>
        <label className={styles.row}>
          <input
            type="checkbox"
            data-testid="setup-no-animation"
            checked={!s.animation}
            onChange={(e) => setS({ ...s, animation: !e.target.checked })}
          />
          {t('showTools.noAnimation')}
        </label>
        <label className={styles.row}>
          {t('showTools.penColor')}
          <input
            type="color"
            data-testid="setup-pen"
            value={`#${(s.penColor ?? 'D7261E').toLowerCase()}`}
            onChange={(e) => setS({ ...s, penColor: e.target.value.slice(1).toUpperCase() })}
          />
        </label>
      </fieldset>
      <fieldset className={styles.form}>
        <legend>{t('showTools.slides')}</legend>
        <label className={styles.row}>
          <input
            type="radio"
            name="setup-range"
            data-testid="setup-range-all"
            checked={range.kind === 'all'}
            onChange={() => setS({ ...s, range: { kind: 'all' } })}
          />
          {t('showTools.allSlides')}
        </label>
        <div className={styles.row}>
          <input
            type="radio"
            name="setup-range"
            data-testid="setup-range-range"
            aria-label={t('showTools.fromTo')}
            checked={range.kind === 'range'}
            onChange={() => setS({ ...s, range: { kind: 'range', from, to } })}
          />
          {t('showTools.from')}
          <input
            type="number"
            min={1}
            max={count}
            data-testid="setup-from"
            value={from}
            onChange={(e) => {
              const f = num(e.target.value, 1, Math.max(1, count));
              setS({ ...s, range: { kind: 'range', from: f, to: Math.max(f, to) } });
            }}
          />
          {t('showTools.to')}
          <input
            type="number"
            min={1}
            max={count}
            data-testid="setup-to"
            value={to}
            onChange={(e) => {
              const v = num(e.target.value, 1, Math.max(1, count));
              setS({ ...s, range: { kind: 'range', from: Math.min(from, v), to: v } });
            }}
          />
        </div>
        <div className={styles.row}>
          <input
            type="radio"
            name="setup-range"
            data-testid="setup-range-custom"
            aria-label={t('showTools.customShow')}
            disabled={firstShow === undefined}
            checked={range.kind === 'custom'}
            onChange={() =>
              firstShow !== undefined && setS({ ...s, range: { kind: 'custom', id: firstShow } })
            }
          />
          {t('showTools.customShow')}
          <select
            data-testid="setup-custom"
            disabled={firstShow === undefined}
            value={range.kind === 'custom' ? range.id : (firstShow ?? '')}
            onChange={(e) => setS({ ...s, range: { kind: 'custom', id: Number(e.target.value) } })}
          >
            {customShows.map((c) => (
              <option key={c.id} value={c.id}>
                {c.name}
              </option>
            ))}
          </select>
        </div>
      </fieldset>
      <fieldset className={styles.form}>
        <legend>{t('showTools.advance')}</legend>
        <label className={styles.row}>
          <input
            type="radio"
            name="setup-advance"
            data-testid="setup-timings-manual"
            checked={!s.useTimings}
            onChange={() => setS({ ...s, useTimings: false })}
          />
          {t('showTools.manually')}
        </label>
        <label className={styles.row}>
          <input
            type="radio"
            name="setup-advance"
            data-testid="setup-timings-use"
            checked={s.useTimings}
            onChange={() => setS({ ...s, useTimings: true })}
          />
          {t('showTools.useTimings')}
        </label>
      </fieldset>
      {displays && (
        <fieldset className={styles.form}>
          <legend>{t('showTools.display')}</legend>
          <div className={styles.row}>
            <select
              data-testid="setup-display"
              value={where}
              onChange={(e) => setWhere(e.target.value)}
            >
              <option value="window">{t('showTools.displayWindow')}</option>
              {screens.map((sc) => (
                <option key={sc.id} value={`screen:${sc.id}`}>
                  {t('showTools.displayScreen', {
                    name: sc.label || t('showTools.screenN', { n: Number(sc.id) + 1 }),
                    w: Math.round(sc.width),
                    h: Math.round(sc.height),
                  })}
                  {sc.primary ? ` · ${t('showTools.primary')}` : ''}
                </option>
              ))}
              {where.startsWith('screen:') && !screens.some((x) => `screen:${x.id}` === where) && (
                <option value={where}>
                  {t('showTools.screenN', { n: Number(where.slice(7)) + 1 })}
                </option>
              )}
              {displays.presentation && (
                <option value="presentation">{t('showTools.displayPresentation')}</option>
              )}
            </select>
            {displays.screens && findScreens && (
              <button
                type="button"
                data-testid="setup-find-screens"
                disabled={busy}
                onClick={() => {
                  setBusy(true);
                  void findScreens()
                    .then(setScreens)
                    .catch(() => setScreens([]))
                    .finally(() => setBusy(false));
                }}
              >
                {t('showTools.findScreens')}
              </button>
            )}
          </div>
          <span className={styles.muted}>{t('showTools.displayHint')}</span>
        </fieldset>
      )}
    </Dialog>
  );
}
