import { useTranslation } from '@blueoffice/i18n';
import type { SlideCustomShow } from '@blueoffice/platform';
import { Dialog } from '@blueoffice/ui';
import { useState } from 'react';
import styles from './showTools.module.css';

export interface CustomShowsDialogProps {
  open: boolean;
  shows: SlideCustomShow[];
  /** Slides of the deck. */
  count: number;
  /** Short label of slide `i` (its title, if any). */
  slideLabel: (i: number) => string;
  /** Shapes are selected: they can link to a show. */
  canLink: boolean;
  error?: string;
  onClose: () => void;
  onSave: (id: number | undefined, name: string, slides: number[]) => void;
  onDelete: (id: number) => void;
  onPlay: (id: number) => void;
  onLink: (id: number, ret: boolean) => void;
}

interface Draft {
  id: number | undefined;
  name: string;
  slides: number[];
}

/**
 * Custom shows (§19.2.1.6): list them, play one, link the selected shapes to
 * one, and create / edit a show by picking slides into a playing order.
 */
export function CustomShowsDialog({
  open,
  shows,
  count,
  slideLabel,
  canLink,
  error,
  onClose,
  onSave,
  onDelete,
  onPlay,
  onLink,
}: CustomShowsDialogProps) {
  const { t } = useTranslation();
  const [draft, setDraft] = useState<Draft | null>(null);
  const [ret, setRet] = useState(true);
  const close = () => {
    setDraft(null);
    onClose();
  };
  const move = (k: number, d: -1 | 1) =>
    setDraft((x) => {
      if (!x) return x;
      const j = k + d;
      if (j < 0 || j >= x.slides.length) return x;
      const s = [...x.slides];
      [s[k], s[j]] = [s[j]!, s[k]!];
      return { ...x, slides: s };
    });

  const footer = draft ? (
    <>
      <button type="button" data-testid="cs-cancel" onClick={() => setDraft(null)}>
        {t('showTools.back')}
      </button>
      <button
        type="button"
        data-testid="cs-save"
        disabled={!draft.name.trim() || draft.slides.length === 0}
        onClick={() => {
          onSave(draft.id, draft.name.trim(), draft.slides);
          setDraft(null);
        }}
      >
        {t('showTools.save')}
      </button>
    </>
  ) : (
    <>
      <button
        type="button"
        data-testid="cs-new"
        onClick={() =>
          setDraft({
            id: undefined,
            name: t('showTools.newShowName', { n: shows.length + 1 }),
            slides: [],
          })
        }
      >
        {t('showTools.newShow')}
      </button>
      <button type="button" data-testid="cs-close" onClick={close}>
        {t('showTools.close')}
      </button>
    </>
  );

  return (
    <Dialog
      open={open}
      onOpenChange={(o) => !o && close()}
      title={t('showTools.customShows')}
      testId="custom-shows-dialog"
      width={draft ? 640 : 460}
      footer={footer}
    >
      {error && (
        <p role="alert" className={styles.muted} data-testid="cs-error">
          {t(`showTools.err.${error}`, { defaultValue: error })}
        </p>
      )}
      {!draft ? (
        <>
          {shows.length === 0 ? (
            <p className={styles.muted}>{t('showTools.noShows')}</p>
          ) : (
            <ul className={styles.list} data-testid="cs-list">
              {shows.map((s) => (
                <li key={s.id} data-testid={`cs-show-${s.id}`}>
                  <span title={s.name}>
                    {s.name}{' '}
                    <span className={styles.muted}>
                      {t('showTools.slideCount', { n: s.slides.length })}
                    </span>
                  </span>
                  <span className={styles.row}>
                    <button
                      type="button"
                      data-testid={`cs-play-${s.id}`}
                      onClick={() => {
                        setDraft(null);
                        onPlay(s.id);
                      }}
                    >
                      {t('showTools.play')}
                    </button>
                    <button
                      type="button"
                      data-testid={`cs-edit-${s.id}`}
                      onClick={() => setDraft({ id: s.id, name: s.name, slides: [...s.slides] })}
                    >
                      {t('showTools.edit')}
                    </button>
                    <button
                      type="button"
                      data-testid={`cs-link-${s.id}`}
                      disabled={!canLink}
                      title={t('showTools.linkHint')}
                      onClick={() => onLink(s.id, ret)}
                    >
                      {t('showTools.link')}
                    </button>
                    <button
                      type="button"
                      data-testid={`cs-delete-${s.id}`}
                      onClick={() => onDelete(s.id)}
                    >
                      {t('showTools.delete')}
                    </button>
                  </span>
                </li>
              ))}
            </ul>
          )}
          <label className={styles.row}>
            <input
              type="checkbox"
              data-testid="cs-link-return"
              checked={ret}
              onChange={(e) => setRet(e.target.checked)}
            />
            {t('showTools.linkReturn')}
          </label>
        </>
      ) : (
        <>
          <label className={styles.row}>
            {t('showTools.showName')}
            <input
              data-testid="cs-name"
              value={draft.name}
              onChange={(e) => setDraft({ ...draft, name: e.target.value })}
            />
          </label>
          <div className={styles.columns}>
            <div>
              <strong>{t('showTools.deckSlides')}</strong>
              <ul className={styles.list}>
                {Array.from({ length: count }, (_, i) => (
                  <li key={i}>
                    <span title={slideLabel(i)}>{slideLabel(i)}</span>
                    <button
                      type="button"
                      data-testid={`cs-add-${i}`}
                      aria-label={t('showTools.addSlide', { n: i + 1 })}
                      onClick={() => setDraft({ ...draft, slides: [...draft.slides, i] })}
                    >
                      →
                    </button>
                  </li>
                ))}
              </ul>
            </div>
            <span />
            <div>
              <strong>{t('showTools.showSlides')}</strong>
              <ul className={styles.list} data-testid="cs-items">
                {draft.slides.map((s, k) => (
                  <li key={`${k}-${s}`} data-testid={`cs-item-${k}`} data-slide={s}>
                    <span>{slideLabel(s)}</span>
                    <span className={styles.row}>
                      <button
                        type="button"
                        data-testid={`cs-up-${k}`}
                        aria-label={t('showTools.moveUp')}
                        disabled={k === 0}
                        onClick={() => move(k, -1)}
                      >
                        ↑
                      </button>
                      <button
                        type="button"
                        data-testid={`cs-down-${k}`}
                        aria-label={t('showTools.moveDown')}
                        disabled={k === draft.slides.length - 1}
                        onClick={() => move(k, 1)}
                      >
                        ↓
                      </button>
                      <button
                        type="button"
                        data-testid={`cs-remove-${k}`}
                        aria-label={t('showTools.remove')}
                        onClick={() =>
                          setDraft({ ...draft, slides: draft.slides.filter((_, j) => j !== k) })
                        }
                      >
                        ✕
                      </button>
                    </span>
                  </li>
                ))}
              </ul>
            </div>
          </div>
        </>
      )}
    </Dialog>
  );
}
