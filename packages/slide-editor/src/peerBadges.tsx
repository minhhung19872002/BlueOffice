/**
 * Live collaboration presence on the slide thumbnails (CORE-COLLAB-001): a badge
 * with the initials of every collaborator on that slide, in their colour.
 */
import { initials, type SlidePeer } from '@blueoffice/platform';
import { useTranslation } from '@blueoffice/i18n';
import styles from './SlideEditor.module.css';

/** Collaborators grouped by slide index. */
export function peersBySlide<P extends { slide: number }>(peers: P[]): Map<number, P[]> {
  const out = new Map<number, P[]>();
  for (const p of peers) {
    const list = out.get(p.slide) ?? [];
    list.push(p);
    out.set(p.slide, list);
  }
  return out;
}

/** Most badges drawn on one thumbnail; the rest are counted in a "+n" badge. */
const MAX_BADGES = 3;

export function PeerBadges({ peers }: { peers: SlidePeer[] | undefined }) {
  const { t } = useTranslation();
  if (!peers?.length) return null;
  const shown = peers.slice(0, MAX_BADGES);
  const more = peers.length - shown.length;
  return (
    <span className={styles.thumbPeers}>
      {shown.map((p) => {
        const name = p.name || t('collab.anonymous');
        const label = t('collab.onSlide', { name });
        return (
          <span
            key={p.clientId}
            className={styles.peerBadge}
            data-testid="peer-badge"
            role="img"
            aria-label={label}
            title={label}
            style={{ background: p.color }}
          >
            {initials(name)}
          </span>
        );
      })}
      {more > 0 && (
        <span className={styles.peerBadge} data-testid="peer-badge-more">
          +{more}
        </span>
      )}
    </span>
  );
}
