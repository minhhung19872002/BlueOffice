import { Popover } from '@blueoffice/ui';
import { useTranslation } from '@blueoffice/i18n';
import { ChevronDown, ListChecks } from 'lucide-react';
import styles from './ReviewingPane.module.css';

/** A checkable entry backed by a command-registry command. */
export interface MarkupEntry {
  id: string;
  label: string;
  checked: boolean;
  enabled: boolean;
  onSelect: () => void;
}

/** Review display mode dropdown (Review tab › Tracking); one entry per mode command. */
export function MarkupModeSelect({
  entries,
  disabled,
}: {
  entries: MarkupEntry[];
  disabled: boolean;
}) {
  const { t } = useTranslation();
  const current = entries.find((e) => e.checked)?.id ?? entries[0]?.id ?? '';
  return (
    <select
      className={styles.modeSelect}
      data-testid="markup-mode"
      aria-label={t('commands.review.markupMode')}
      title={t('commands.review.markupMode')}
      disabled={disabled}
      value={current}
      onChange={(ev) => entries.find((e) => e.id === ev.target.value)?.onSelect()}
    >
      {entries.map((e) => (
        <option key={e.id} value={e.id} disabled={!e.enabled}>
          {e.label}
        </option>
      ))}
    </select>
  );
}

/**
 * "Marks to show" menu: what the markup marks (commands) and the reviewers whose
 * changes and comments are shown.
 */
export function ShowMarkupMenu({
  entries,
  reviewers,
  hidden,
  onReviewers,
  disabled,
}: {
  entries: MarkupEntry[];
  /** Every reviewer of the document. */
  reviewers: string[];
  /** Reviewers currently hidden. */
  hidden: string[];
  /** New list of hidden reviewers. */
  onReviewers: (hidden: string[]) => void;
  disabled: boolean;
}) {
  const { t } = useTranslation();
  return (
    <Popover
      label={t('commands.review.showMarkup')}
      disabled={disabled}
      testId="show-markup"
      trigger={
        <>
          <ListChecks size={16} />
          <ChevronDown size={12} />
        </>
      }
    >
      <div role="menu" aria-label={t('commands.review.showMarkup')}>
        {entries.map((e) => (
          <label key={e.id} className={styles.check} data-command={e.id}>
            <input
              type="checkbox"
              role="menuitemcheckbox"
              aria-checked={e.checked}
              checked={e.checked}
              disabled={!e.enabled}
              onChange={() => e.onSelect()}
            />
            {e.label}
          </label>
        ))}
        {reviewers.length > 0 && (
          <>
            <div className={styles.meta} style={{ padding: '4px 8px 0' }}>
              {t('commands.review.markupReviewers')}
            </div>
            <label className={styles.check}>
              <input
                type="checkbox"
                data-testid="markup-reviewer-all"
                checked={hidden.length === 0}
                onChange={() => onReviewers(hidden.length === 0 ? [...reviewers] : [])}
              />
              {t('commands.review.markupAllReviewers')}
            </label>
            {reviewers.map((r) => (
              <label key={r} className={styles.check} style={{ paddingLeft: 20 }}>
                <input
                  type="checkbox"
                  data-testid="markup-reviewer"
                  data-reviewer={r}
                  checked={!hidden.includes(r)}
                  onChange={() =>
                    onReviewers(hidden.includes(r) ? hidden.filter((h) => h !== r) : [...hidden, r])
                  }
                />
                {r}
              </label>
            ))}
          </>
        )}
      </div>
    </Popover>
  );
}

/** Authors of the document's revisions and comments, sorted. */
export function reviewersOf(
  review: {
    revisions: { author: string }[];
    comments: { author: string }[];
  } | null,
): string[] {
  if (!review) return [];
  const all = new Set([...review.revisions, ...review.comments].map((x) => x.author));
  return [...all].sort((a, b) => a.localeCompare(b));
}
