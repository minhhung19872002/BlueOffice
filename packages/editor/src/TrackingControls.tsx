/**
 * Track changes extras (DOC-REVIEW-002): locking tracking with a password (ECMA-376
 * Part 1, §17.15.1.29 documentProtection `w:edit="trackedChanges"`), moving paragraphs
 * as tracked moves (§17.13.5.22 moveFrom / §17.13.5.25 moveTo) and naming the
 * properties a formatting revision changed (§17.13.5.29 pPrChange, §17.13.5.31 rPrChange).
 */
import { useTranslation } from '@blueoffice/i18n';
import type { EditCommand, ReviewState, RevisionItem } from '@blueoffice/platform';
import { Dialog } from '@blueoffice/ui';
import { useState } from 'react';
import type { CommandRegistry } from './commands';
import styles from './Editor.module.css';
import type { EditorState } from './store';

/** Engine error of a change refused while tracking is locked (`bo_edit::TRACKING_LOCKED`). */
export const TRACKING_LOCKED = 'review:locked';

/** i18n key of a review error, or null when the error is another one. */
export function reviewErrorKey(error: string): string | null {
  return error === TRACKING_LOCKED ? 'review.lock.lockedError' : null;
}

/** Translate the changed-property keys of a formatting revision ("Bold, Italic"). */
export function revisionDetail(
  t: (key: string, opts?: { defaultValue: string }) => string,
  item: Pick<RevisionItem, 'detail'>,
): string {
  return (item.detail ?? []).map((k) => t(`review.prop.${k}`, { defaultValue: k })).join(', ');
}

/** Label of a change kind in a balloon, with the changed properties of a formatting change. */
export function balloonLabel(
  t: (key: string, opts?: { defaultValue: string }) => string,
  kind: string,
  item: Pick<RevisionItem, 'detail'> | undefined,
): string {
  const base = t(`markup.kind.${kind}`);
  const detail = item ? revisionDetail(t, item) : '';
  return detail ? `${base} (${detail})` : base;
}

/** Review tab commands of this feature. */
export function registerTrackingCommands(
  r: CommandRegistry<EditorState>,
  deps: {
    hasDoc: (s: EditorState) => boolean;
    run: (c: EditCommand) => Promise<void>;
    openLockDialog: () => void;
  },
): void {
  const { hasDoc, run } = deps;
  r.register({
    id: 'review.lockTracking',
    isEnabled: hasDoc,
    isChecked: (s) => !!s.review?.trackingLocked,
    run: () => deps.openLockDialog(),
  });
  r.register({
    id: 'review.moveParagraphUp',
    isEnabled: (s) => hasDoc(s) && !s.edit?.story,
    run: () => run({ cmd: 'moveParagraphs', up: true }),
  });
  r.register({
    id: 'review.moveParagraphDown',
    isEnabled: (s) => hasDoc(s) && !s.edit?.story,
    run: () => run({ cmd: 'moveParagraphs', up: false }),
  });
}

/**
 * Lock tracking (track changes stays on; accepting, rejecting and turning tracking off
 * need the password) or, when locked, unlock it.
 */
export function LockTrackingDialog({
  review,
  onApply,
  onClose,
}: {
  review: ReviewState | null;
  onApply: (cmd: EditCommand) => void;
  onClose: () => void;
}) {
  const { t } = useTranslation();
  const locked = !!review?.trackingLocked;
  const [password, setPassword] = useState('');
  const [confirm, setConfirm] = useState('');
  const mismatch = !locked && password !== confirm;
  const submit = () => {
    if (locked) onApply({ cmd: 'unprotectDocument', password: password || null });
    else onApply({ cmd: 'protectDocument', edit: 'trackedChanges', password: password || null });
  };
  return (
    <Dialog
      open
      onOpenChange={(o) => !o && onClose()}
      title={locked ? t('review.lock.stopTitle') : t('review.lock.title')}
      testId="lock-tracking-dialog"
      footer={
        <>
          <button type="button" className={styles.dialogButton} onClick={onClose}>
            {t('review.lock.cancel')}
          </button>
          <button
            type="button"
            className={styles.dialogPrimary}
            data-testid="lock-tracking-ok"
            disabled={mismatch}
            onClick={submit}
          >
            {locked ? t('review.lock.stop') : t('review.lock.start')}
          </button>
        </>
      }
    >
      <div className={styles.panelMessage}>
        {locked ? t('review.lock.stopHint') : t('review.lock.hint')}
      </div>
      {(!locked || review?.lockPassword) && (
        <label className={styles.field}>
          {t('review.lock.password')}
          <input
            type="password"
            data-testid="lock-tracking-password"
            value={password}
            onChange={(e) => setPassword(e.target.value)}
          />
        </label>
      )}
      {!locked && (
        <label className={styles.field}>
          {t('review.lock.confirm')}
          <input
            type="password"
            data-testid="lock-tracking-confirm"
            value={confirm}
            onChange={(e) => setConfirm(e.target.value)}
          />
        </label>
      )}
      {mismatch && confirm !== '' && (
        <div className={styles.panelMessage} role="alert">
          {t('review.lock.mismatch')}
        </div>
      )}
    </Dialog>
  );
}
